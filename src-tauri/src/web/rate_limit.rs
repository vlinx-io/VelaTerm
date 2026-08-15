//! Login rate limiting and Argon2 verification throttling for the unauthenticated surface.
//!
//! `POST /api/login` and the WebSocket E2EE handshake both verify an Argon2id password and are reachable
//! without credentials, so they form a DoS-amplification primitive: each request costs the server a
//! memory-hard hash. Two independent brakes contain that:
//!
//! - [`LoginRateLimiter`]: a per-IP fixed window of login attempts. A blocked IP is rejected before any
//!   Argon2 work happens. The limiter is in-memory by design — a restart resets it, which is acceptable
//!   because the pairing token plus Argon2 remain as the actual credential barrier. One limiter is
//!   shared per data directory across every in-process server instance (see [`LoginRateLimiter::shared`],
//!   the `PAIRING_STORES` pattern from auth.rs), so a dual-instance `--serve` setup no longer doubles an
//!   attacker's budget. [`allow`](LoginRateLimiter::allow) *reserves* an attempt atomically, so N
//!   parallel requests from one IP cannot slip under the limit before any of them records a failure.
//!   The reservation is an RAII [`AttemptGuard`]: dropping it without an explicit outcome (a client
//!   disconnecting mid-verify cancels the request future) releases the slot immediately instead of
//!   leaking it until the window prune.
//! - [`VERIFY_SEMAPHORE`]: a process-wide cap on concurrent Argon2 verifications, combined with
//!   `spawn_blocking` in `AuthState::verify_password_async` so the async executor is never blocked.

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::{Duration, Instant};

use tokio::sync::Semaphore;

/// Maximum failed attempts per IP within one window before further attempts are rejected.
const MAX_FAILURES: u32 = 5;
/// Fixed window length; the failure counter of an IP resets when its window expires.
const WINDOW: Duration = Duration::from_secs(60);

/// Process-wide bound on concurrent Argon2id verifications across all server instances. Two permits keep
/// interactive logins responsive while denying an attacker the ability to saturate every blocking thread
/// with memory-hard hashing.
pub static VERIFY_SEMAPHORE: Semaphore = Semaphore::const_new(2);

/// Per-IP login-attempt window. All mutating accesses prune expired entries so the map cannot grow
/// unboundedly under a spread of source addresses.
pub struct LoginRateLimiter {
    entries: Mutex<HashMap<IpAddr, WindowEntry>>,
}

struct WindowEntry {
    window_start: Instant,
    /// Completed failed attempts within this window.
    failures: u32,
    /// Attempts reserved by [`LoginRateLimiter::allow`] whose outcome is still pending. Counted
    /// against the limit so parallel requests from one IP cannot all pass the check before the
    /// first failure is recorded (TOCTOU). A failure converts a reservation into a failure; a
    /// success or a dropped [`AttemptGuard`] releases only its own reservation.
    pending: u32,
}

/// Process-wide registry of live limiters keyed by canonicalized data directory (the `PAIRING_STORES`
/// pattern). Weak entries let a limiter die with its last server instance; sister instances serving the
/// same data directory share one budget instead of multiplying it.
static LOGIN_LIMITERS: OnceLock<Mutex<HashMap<PathBuf, Weak<LoginRateLimiter>>>> = OnceLock::new();

impl LoginRateLimiter {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// Shared limiter for one data directory: every in-process server instance over the same directory
    /// (CLI `--serve` primary, auto-started secondary, GUI/Electron) gets the same limiter, so an
    /// attacker cannot multiply the per-IP budget by the number of instances.
    pub fn shared(data_dir: &Path) -> Arc<Self> {
        // Canonicalize so `/tmp/x` and `/private/tmp/x` (macOS) resolve to the same limiter; fall back
        // to the raw path when the directory cannot be resolved.
        let key = data_dir
            .canonicalize()
            .unwrap_or_else(|_| data_dir.to_path_buf());
        let registry = LOGIN_LIMITERS.get_or_init(|| Mutex::new(HashMap::new()));
        let mut map = registry.lock().unwrap();
        // Prune dead entries so the map does not accumulate one Weak per finished server lifetime.
        map.retain(|_, w| w.strong_count() > 0);
        if let Some(existing) = map.get(&key).and_then(Weak::upgrade) {
            return existing;
        }
        let limiter = Arc::new(Self::new());
        map.insert(key, Arc::downgrade(&limiter));
        limiter
    }

    /// Whether this IP may attempt a login now. `Some` **reserves** one attempt against the window
    /// immediately and hands it back as an RAII [`AttemptGuard`]: resolve it with
    /// [`AttemptGuard::failure`] or [`AttemptGuard::success`]; merely dropping it (the request future
    /// was cancelled, e.g. a client disconnect during the Argon2 await) releases the reservation at
    /// once instead of leaking it until the window prune.
    pub fn allow(self: &Arc<Self>, ip: IpAddr) -> Option<AttemptGuard> {
        if self.allow_at(ip, Instant::now()) {
            Some(AttemptGuard {
                limiter: Arc::clone(self),
                ip,
                resolved: false,
            })
        } else {
            None
        }
    }

    /// Record a failed login attempt for this IP, converting its pending reservation into a failure.
    fn record_failure(&self, ip: IpAddr) {
        self.record_failure_at(ip, Instant::now());
    }

    /// Release one pending reservation for this IP without recording a failure — used by a successful
    /// login and by a dropped [`AttemptGuard`]. Deliberately surgical: it never touches the failure
    /// counter or other requests' reservations, because behind a shared NAT one legitimate success
    /// must not refresh an attacker's budget (the old whole-entry removal did exactly that). A
    /// legitimate user who mistyped keeps the recorded failures until the 60s window expires.
    fn release_pending(&self, ip: IpAddr) {
        self.release_pending_at(ip, Instant::now());
    }

    /// Time-injectable core of [`allow`], used by tests to step through window expiry.
    fn allow_at(&self, ip: IpAddr, now: Instant) -> bool {
        let mut entries = self.entries.lock().unwrap();
        prune(&mut entries, now);
        let e = entries.entry(ip).or_insert(WindowEntry {
            window_start: now,
            failures: 0,
            pending: 0,
        });
        if e.failures.saturating_add(e.pending) >= MAX_FAILURES {
            return false;
        }
        e.pending = e.pending.saturating_add(1);
        true
    }

    /// Time-injectable core of [`record_failure`].
    fn record_failure_at(&self, ip: IpAddr, now: Instant) {
        let mut entries = self.entries.lock().unwrap();
        prune(&mut entries, now);
        let e = entries.entry(ip).or_insert(WindowEntry {
            window_start: now,
            failures: 0,
            pending: 0,
        });
        e.pending = e.pending.saturating_sub(1);
        e.failures = e.failures.saturating_add(1);
    }

    /// Time-injectable core of [`release_pending`]. An entry whose window already expired was pruned
    /// together with its reservations, so a missing entry is a valid no-op.
    fn release_pending_at(&self, ip: IpAddr, now: Instant) {
        let mut entries = self.entries.lock().unwrap();
        prune(&mut entries, now);
        if let Some(e) = entries.get_mut(&ip) {
            e.pending = e.pending.saturating_sub(1);
        }
    }
}

/// RAII reservation handle returned by [`LoginRateLimiter::allow`]. Exactly one of [`failure`]
/// (converts the reservation into a counted failure) or [`success`] (releases the reservation
/// surgically, see [`LoginRateLimiter::release_pending`]) should be called; dropping the guard
/// unresolved — the request future was cancelled — releases the reservation immediately.
///
/// [`failure`]: AttemptGuard::failure
/// [`success`]: AttemptGuard::success
pub struct AttemptGuard {
    limiter: Arc<LoginRateLimiter>,
    ip: IpAddr,
    resolved: bool,
}

impl AttemptGuard {
    /// Convert the reservation into a recorded failure.
    pub fn failure(mut self) {
        self.resolved = true;
        self.limiter.record_failure(self.ip);
    }

    /// Release the reservation after a successful login without touching recorded failures.
    pub fn success(mut self) {
        self.resolved = true;
        self.limiter.release_pending(self.ip);
    }
}

impl Drop for AttemptGuard {
    fn drop(&mut self) {
        if !self.resolved {
            self.limiter.release_pending(self.ip);
        }
    }
}

impl Default for LoginRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// Drop entries whose window has expired; called on every access so memory stays bounded.
fn prune(entries: &mut HashMap<IpAddr, WindowEntry>, now: Instant) {
    entries.retain(|_, e| now.duration_since(e.window_start) < WINDOW);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn ip(last: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, last))
    }

    #[test]
    fn blocks_after_max_failures_and_recovers_after_window() {
        let l = LoginRateLimiter::new();
        let t0 = Instant::now();
        for _ in 0..MAX_FAILURES {
            assert!(l.allow_at(ip(1), t0));
            l.record_failure_at(ip(1), t0);
        }
        // The sixth attempt within the window is rejected.
        assert!(!l.allow_at(ip(1), t0));
        // Still rejected just before the window expires.
        assert!(!l.allow_at(ip(1), t0 + WINDOW - Duration::from_millis(1)));
        // Allowed again once the fixed window has passed.
        assert!(l.allow_at(ip(1), t0 + WINDOW));
    }

    /// Shared-NAT semantics: a success releases ONLY its own reservation and never refreshes the
    /// failure counter — otherwise one legitimate login behind the same NAT would reset an
    /// attacker's budget. Failures therefore persist until the window expires, success or not.
    #[test]
    fn success_does_not_refresh_failures_behind_shared_nat() {
        let l = Arc::new(LoginRateLimiter::new());
        // Attacker on the shared IP burns 4 of 5 attempts.
        for _ in 0..MAX_FAILURES - 1 {
            l.allow(ip(1)).expect("within budget").failure();
        }
        // Legitimate user on the same IP logs in successfully.
        l.allow(ip(1)).expect("one attempt left").success();
        // The success released only its own reservation: exactly one attempt remains, then blocked.
        l.allow(ip(1)).expect("failures must be untouched by the success").failure();
        assert!(l.allow(ip(1)).is_none(), "budget must be exhausted, not refreshed");
    }

    #[test]
    fn ips_are_independent() {
        let l = LoginRateLimiter::new();
        let t0 = Instant::now();
        for _ in 0..MAX_FAILURES {
            l.record_failure_at(ip(1), t0);
        }
        assert!(!l.allow_at(ip(1), t0));
        assert!(l.allow_at(ip(2), t0), "another IP must not inherit the block");
    }

    #[test]
    fn expired_entries_are_pruned_so_the_map_stays_bounded() {
        let l = LoginRateLimiter::new();
        let t0 = Instant::now();
        for last in 1..=100u8 {
            l.record_failure_at(ip(last), t0);
        }
        assert_eq!(l.entries.lock().unwrap().len(), 100);
        // Any access after the window prunes every expired entry; only the fresh reservation that
        // this allow itself creates remains.
        assert!(l.allow_at(ip(1), t0 + WINDOW));
        assert_eq!(l.entries.lock().unwrap().len(), 1);
    }

    /// TOCTOU regression: `allow` reserves the attempt, so MAX_FAILURES parallel requests from one IP
    /// exhaust the budget even before any of them records its failure.
    #[test]
    fn parallel_reservations_cannot_undercut_the_limit() {
        let l = LoginRateLimiter::new();
        let t0 = Instant::now();
        // Simulate N in-flight requests: all call allow before any outcome is recorded.
        for _ in 0..MAX_FAILURES {
            assert!(l.allow_at(ip(1), t0));
        }
        // Attempt N+1 is rejected although zero failures have been recorded yet.
        assert!(!l.allow_at(ip(1), t0));
        // The in-flight requests now fail; the budget stays exhausted, not doubled.
        for _ in 0..MAX_FAILURES {
            l.record_failure_at(ip(1), t0);
        }
        assert!(!l.allow_at(ip(1), t0));
        // Reservations expire with the window like failures do.
        assert!(l.allow_at(ip(1), t0 + WINDOW));
    }

    /// A success releases its reservation, so repeated legitimate logins never block the next attempt.
    #[test]
    fn success_releases_the_reservation() {
        let l = Arc::new(LoginRateLimiter::new());
        for _ in 0..MAX_FAILURES {
            l.allow(ip(1)).expect("a released reservation frees the slot").success();
        }
        assert!(l.allow(ip(1)).is_some());
    }

    /// FIX for the cancelled-request leak: a guard dropped without an outcome (the request future was
    /// cancelled by a client disconnect during the Argon2 await) releases its reservation at once —
    /// the slot is free immediately, not only after the 60s window prune.
    #[test]
    fn dropped_guard_releases_the_reservation_immediately() {
        let l = Arc::new(LoginRateLimiter::new());
        // Exhaust the budget with unresolved in-flight reservations.
        let guards: Vec<_> = (0..MAX_FAILURES)
            .map(|_| l.allow(ip(1)).expect("within budget"))
            .collect();
        assert!(l.allow(ip(1)).is_none(), "in-flight reservations must count");
        // Cancel every request: dropping the guards frees the slots without any window expiry.
        drop(guards);
        assert!(
            l.allow(ip(1)).is_some(),
            "a dropped guard must release its reservation immediately"
        );
    }

    /// An explicitly resolved guard releases exactly once: were Drop to release a second time, it
    /// would steal a *different* request's still-pending reservation and re-open its slot.
    #[test]
    fn resolved_guard_does_not_double_release_on_drop() {
        let l = Arc::new(LoginRateLimiter::new());
        // One reservation stays in flight while two others resolve; a double release would
        // decrement the in-flight reservation away.
        let mut guards = vec![l.allow(ip(1)).expect("in-flight reservation")];
        l.allow(ip(1)).expect("second attempt").failure();
        l.allow(ip(1)).expect("third attempt").success();
        // Correct accounting: 1 pending + 1 failure leaves exactly MAX_FAILURES - 2 slots.
        for _ in 0..MAX_FAILURES - 2 {
            guards.push(l.allow(ip(1)).expect("remaining budget"));
        }
        assert!(
            l.allow(ip(1)).is_none(),
            "a resolved guard must not release a second time on drop"
        );
    }

    /// Dual-instance fix: two server instances over the same data directory share ONE limiter, so an
    /// attacker gets one budget, not one per instance. Different directories stay independent, and the
    /// registry is Weak: once the last instance drops its Arc, a later instance gets a fresh limiter.
    #[test]
    fn limiter_is_shared_per_data_dir() {
        let dir_a = std::env::temp_dir().join(format!(
            "vlx-rl-a-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        let dir_b = std::env::temp_dir().join(format!(
            "vlx-rl-b-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&dir_a).unwrap();
        std::fs::create_dir_all(&dir_b).unwrap();

        let first = LoginRateLimiter::shared(&dir_a);
        let second = LoginRateLimiter::shared(&dir_a);
        assert!(
            std::sync::Arc::ptr_eq(&first, &second),
            "instances over the same data dir must share one limiter"
        );

        // Exhaust the budget through the first handle; the second handle sees the block immediately.
        let t0 = Instant::now();
        for _ in 0..MAX_FAILURES {
            assert!(first.allow_at(ip(9), t0));
            first.record_failure_at(ip(9), t0);
        }
        assert!(!second.allow_at(ip(9), t0), "5x5 dual-instance budget must be closed");

        // An unrelated data dir gets its own limiter and budget.
        let other = LoginRateLimiter::shared(&dir_b);
        assert!(!std::sync::Arc::ptr_eq(&first, &other));
        assert!(other.allow_at(ip(9), t0));

        // Weak registry: dropping every Arc lets the limiter die; the next shared() starts fresh.
        drop(first);
        drop(second);
        let revived = LoginRateLimiter::shared(&dir_a);
        assert!(revived.allow_at(ip(9), t0), "a revived limiter starts with an empty window");

        let _ = std::fs::remove_dir_all(&dir_a);
        let _ = std::fs::remove_dir_all(&dir_b);
    }
}
