//! User port forwarding for SSH remote windows: remote port detection, per-session forward registry, and
//! the `vlx://ports-request` handler. The remote window cannot invoke local commands, so every request
//! is answered with a full `ssh://ports-state` snapshot instead of a reply.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, MutexGuard, OnceLock};

/// Most user forwards one SSH session may hold.
pub const MAX_FORWARDS: usize = 20;

/// `/proc/net/tcp{,6}` local addresses for 127.0.0.1, 0.0.0.0, ::1 and ::. The kernel prints each 32-bit
/// word in host byte order, so 127.0.0.1 reads `0100007F` on the little-endian hosts vela-server ships for.
const LOOPBACK_OR_ANY: [&str; 4] = [
    "0100007F",
    "00000000",
    "00000000000000000000000001000000",
    "00000000000000000000000000000000",
];

/// Ports in LISTEN state (`0A`) on loopback or any address, sorted and deduplicated across IPv4/IPv6.
pub fn parse_listening_ports(raw: &str) -> Vec<u16> {
    let mut ports: Vec<u16> = raw
        .lines()
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
            u16::from_str_radix(port, 16).ok()
        })
        .collect();
    ports.sort_unstable();
    ports.dedup();
    ports
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
        Some(p) => Err(format!("invalid port: {p}")),
        None => Err("missing port".to_string()),
    };
    match req.action.as_str() {
        "list" => Ok(PortsAction::List),
        "forward" => port().map(PortsAction::Forward),
        "unforward" => port().map(PortsAction::Unforward),
        "open" => port().map(PortsAction::Open),
        other => Err(format!("unknown action: {other}")),
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
    forwards: BTreeMap<u16, Forward>,
    listening: Vec<u16>,
    detection_available: bool,
}

impl ForwardRegistry {
    pub fn lport_of(&self, rport: u16) -> Option<u16> {
        self.forwards.get(&rport).map(|f| f.lport)
    }

    pub fn check_capacity(&self) -> Result<(), String> {
        if self.forwards.len() >= MAX_FORWARDS {
            return Err(format!("at most {MAX_FORWARDS} ports can be forwarded per connection"));
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

/// Record the remote port the vela-server tunnel targets so detection never offers it.
pub fn set_service_port(session: &str, rport: u16) {
    with_registry(session, |r| r.service_rport = Some(rport));
}

/// Drop a session's registry and return the local ports of its forwards so the caller can close them.
pub fn forget_session(session: &str) -> Vec<u16> {
    registries()
        .remove(session)
        .map(|r| r.forwards.values().map(|f| f.lport).collect())
        .unwrap_or_default()
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
        assert_eq!(parse_listening_ports(&raw), vec![22, 3000, 5058, 8080]);
    }

    #[test]
    fn parse_ignores_output_that_is_not_proc_net_tcp() {
        assert!(parse_listening_ports("").is_empty());
        assert!(parse_listening_ports("cat : Cannot find path '/proc/net/tcp'").is_empty());
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
        assert!(parse_action(&req("forward", None)).is_err());
        assert!(parse_action(&req("forward", Some(0))).is_err());
        assert!(parse_action(&req("forward", Some(70000))).is_err());
        assert!(parse_action(&req("forward", Some(-1))).is_err());
        assert!(parse_action(&req("delete", Some(3000))).is_err());
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
        set_service_port("forget-a", 41000);
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
}
