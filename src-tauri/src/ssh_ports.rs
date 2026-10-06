//! User port forwarding for SSH remote windows: remote port detection, per-session forward registry, and
//! the `vlx://ports-request` handler. The remote window cannot invoke local commands, so every request
//! is answered with a full `ssh://ports-state` snapshot instead of a reply.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Mutex, MutexGuard, OnceLock};

use crate::ssh_remote::SshTransport;

/// Most user forwards one SSH session may hold.
pub const MAX_FORWARDS: usize = 20;

// Stable codes the frontend maps to localized text (`mapBackendError`); detail follows the first colon.
const ERR_INVALID_PORT: &str = "ports_invalid_port";
const ERR_MISSING_PORT: &str = "ports_missing_port";
const ERR_UNKNOWN_ACTION: &str = "ports_unknown_action";
const ERR_LIMIT: &str = "ports_limit";
const ERR_NOT_FORWARDED: &str = "ports_not_forwarded";

fn limit_error() -> String {
    format!("{ERR_LIMIT}:{MAX_FORWARDS}")
}

/// `/proc/net/tcp{,6}` local addresses for 127.0.0.1, 0.0.0.0, ::1 and ::. The kernel prints each 32-bit
/// word in host byte order, so 127.0.0.1 reads `0100007F` on the little-endian hosts vela-server ships for.
const LOOPBACK_OR_ANY: [&str; 4] = [
    "0100007F",
    "00000000",
    "00000000000000000000000001000000",
    "00000000000000000000000000000000",
];

/// Ports in LISTEN state (`0A`) on loopback or any address, sorted and deduplicated across IPv4/IPv6,
/// skipping sockets whose inode is in `exclude_inodes`.
pub fn parse_listening_ports(raw: &str, exclude_inodes: &HashSet<u64>) -> Vec<u16> {
    let mut ports: Vec<u16> = parse_listening_sockets(raw)
        .into_iter()
        .filter(|(_, inode)| !exclude_inodes.contains(inode))
        .map(|(port, _)| port)
        .collect();
    ports.sort_unstable();
    ports.dedup();
    ports
}

/// `(port, socket inode)` for every LISTEN row on loopback or any address.
fn parse_listening_sockets(raw: &str) -> Vec<(u16, u64)> {
    raw.lines()
        .filter_map(|line| {
            let mut cols = line.split_whitespace();
            cols.next()?;
            let local = cols.next()?;
            cols.next()?;
            if cols.next()? != "0A" {
                return None;
            }
            let (addr, port) = local.split_once(':')?;
            if !LOOPBACK_OR_ANY.contains(&addr) {
                return None;
            }
            // After the state: tx:rx, tr:tm, retrnsmt, uid, timeout, inode.
            let inode = cols.nth(5)?.parse().ok()?;
            Some((u16::from_str_radix(port, 16).ok()?, inode))
        })
        .collect()
}

/// Inodes of the sockets in an `ls -l /proc/<pid>/fd` listing (`... 7 -> socket:[12345]`); pipes, files and
/// anon inodes are ignored.
pub fn parse_socket_inodes(listing: &str) -> HashSet<u64> {
    listing
        .lines()
        .filter_map(|line| {
            let (_, target) = line.split_once(" -> socket:[")?;
            target.trim_end().strip_suffix(']')?.parse().ok()
        })
        .collect()
}

/// Ports worth offering: unprivileged, not the vela-server tunnel target, not already forwarded.
pub fn filter_detected(listening: &[u16], service_rport: Option<u16>, forwarded: &[u16]) -> Vec<u16> {
    listening
        .iter()
        .copied()
        .filter(|p| *p >= 1024 && Some(*p) != service_rport && !forwarded.contains(p))
        .collect()
}

/// Raw `vlx://ports-request` payload. `rport` is wide so out-of-range numbers reach validation instead of
/// failing deserialization silently.
#[derive(Debug, serde::Deserialize)]
pub struct PortsRequest {
    pub session: String,
    pub action: String,
    pub rport: Option<i64>,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum PortsAction {
    List,
    Forward(u16),
    Unforward(u16),
    Open(u16),
}

pub fn parse_action(req: &PortsRequest) -> Result<PortsAction, String> {
    let port = || match req.rport {
        Some(p) if (1..=65535).contains(&p) => Ok(p as u16),
        Some(p) => Err(format!("{ERR_INVALID_PORT}:{p}")),
        None => Err(ERR_MISSING_PORT.to_string()),
    };
    match req.action.as_str() {
        "list" => Ok(PortsAction::List),
        "forward" => port().map(PortsAction::Forward),
        "unforward" => port().map(PortsAction::Unforward),
        "open" => port().map(PortsAction::Open),
        other => Err(format!("{ERR_UNKNOWN_ACTION}:{other}")),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ForwardSource {
    Manual,
    Detected,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Forward {
    pub rport: u16,
    pub lport: u16,
    pub source: ForwardSource,
}

/// Full `ssh://ports-state` payload; the last snapshot a window receives wins.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PortsSnapshot {
    pub session: String,
    pub detected: Vec<u16>,
    pub forwards: Vec<Forward>,
    pub detection_available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// One SSH session's user forwards plus the last detection result.
#[derive(Default)]
pub struct ForwardRegistry {
    service_rport: Option<u16>,
    service_pid: Option<u32>,
    forwards: BTreeMap<u16, Forward>,
    listening: Vec<u16>,
    detection_available: bool,
    detecting: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum InsertOutcome {
    Inserted,
    Duplicate,
    Full,
}

impl ForwardRegistry {
    pub fn lport_of(&self, rport: u16) -> Option<u16> {
        self.forwards.get(&rport).map(|f| f.lport)
    }

    pub fn check_capacity(&self) -> Result<(), String> {
        if self.forwards.len() >= MAX_FORWARDS {
            return Err(limit_error());
        }
        Ok(())
    }

    pub fn source_for(&self, rport: u16) -> ForwardSource {
        if self.detected().contains(&rport) {
            ForwardSource::Detected
        } else {
            ForwardSource::Manual
        }
    }

    /// Returns false, keeping the existing entry, when the remote port is already forwarded.
    pub fn insert(&mut self, forward: Forward) -> bool {
        if self.forwards.contains_key(&forward.rport) {
            return false;
        }
        self.forwards.insert(forward.rport, forward);
        true
    }

    /// Atomic check-and-insert for a forward opened after the pre-open capacity check, which a concurrent
    /// request may have outdated.
    pub fn insert_within_cap(&mut self, forward: Forward) -> InsertOutcome {
        if self.forwards.contains_key(&forward.rport) {
            return InsertOutcome::Duplicate;
        }
        if self.check_capacity().is_err() {
            return InsertOutcome::Full;
        }
        self.insert(forward);
        InsertOutcome::Inserted
    }

    pub fn remove(&mut self, rport: u16) -> Option<Forward> {
        self.forwards.remove(&rport)
    }

    /// `None` means detection failed or the host has no `/proc/net/tcp`.
    pub fn set_listening(&mut self, listening: Option<Vec<u16>>) {
        self.detection_available = listening.is_some();
        self.listening = listening.unwrap_or_default();
    }

    fn detected(&self) -> Vec<u16> {
        let forwarded: Vec<u16> = self.forwards.keys().copied().collect();
        filter_detected(&self.listening, self.service_rport, &forwarded)
    }

    pub fn snapshot(&self, session: &str, error: Option<String>) -> PortsSnapshot {
        PortsSnapshot {
            session: session.to_string(),
            detected: self.detected(),
            forwards: self.forwards.values().cloned().collect(),
            detection_available: self.detection_available,
            error,
        }
    }
}

static REGISTRIES: OnceLock<Mutex<HashMap<String, ForwardRegistry>>> = OnceLock::new();

/// Registry data is plain, so a poisoned lock is safe to recover and keeps disconnect cleanup working.
fn registries() -> MutexGuard<'static, HashMap<String, ForwardRegistry>> {
    REGISTRIES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

fn with_registry<R>(session: &str, f: impl FnOnce(&mut ForwardRegistry) -> R) -> R {
    f(registries().entry(session.to_string()).or_default())
}

/// Record the remote port the vela-server tunnel targets, and its PID when known, so detection never
/// offers vela-server's own listeners.
pub fn set_service_port(session: &str, rport: u16, pid: Option<u32>) {
    with_registry(session, |r| {
        r.service_rport = Some(rport);
        r.service_pid = pid;
    });
}

/// Drop a session's registry and return the local ports of its forwards so the caller can close them.
pub fn forget_session(session: &str) -> Vec<u16> {
    registries()
        .remove(session)
        .map(|r| r.forwards.values().map(|f| f.lport).collect())
        .unwrap_or_default()
}

/// `2>/dev/null; true` keeps hosts without IPv6 (no tcp6 file) from failing the whole command; output
/// without the `local_address` header (macOS, PowerShell) means detection is unavailable.
const PROC_NET_CMD: &str = "cat /proc/net/tcp /proc/net/tcp6 2>/dev/null; true";

const FD_MARKER: &str = "---vela-fds---";

/// With a known service PID the same exec also lists that process's fds, so its own listeners (e.g. the
/// hook server on a random port) can be told apart from the user's.
fn proc_net_cmd(service_pid: Option<u32>) -> String {
    match service_pid {
        Some(pid) => format!(
            "cat /proc/net/tcp /proc/net/tcp6 2>/dev/null; echo {FD_MARKER}; ls -l /proc/{pid}/fd 2>/dev/null; true"
        ),
        None => PROC_NET_CMD.to_string(),
    }
}

/// Exact PID, not its descendants: remote terminals and the user's dev servers are children of vela-server
/// and must stay visible. An empty or unreadable fd listing excludes nothing.
fn parse_detection(out: &str) -> Vec<u16> {
    let (tcp, fds) = out.split_once(FD_MARKER).unwrap_or((out, ""));
    parse_listening_ports(tcp, &parse_socket_inodes(fds))
}

pub struct PortsOutcome {
    pub snapshot: PortsSnapshot,
    /// Local port of a forward to open in the browser, set only for an `open` of an existing forward.
    pub open_lport: Option<u16>,
}

/// Run one request and return the snapshot to emit. Unknown sessions return None: the window is closing
/// or the request did not come from a live SSH window. Blocks on SSH I/O; never holds the registry lock
/// across it.
pub fn handle(req: PortsRequest) -> Option<PortsOutcome> {
    let t = crate::ssh_remote::transport(&req.session)?;
    let session = req.session.as_str();
    let mut open_lport = None;
    let error = match parse_action(&req) {
        Err(e) => Some(e),
        Ok(PortsAction::List) => {
            refresh_detection(session, t.as_ref());
            None
        }
        Ok(PortsAction::Forward(rport)) => match forward(session, t.as_ref(), rport, &|| {
            crate::ssh_remote::transport(session).is_some()
        }) {
            Ok(true) => None,
            Ok(false) => return None,
            Err(e) => Some(e),
        },
        Ok(PortsAction::Unforward(rport)) => unforward(session, t.as_ref(), rport).err(),
        Ok(PortsAction::Open(rport)) => match open_port(session, rport) {
            Ok(lport) => {
                open_lport = Some(lport);
                None
            }
            Err(e) => Some(e),
        },
    };
    crate::ssh_remote::transport(session)?;
    let snapshot = with_registry(session, |r| r.snapshot(session, error));
    Some(PortsOutcome { snapshot, open_lport })
}

/// The frontend polls every few seconds; a slow host must not stack concurrent detections.
fn refresh_detection(session: &str, t: &dyn SshTransport) {
    let (busy, pid) = with_registry(session, |r| (std::mem::replace(&mut r.detecting, true), r.service_pid));
    if busy {
        return;
    }
    struct ClearDetecting<'a>(&'a str);
    impl Drop for ClearDetecting<'_> {
        fn drop(&mut self) {
            with_registry(self.0, |r| r.detecting = false);
        }
    }
    let _clear = ClearDetecting(session);
    let listening = match t.exec(&proc_net_cmd(pid)) {
        Ok(out) if out.contains("local_address") => Some(parse_detection(&out)),
        _ => None,
    };
    with_registry(session, |r| r.set_listening(listening));
}

fn unforward(session: &str, t: &dyn SshTransport, rport: u16) -> Result<(), String> {
    match with_registry(session, |r| r.remove(rport)) {
        Some(f) => t.close_forward(f.lport),
        None => Ok(()),
    }
}

fn open_port(session: &str, rport: u16) -> Result<u16, String> {
    with_registry(session, |r| r.lport_of(rport)).ok_or_else(|| format!("{ERR_NOT_FORWARDED}:{rport}"))
}

/// `Ok(false)` means `alive` reported the session gone while the tunnel was opening; the tunnel is closed again.
fn forward(session: &str, t: &dyn SshTransport, rport: u16, alive: &dyn Fn() -> bool) -> Result<bool, String> {
    let source = with_registry(session, |r| {
        if r.lport_of(rport).is_some() {
            return Ok(None);
        }
        r.check_capacity().map(|_| Some(r.source_for(rport)))
    })?;
    let Some(source) = source else { return Ok(true) };
    let lport = t.open_user_forward(rport, Some(rport))?;
    if !alive() {
        let _ = t.close_forward(lport);
        return Ok(false);
    }
    match with_registry(session, |r| r.insert_within_cap(Forward { rport, lport, source })) {
        InsertOutcome::Inserted => Ok(true),
        InsertOutcome::Duplicate => {
            let _ = t.close_forward(lport);
            Ok(true)
        }
        InsertOutcome::Full => {
            let _ = t.close_forward(lport);
            Err(limit_error())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TCP4: &str = "  sl  local_address rem_address   st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode
   0: 0100007F:0BB8 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 1 1 0000000000000000 100 0 0 10 0
   1: 00000000:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 2 1 0000000000000000 100 0 0 10 0
   2: 0100007F:0BB8 0100007F:D431 01 00000000:00000000 00:00000000 00000000  1000        0 3 1 0000000000000000 20 4 30 10 -1
   3: 0F02000A:1F91 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 4 1 0000000000000000 100 0 0 10 0
   4: 00000000:0016 00000000:0000 0A 00000000:00000000 00:00000000 00000000     0        0 5 1 0000000000000000 100 0 0 10 0";

    const TCP6: &str = "  sl  local_address                         remote_address                        st tx_queue rx_queue tr tm->when retrnsmt   uid  timeout inode
   0: 00000000000000000000000001000000:0BB8 00000000000000000000000000000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 6 1 0000000000000000 100 0 0 10 0
   1: 00000000000000000000000000000000:13C2 00000000000000000000000000000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 7 1 0000000000000000 100 0 0 10 0";

    #[test]
    fn parses_listening_ports_on_loopback_and_any_and_dedupes_families() {
        let raw = format!("{TCP4}\n{TCP6}");
        // 3000 (127.0.0.1 and ::1), 8080 (0.0.0.0), 5058 (::), 22 (0.0.0.0). 8081 is bound to 10.0.2.15
        // and the 3000 row in state 01 is an established connection.
        assert_eq!(parse_listening_ports(&raw, &HashSet::new()), vec![22, 3000, 5058, 8080]);
    }

    #[test]
    fn parse_ignores_output_that_is_not_proc_net_tcp() {
        assert!(parse_listening_ports("", &HashSet::new()).is_empty());
        assert!(parse_listening_ports("cat : Cannot find path '/proc/net/tcp'", &HashSet::new()).is_empty());
    }

    #[test]
    fn parses_port_and_inode_from_tcp_and_tcp6_rows() {
        let raw = format!("{TCP4}\n{TCP6}");
        let mut sockets = parse_listening_sockets(&raw);
        sockets.sort_unstable();
        assert_eq!(sockets, vec![(22, 5), (3000, 1), (3000, 6), (5058, 7), (8080, 2)]);
    }

    #[test]
    fn parses_socket_inodes_and_ignores_other_fd_kinds() {
        let listing = "total 0
lr-x------ 1 u u 64 Oct  6 12:00 0 -> /dev/null
lrwx------ 1 u u 64 Oct  6 12:00 3 -> socket:[34933]
lrwx------ 1 u u 64 Oct  6 12:00 4 -> socket:[34934]
l-wx------ 1 u u 64 Oct  6 12:00 5 -> pipe:[999]
lrwx------ 1 u u 64 Oct  6 12:00 6 -> anon_inode:[eventpoll]
lr-x------ 1 u u 64 Oct  6 12:00 7 -> /home/u/socket:[1]";
        assert_eq!(parse_socket_inodes(listing), HashSet::from([34933, 34934]));
        assert!(parse_socket_inodes("").is_empty());
        assert!(parse_socket_inodes("ls: cannot access '/proc/9/fd': No such file or directory").is_empty());
    }

    #[test]
    fn detection_drops_service_pid_sockets_and_keeps_other_owners() {
        // Inode 2 is the service's hook server (8080 here), inode 1 a user's dev server on 3000.
        let out = format!("{TCP4}\n{TCP6}\n{FD_MARKER}\nlrwx------ 1 u u 64 Oct  6 12:00 3 -> socket:[2]\n");
        assert_eq!(parse_detection(&out), vec![22, 3000, 5058]);
    }

    #[test]
    fn detection_without_fd_section_or_listing_filters_nothing() {
        let raw = format!("{TCP4}\n{TCP6}");
        assert_eq!(parse_detection(&raw), vec![22, 3000, 5058, 8080]);
        assert_eq!(parse_detection(&format!("{raw}\n{FD_MARKER}\n")), vec![22, 3000, 5058, 8080]);
    }

    #[test]
    fn detection_command_includes_the_pid_only_when_known() {
        assert_eq!(proc_net_cmd(None), PROC_NET_CMD);
        let cmd = proc_net_cmd(Some(4242));
        assert!(cmd.contains("ls -l /proc/4242/fd") && cmd.contains(FD_MARKER));
        assert!(!PROC_NET_CMD.contains("/fd"));
    }

    #[test]
    fn filter_drops_privileged_service_and_forwarded_ports() {
        let listening = [22, 3000, 5058, 8080, 41234];
        assert_eq!(filter_detected(&listening, Some(41234), &[8080]), vec![3000, 5058]);
        assert_eq!(filter_detected(&listening, None, &[]), vec![3000, 5058, 8080, 41234]);
    }

    fn req(action: &str, rport: Option<i64>) -> PortsRequest {
        PortsRequest { session: "s".into(), action: action.into(), rport }
    }

    #[test]
    fn parse_action_validates_actions_and_ports() {
        assert_eq!(parse_action(&req("list", None)), Ok(PortsAction::List));
        assert_eq!(parse_action(&req("forward", Some(3000))), Ok(PortsAction::Forward(3000)));
        assert_eq!(parse_action(&req("unforward", Some(1))), Ok(PortsAction::Unforward(1)));
        assert_eq!(parse_action(&req("open", Some(65535))), Ok(PortsAction::Open(65535)));
        assert_eq!(parse_action(&req("forward", None)), Err("ports_missing_port".into()));
        assert_eq!(parse_action(&req("forward", Some(0))), Err("ports_invalid_port:0".into()));
        assert_eq!(parse_action(&req("forward", Some(70000))), Err("ports_invalid_port:70000".into()));
        assert_eq!(parse_action(&req("forward", Some(-1))), Err("ports_invalid_port:-1".into()));
        assert_eq!(parse_action(&req("delete", Some(3000))), Err("ports_unknown_action:delete".into()));
    }

    #[test]
    fn request_payload_deserializes_with_and_without_port() {
        let r: PortsRequest =
            serde_json::from_str(r#"{"session":"abc","action":"forward","rport":3000}"#).unwrap();
        assert_eq!((r.session.as_str(), r.action.as_str(), r.rport), ("abc", "forward", Some(3000)));
        let r: PortsRequest = serde_json::from_str(r#"{"session":"abc","action":"list"}"#).unwrap();
        assert_eq!(r.rport, None);
    }

    fn fwd(rport: u16, lport: u16, source: ForwardSource) -> Forward {
        Forward { rport, lport, source }
    }

    #[test]
    fn registry_adds_removes_and_refuses_duplicates() {
        let mut reg = ForwardRegistry::default();
        assert!(reg.insert(fwd(3000, 3000, ForwardSource::Detected)));
        assert!(!reg.insert(fwd(3000, 3001, ForwardSource::Manual)));
        assert_eq!(reg.lport_of(3000), Some(3000));
        assert_eq!(reg.remove(3000), Some(fwd(3000, 3000, ForwardSource::Detected)));
        assert_eq!(reg.remove(3000), None);
        assert_eq!(reg.lport_of(3000), None);
    }

    #[test]
    fn registry_caps_forwards_per_session() {
        let mut reg = ForwardRegistry::default();
        for p in 0..MAX_FORWARDS as u16 {
            assert!(reg.check_capacity().is_ok());
            reg.insert(fwd(5000 + p, 5000 + p, ForwardSource::Manual));
        }
        assert!(reg.check_capacity().unwrap_err().contains("20"));
    }

    #[test]
    fn snapshot_filters_detected_and_reports_forwards() {
        let mut reg = ForwardRegistry::default();
        reg.service_rport = Some(41234);
        reg.set_listening(Some(vec![22, 3000, 8080, 41234]));
        assert_eq!(reg.source_for(3000), ForwardSource::Detected);
        assert_eq!(reg.source_for(9999), ForwardSource::Manual);
        reg.insert(fwd(8080, 18080, ForwardSource::Detected));
        let snap = reg.snapshot("s1", Some("boom".into()));
        assert_eq!(snap.detected, vec![3000]);
        assert_eq!(snap.forwards, vec![fwd(8080, 18080, ForwardSource::Detected)]);
        assert!(snap.detection_available);
        assert_eq!(snap.error.as_deref(), Some("boom"));
    }

    #[test]
    fn failed_detection_marks_unavailable_and_clears_ports() {
        let mut reg = ForwardRegistry::default();
        reg.set_listening(Some(vec![3000]));
        reg.set_listening(None);
        let snap = reg.snapshot("s1", None);
        assert!(!snap.detection_available);
        assert!(snap.detected.is_empty());
    }

    #[test]
    fn snapshot_serializes_to_the_wire_shape() {
        let mut reg = ForwardRegistry::default();
        reg.set_listening(Some(vec![3000]));
        reg.insert(fwd(5432, 15432, ForwardSource::Manual));
        let json = serde_json::to_value(reg.snapshot("s1", None)).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "session": "s1",
                "detected": [3000],
                "forwards": [{ "rport": 5432, "lport": 15432, "source": "manual" }],
                "detectionAvailable": true
            })
        );
    }

    #[test]
    fn forget_session_returns_its_local_ports_and_keeps_other_sessions() {
        set_service_port("forget-a", 41000, Some(77));
        with_registry("forget-a", |r| {
            r.insert(fwd(3000, 3000, ForwardSource::Manual));
            r.insert(fwd(8080, 18080, ForwardSource::Detected));
        });
        with_registry("forget-b", |r| r.insert(fwd(5000, 5000, ForwardSource::Manual)));
        let mut freed = forget_session("forget-a");
        freed.sort_unstable();
        assert_eq!(freed, vec![3000, 18080]);
        assert!(forget_session("forget-a").is_empty());
        assert_eq!(with_registry("forget-b", |r| r.lport_of(5000)), Some(5000));
        forget_session("forget-b");
    }

    #[test]
    fn requests_for_unknown_sessions_are_ignored() {
        let session = "ports-unknown-session";
        let out = handle(PortsRequest { session: session.into(), action: "forward".into(), rport: Some(3000) });
        assert!(out.is_none());
        assert!(!registries().contains_key(session));
    }

    type Hook = Box<dyn Fn() + Send + Sync>;

    #[derive(Default)]
    struct FakeTransport {
        exec_out: Mutex<Option<Result<String, String>>>,
        exec_calls: Mutex<u32>,
        exec_cmds: Mutex<Vec<String>>,
        on_exec: Mutex<Option<Hook>>,
        open_result: Mutex<Option<Result<u16, String>>>,
        open_calls: Mutex<Vec<(u16, Option<u16>)>>,
        on_open: Mutex<Option<Hook>>,
        close_result: Mutex<Option<Result<(), String>>>,
        closed: Mutex<Vec<u16>>,
    }

    impl SshTransport for FakeTransport {
        fn exec(&self, cmd: &str) -> Result<String, String> {
            self.exec_cmds.lock().unwrap().push(cmd.to_string());
            *self.exec_calls.lock().unwrap() += 1;
            if let Some(h) = self.on_exec.lock().unwrap().as_ref() {
                h();
            }
            self.exec_out.lock().unwrap().clone().unwrap_or(Err("no exec result".into()))
        }
        fn exec_input(&self, _cmd: &str, _input: &str) -> Result<String, String> {
            unreachable!()
        }
        fn upload(&self, _l: &std::path::Path, _r: &str, _p: crate::ssh_remote::Progress) -> Result<(), String> {
            unreachable!()
        }
        fn open_forward(&self, _rport: u16) -> Result<u16, String> {
            unreachable!()
        }
        fn open_user_forward(&self, rport: u16, preferred_lport: Option<u16>) -> Result<u16, String> {
            self.open_calls.lock().unwrap().push((rport, preferred_lport));
            if let Some(h) = self.on_open.lock().unwrap().as_ref() {
                h();
            }
            self.open_result.lock().unwrap().clone().unwrap_or(Ok(rport))
        }
        fn close_forward(&self, lport: u16) -> Result<(), String> {
            self.closed.lock().unwrap().push(lport);
            self.close_result.lock().unwrap().clone().unwrap_or(Ok(()))
        }
        fn close(&self) {}
        fn session(&self) -> &str {
            "fake"
        }
    }

    fn snap(session: &str) -> PortsSnapshot {
        with_registry(session, |r| r.snapshot(session, None))
    }

    fn fill_registry(session: &str) {
        with_registry(session, |r| {
            for p in 0..MAX_FORWARDS as u16 {
                r.insert(fwd(3000 + p, 3000 + p, ForwardSource::Manual));
            }
        });
    }

    const LIVE: &dyn Fn() -> bool = &|| true;

    const PROC_OK: &str = "  sl  local_address rem_address   st\n   0: 0100007F:1F90 00000000:0000 0A 00000000:00000000 00:00000000 00000000  1000        0 2 1 0\n";

    #[test]
    fn refresh_detection_resets_guard_after_ok_and_err() {
        let s = "ports-detect-reset";
        let t = FakeTransport::default();
        *t.exec_out.lock().unwrap() = Some(Ok(PROC_OK.into()));
        refresh_detection(s, &t);
        let sn = snap(s);
        assert!(sn.detection_available);
        assert_eq!(sn.detected, vec![8080]);
        assert!(!with_registry(s, |r| r.detecting));

        *t.exec_out.lock().unwrap() = Some(Err("boom".into()));
        refresh_detection(s, &t);
        let sn = snap(s);
        assert!(!sn.detection_available);
        assert!(sn.detected.is_empty());
        assert!(!with_registry(s, |r| r.detecting));

        refresh_detection(s, &t);
        assert_eq!(*t.exec_calls.lock().unwrap(), 3);
        forget_session(s);
    }

    #[test]
    fn refresh_detection_uses_the_recorded_service_pid() {
        let s = "ports-detect-pid";
        let fds = "lrwx------ 1 u u 64 Oct  6 12:00 3 -> socket:[2]\n";
        let t = FakeTransport::default();
        *t.exec_out.lock().unwrap() = Some(Ok(PROC_OK.into()));
        refresh_detection(s, &t);
        assert_eq!(t.exec_cmds.lock().unwrap()[0], PROC_NET_CMD);
        assert_eq!(snap(s).detected, vec![8080]);

        set_service_port(s, 41000, Some(321));
        *t.exec_out.lock().unwrap() = Some(Ok(format!("{PROC_OK}{FD_MARKER}\n{fds}")));
        refresh_detection(s, &t);
        assert!(t.exec_cmds.lock().unwrap()[1].contains("/proc/321/fd"));
        assert!(snap(s).detected.is_empty());
        forget_session(s);
    }

    #[test]
    fn refresh_detection_clears_guard_when_exec_panics() {
        let s = "ports-detect-panic";
        let t = FakeTransport::default();
        *t.exec_out.lock().unwrap() = Some(Ok(PROC_OK.into()));
        *t.on_exec.lock().unwrap() = Some(Box::new(|| panic!("exec exploded")));
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| refresh_detection(s, &t)));
        assert!(panicked.is_err());
        assert!(!with_registry(s, |r| r.detecting));

        t.on_exec.clear_poison();
        *t.on_exec.lock().unwrap() = None;
        refresh_detection(s, &t);
        assert_eq!(*t.exec_calls.lock().unwrap(), 2);
        forget_session(s);
    }

    #[test]
    fn refresh_detection_without_header_marks_unavailable() {
        let s = "ports-detect-noheader";
        let t = FakeTransport::default();
        *t.exec_out.lock().unwrap() = Some(Ok("Get-NetTCPConnection: not found".into()));
        refresh_detection(s, &t);
        assert!(!snap(s).detection_available);
        forget_session(s);
    }

    #[test]
    fn refresh_detection_skips_exec_while_one_is_in_flight() {
        let s = "ports-detect-guard";
        let t = FakeTransport::default();
        *t.exec_out.lock().unwrap() = Some(Ok(PROC_OK.into()));
        let inner = std::sync::Arc::new(FakeTransport::default());
        let inner_hook = inner.clone();
        *t.on_exec.lock().unwrap() = Some(Box::new(move || refresh_detection(s, inner_hook.as_ref())));
        refresh_detection(s, &t);
        assert_eq!(*t.exec_calls.lock().unwrap(), 1);
        assert_eq!(*inner.exec_calls.lock().unwrap(), 0);
        assert!(snap(s).detection_available);
        forget_session(s);
    }

    #[test]
    fn forward_new_port_opens_once_and_inserts() {
        let s = "ports-fwd-new";
        let t = FakeTransport::default();
        assert_eq!(forward(s, &t, 3000, LIVE), Ok(true));
        assert_eq!(*t.open_calls.lock().unwrap(), vec![(3000, Some(3000))]);
        assert_eq!(snap(s).forwards, vec![fwd(3000, 3000, ForwardSource::Manual)]);
        forget_session(s);
    }

    #[test]
    fn forward_marks_detected_source() {
        let s = "ports-fwd-detected";
        with_registry(s, |r| r.set_listening(Some(vec![8080])));
        let t = FakeTransport::default();
        assert_eq!(forward(s, &t, 8080, LIVE), Ok(true));
        assert_eq!(snap(s).forwards[0].source, ForwardSource::Detected);
        forget_session(s);
    }

    #[test]
    fn forward_already_forwarded_is_idempotent() {
        let s = "ports-fwd-idem";
        let t = FakeTransport::default();
        assert_eq!(forward(s, &t, 3000, LIVE), Ok(true));
        assert_eq!(forward(s, &t, 3000, LIVE), Ok(true));
        assert_eq!(t.open_calls.lock().unwrap().len(), 1);
        assert!(t.closed.lock().unwrap().is_empty());
        assert_eq!(snap(s).forwards.len(), 1);
        forget_session(s);
    }

    #[test]
    fn forward_duplicate_after_open_closes_new_tunnel_and_keeps_first() {
        let s = "ports-fwd-dup-race";
        let t = FakeTransport::default();
        *t.open_result.lock().unwrap() = Some(Ok(4001));
        *t.on_open.lock().unwrap() = Some(Box::new(move || {
            with_registry(s, |r| r.insert(fwd(3000, 3000, ForwardSource::Manual)));
        }));
        *t.close_result.lock().unwrap() = Some(Err("close failed".into()));
        assert_eq!(forward(s, &t, 3000, LIVE), Ok(true));
        assert_eq!(*t.closed.lock().unwrap(), vec![4001]);
        assert_eq!(snap(s).forwards, vec![fwd(3000, 3000, ForwardSource::Manual)]);
        forget_session(s);
    }

    #[test]
    fn forward_full_after_open_closes_tunnel_and_reports_cap() {
        let s = "ports-fwd-full-race";
        let t = FakeTransport::default();
        *t.open_result.lock().unwrap() = Some(Ok(9001));
        *t.on_open.lock().unwrap() = Some(Box::new(move || fill_registry(s)));
        *t.close_result.lock().unwrap() = Some(Err("close failed".into()));
        let err = forward(s, &t, 9000, LIVE).unwrap_err();
        assert_eq!(err, "ports_limit:20");
        assert_eq!(*t.closed.lock().unwrap(), vec![9001]);
        let sn = snap(s);
        assert_eq!(sn.forwards.len(), MAX_FORWARDS);
        assert!(sn.forwards.iter().all(|f| f.rport != 9000));
        forget_session(s);
    }

    #[test]
    fn forward_at_cap_does_not_open() {
        let s = "ports-fwd-cap-pre";
        fill_registry(s);
        let t = FakeTransport::default();
        assert!(forward(s, &t, 9000, LIVE).is_err());
        assert!(t.open_calls.lock().unwrap().is_empty());
        forget_session(s);
    }

    #[test]
    fn forward_open_error_leaves_registry_unchanged() {
        let s = "ports-fwd-open-err";
        let t = FakeTransport::default();
        *t.open_result.lock().unwrap() = Some(Err("refused".into()));
        assert_eq!(forward(s, &t, 3000, LIVE), Err("refused".into()));
        assert!(snap(s).forwards.is_empty());
        assert!(t.closed.lock().unwrap().is_empty());
        forget_session(s);
    }

    #[test]
    fn forward_closes_tunnel_when_session_disconnected_meanwhile() {
        let s = "ports-fwd-gone";
        let t = FakeTransport::default();
        *t.open_result.lock().unwrap() = Some(Ok(3000));
        assert_eq!(forward(s, &t, 3000, &|| false), Ok(false));
        assert_eq!(*t.closed.lock().unwrap(), vec![3000]);
        assert!(snap(s).forwards.is_empty());
        forget_session(s);
    }

    #[test]
    fn unforward_removes_and_closes_local_port() {
        let s = "ports-unfwd";
        let t = FakeTransport::default();
        with_registry(s, |r| r.insert(fwd(3000, 4000, ForwardSource::Manual)));
        assert_eq!(unforward(s, &t, 3000), Ok(()));
        assert_eq!(*t.closed.lock().unwrap(), vec![4000]);
        assert!(snap(s).forwards.is_empty());
        forget_session(s);
    }

    #[test]
    fn unforward_unknown_port_is_a_noop() {
        let s = "ports-unfwd-none";
        let t = FakeTransport::default();
        assert_eq!(unforward(s, &t, 3000), Ok(()));
        assert!(t.closed.lock().unwrap().is_empty());
        forget_session(s);
    }

    #[test]
    fn unforward_reports_close_error_but_still_removes() {
        let s = "ports-unfwd-err";
        let t = FakeTransport::default();
        *t.close_result.lock().unwrap() = Some(Err("close failed".into()));
        with_registry(s, |r| r.insert(fwd(3000, 3000, ForwardSource::Manual)));
        assert_eq!(unforward(s, &t, 3000), Err("close failed".into()));
        assert!(snap(s).forwards.is_empty());
        forget_session(s);
    }

    #[test]
    fn open_port_returns_lport_or_not_forwarded_error() {
        let s = "ports-open";
        with_registry(s, |r| r.insert(fwd(3000, 4000, ForwardSource::Manual)));
        assert_eq!(open_port(s, 3000), Ok(4000));
        assert_eq!(open_port(s, 3001), Err("ports_not_forwarded:3001".into()));
        forget_session(s);
    }

    fn manual(rport: u16) -> Forward {
        fwd(rport, rport, ForwardSource::Manual)
    }

    #[test]
    fn insert_within_cap_keeps_first_duplicate() {
        let mut r = ForwardRegistry::default();
        assert_eq!(r.insert_within_cap(manual(3000)), InsertOutcome::Inserted);
        assert_eq!(r.insert_within_cap(Forward { lport: 4000, ..manual(3000) }), InsertOutcome::Duplicate);
        assert_eq!(r.lport_of(3000), Some(3000));
    }

    #[test]
    fn insert_within_cap_rejects_past_the_limit() {
        let mut r = ForwardRegistry::default();
        for p in 0..MAX_FORWARDS as u16 {
            assert_eq!(r.insert_within_cap(manual(3000 + p)), InsertOutcome::Inserted);
        }
        assert_eq!(r.insert_within_cap(manual(9000)), InsertOutcome::Full);
        assert_eq!(r.insert_within_cap(manual(3000)), InsertOutcome::Duplicate);
        assert!(r.lport_of(9000).is_none());
    }
}
