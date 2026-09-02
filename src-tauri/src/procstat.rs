//! Sample CPU and memory for a PID and its complete descendant tree, plus whole-machine CPU, memory,
//! and saturation, for the Info panel's Resources section.
//!
//! Agent PTYs launch a shell whose Claude process and commands are descendants. Measuring only the
//! root is nearly zero, so subtree totals represent the session's actual resource use.
//!
//! All platforms share a persistent `sysinfo` sampler using native in-process APIs (`/proc`,
//! libproc/sysctl, or Windows APIs). This replaces the previous non-Windows `ps -axo` subprocess,
//! which scanned the entire process table every two seconds.
//!
//! CPU percentage is an increment between refreshes and requires a persistent `System`; a newly
//! created one often reports zero and has a roughly 200 ms minimum interval. A process-wide
//! `Mutex<System>` retains snapshots, reconstructs descendants by ppid, and sums CPU/memory. CPU
//! warms up over one or two samples, while memory is an accurate instantaneous snapshot immediately.
//!
//! Sampling is machine-wide, not per session or per client: every Info panel on every window, browser,
//! and remote device reads these same two samplers. That makes the refresh interval shared state, which
//! is why both entry points refuse to refresh more often than `MIN_REFRESH`. Without that floor, two
//! clients polling 0.2 s apart would leave each other a 0.2 s CPU window and both would read near zero.
//! Callers within the floor get the previous snapshot, which is at most `MIN_REFRESH` old.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcStats {
    /// Sum of subtree CPU percentages; 100% is one core and totals may exceed it.
    pub cpu: f64,
    /// Sum of subtree resident memory (RSS) in bytes.
    pub rss_bytes: u64,
}

/// Shortest interval between two real refreshes. Clients poll every 3 s, so a single client never hits
/// this floor; it only collapses the extra refreshes that additional clients would otherwise force.
const MIN_REFRESH: Duration = Duration::from_millis(2000);

/// A `System` plus the moment it was last refreshed, so callers can honor `MIN_REFRESH`.
struct Sampler {
    sys: System,
    refreshed_at: Option<Instant>,
}

impl Sampler {
    fn new() -> Self {
        Sampler {
            sys: System::new(),
            refreshed_at: None,
        }
    }

    /// Whether the retained snapshot is still fresh enough to answer without refreshing.
    fn fresh(&self) -> bool {
        self.refreshed_at
            .is_some_and(|t| t.elapsed() < MIN_REFRESH)
    }

    fn mark_refreshed(&mut self) {
        self.refreshed_at = Some(Instant::now());
    }
}

/// Process-wide persistent sampler retaining the previous snapshot for CPU deltas.
fn sampler() -> &'static Mutex<Sampler> {
    static SYS: OnceLock<Mutex<Sampler>> = OnceLock::new();
    SYS.get_or_init(|| Mutex::new(Sampler::new()))
}

/// Sum CPU percentage and RSS for the process tree rooted at `pid`. Refresh the persistent `System`,
/// reconstruct parent-child relationships by ppid, and traverse from the root. Return None if absent.
pub fn subtree_stats(pid: u32) -> Option<ProcStats> {
    let mut guard = sampler().lock().unwrap_or_else(|e| e.into_inner());
    // Sum from the retained snapshot when another client just refreshed it: the process table is the
    // expensive part here, and a snapshot under two seconds old is indistinguishable in the panel.
    if !guard.fresh() {
        // Refresh only CPU and memory; refresh_all would unnecessarily scan disks and networks. The true
        // flag removes exited processes from the snapshot.
        guard.sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cpu().with_memory(),
        );
        guard.mark_refreshed();
    }
    let sys = &guard.sys;

    let root = Pid::from_u32(pid);
    // Return None when the target process does not exist.
    if sys.process(root).is_none() {
        return None;
    }

    // Map ppid to child PIDs, copying Pid values to release the System borrow.
    let mut children: HashMap<Pid, Vec<Pid>> = HashMap::new();
    for (p, process) in sys.processes() {
        if let Some(parent) = process.parent() {
            children.entry(parent).or_default().push(*p);
        }
    }

    // Traverse from the root and accumulate CPU percentage and RSS.
    let mut cpu = 0.0_f64;
    let mut rss_bytes: u64 = 0;
    let mut stack = vec![root];
    let mut seen: HashMap<Pid, ()> = HashMap::new();
    while let Some(cur) = stack.pop() {
        if seen.insert(cur, ()).is_some() {
            continue; // Defensive cycle guard; valid process trees are acyclic.
        }
        if let Some(process) = sys.process(cur) {
            cpu += process.cpu_usage() as f64; // 100% equals one core.
            rss_bytes += process.memory(); // sysinfo 0.33 reports memory in bytes.
        }
        if let Some(kids) = children.get(&cur) {
            stack.extend(kids.iter().copied());
        }
    }

    Some(ProcStats { cpu, rss_bytes })
}

/// Machine-wide CPU, memory, and swap, plus the platform's own saturation signal: macOS reports the
/// kernel's memory pressure level, Linux reports load average. Windows has neither, so both are None.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStats {
    /// Whole-machine CPU usage normalized to 0-100 regardless of core count, unlike `ProcStats::cpu`.
    pub cpu: f32,
    /// Logical core count, used by the UI to judge whether a load average is high.
    pub cores: usize,
    pub mem_used: u64,
    pub mem_total: u64,
    pub swap_used: u64,
    pub swap_total: u64,
    /// macOS only: "normal", "warning", or "critical" from the kernel. None elsewhere.
    pub pressure: Option<&'static str>,
    /// macOS only: memory pressure as 0-100, the same figure `memory_pressure -Q` reports as free
    /// percentage, inverted so higher means more pressure. None elsewhere.
    pub pressure_pct: Option<f32>,
    /// Linux only: 1, 5, and 15 minute load averages. None elsewhere.
    pub load: Option<[f64; 3]>,
}

/// Separate sampler from `sampler()`: system CPU also needs a previous snapshot for its delta, and a
/// dedicated instance keeps the cheap system refresh from waiting on the process-tree scan's lock. The
/// last result is kept alongside it so calls inside `MIN_REFRESH` are answered without touching sysinfo.
struct SysSampler {
    inner: Sampler,
    last: Option<SystemStats>,
}

fn sys_sampler() -> &'static Mutex<SysSampler> {
    static SYS: OnceLock<Mutex<SysSampler>> = OnceLock::new();
    SYS.get_or_init(|| {
        Mutex::new(SysSampler {
            inner: Sampler::new(),
            last: None,
        })
    })
}

/// Read one integer sysctl by name, returning None when the OID is unknown or unreadable.
#[cfg(target_os = "macos")]
fn sysctl_int(name: &[u8]) -> Option<libc::c_int> {
    use std::ffi::CStr;
    // Edition 2021 has no c"..." literals, so callers pass a NUL-terminated byte string.
    let name = CStr::from_bytes_with_nul(name).ok()?;
    let mut value: libc::c_int = 0;
    let mut len = std::mem::size_of::<libc::c_int>();
    // Safety: sysctlbyname writes at most `len` bytes into `value`, which is exactly its size.
    let rc = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            &mut value as *mut libc::c_int as *mut libc::c_void,
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc == 0 {
        Some(value)
    } else {
        None
    }
}

/// Memory pressure on macOS, as the kernel's own level plus a percentage. Free memory is a poor signal
/// there because the kernel compresses and caches aggressively, so both numbers come from the kernel:
/// `kern.memorystatus_vm_pressure_level` is the level Apple's own notifications use, and
/// `kern.memorystatus_level` is the free-memory percentage `memory_pressure -Q` prints, inverted here so
/// that a larger number means more pressure and matches the direction of every other meter in the panel.
#[cfg(target_os = "macos")]
fn memory_pressure() -> (Option<&'static str>, Option<f32>) {
    // Kernel constants: 1 normal, 2 warning, 4 critical.
    let level = sysctl_int(b"kern.memorystatus_vm_pressure_level\0").map(|v| match v {
        2 => "warning",
        4 => "critical",
        _ => "normal",
    });
    let pct = sysctl_int(b"kern.memorystatus_level\0")
        .map(|free| (100 - free.clamp(0, 100)) as f32);
    (level, pct)
}

#[cfg(not(target_os = "macos"))]
fn memory_pressure() -> (Option<&'static str>, Option<f32>) {
    (None, None)
}

/// Load average, which only Linux reports meaningfully. macOS has one too, but its memory pressure
/// level is the signal users actually read there, and Windows has no load average at all.
#[cfg(target_os = "linux")]
fn load_average() -> Option<[f64; 3]> {
    let la = System::load_average();
    Some([la.one, la.five, la.fifteen])
}

#[cfg(not(target_os = "linux"))]
fn load_average() -> Option<[f64; 3]> {
    None
}

/// Sample whole-machine CPU, memory, and swap. Only CPU and memory are refreshed, so this never scans
/// the process table; like `subtree_stats`, the first call after startup can report 0% CPU.
pub fn system_stats() -> SystemStats {
    let mut guard = sys_sampler().lock().unwrap_or_else(|e| e.into_inner());
    if guard.inner.fresh() {
        if let Some(last) = &guard.last {
            return last.clone();
        }
    }
    let (pressure, pressure_pct) = memory_pressure();
    let sys = &mut guard.inner.sys;
    sys.refresh_cpu_usage();
    sys.refresh_memory();
    let out = SystemStats {
        cpu: sys.global_cpu_usage(),
        cores: sys.cpus().len(),
        mem_used: sys.used_memory(),
        mem_total: sys.total_memory(),
        swap_used: sys.used_swap(),
        swap_total: sys.total_swap(),
        pressure,
        pressure_pct,
        load: load_average(),
    };
    guard.inner.mark_refreshed();
    guard.last = Some(out.clone());
    out
}

