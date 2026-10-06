//! User port forwarding for SSH remote windows: remote port detection, per-session forward registry, and
//! the `vlx://ports-request` handler. The remote window cannot invoke local commands, so every request
//! is answered with a full `ssh://ports-state` snapshot instead of a reply.

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
}
