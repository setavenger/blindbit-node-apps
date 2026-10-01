//! Classify friglet failures into UI failure classes.
//!
//! At the friglet pin `GetStatus.last_error` is never filled (the scan loop logs and retries
//! or panics), so classification reads, in order: `last_error` when present, then the raw
//! log ring (stdout logs, stderr `Error:` lines and panic messages), then the exit status.
//! The strings matched here are the ones friglet/blindbit-lib/bitcoin-p2p print at the pin
//! (see the golden lines in the tests).

use crate::states::Failure;

/// How friglet ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitKind {
    Code(i32),
    Signal(i32),
    /// friglet-ui killed it because the control socket never came up.
    StartupDeadline,
}

/// Classify a process exit from its status and the last raw log lines (oldest first).
pub fn classify_exit(exit: ExitKind, lines: &[String]) -> Failure {
    if let Some(port) = port_in_use(lines) {
        let _ = port;
        return Failure::PortInUse;
    }
    if exit == ExitKind::StartupDeadline {
        return Failure::OracleUnreachable;
    }
    let recent = |needle: &str| lines.iter().rev().take(60).any(|l| l.contains(needle));
    match exit {
        ExitKind::Code(1) if recent("Error: transport error") => Failure::OracleUnreachable,
        ExitKind::Code(1) if recent("Error: Oracle URL") => Failure::Unknown,
        ExitKind::Signal(6) | ExitKind::Code(134) => {
            if recent("panicked at") && (recent("scanning.rs") || recent("Status {")) {
                Failure::OracleDropped
            } else {
                Failure::Unknown
            }
        }
        _ => Failure::Unknown,
    }
}

/// Which of friglet's own listeners failed to bind, when the log says so.
pub fn port_in_use(lines: &[String]) -> Option<&'static str> {
    for l in lines.iter().rev().take(80) {
        let addr_in_use = l.contains("Address already in use") || l.contains("AddrInUse");
        if l.contains("Failed to bind HTTP server") && addr_in_use {
            return Some("friglet HTTP 127.0.0.1:8095");
        }
        if l.contains("Electrum server terminated with error") && addr_in_use {
            return Some("friglet Electrum 127.0.0.1:50001");
        }
        if l.contains("another friglet daemon is already listening") {
            return Some("friglet control socket");
        }
    }
    None
}

/// Kind of scan-time trouble friglet reported in its logs while it keeps running.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogCondition {
    /// `failed to reach oracle, will retry`.
    OracleRetrying,
    /// P2P trouble while fetching blocks; `height` is the block it asked for, when known.
    P2p {
        kind: P2pTrouble,
        height: Option<u64>,
    },
    /// Progress was made: clears any previous condition.
    Progress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum P2pTrouble {
    /// Could not open a connection.
    Connect,
    /// notfound / rejected / peer closed / 90 s timeout: the peer lacks or withholds the block.
    BlockUnavailable,
}

/// Read one raw (un-redacted) log line for a scan-time condition.
pub fn classify_line(line: &str) -> Option<LogCondition> {
    if line.contains("failed to reach oracle") {
        return Some(LogCondition::OracleRetrying);
    }
    if line.contains("scan progress") || line.contains("balance after scan") {
        return Some(LogCondition::Progress);
    }
    let height = field_u64(line, "height=");
    if line.contains("failed to open P2P connection") {
        return Some(LogCondition::P2p {
            kind: P2pTrouble::Connect,
            height,
        });
    }
    let unavailable = [
        "replied notfound for block",
        "rejected getdata for block",
        "peer closed the connection",
        "gave up waiting for block",
        "still waiting for block",
        "block fetch failed",
    ];
    if unavailable.iter().any(|n| line.contains(n)) {
        return Some(LogCondition::P2p {
            kind: P2pTrouble::BlockUnavailable,
            height,
        });
    }
    if line.contains("scan_block_range failed") {
        let kind = if line.contains("Connection refused")
            || line.contains("connection timed out")
            || line.contains("No route to host")
            || line.contains("Network is unreachable")
        {
            P2pTrouble::Connect
        } else {
            P2pTrouble::BlockUnavailable
        };
        return Some(LogCondition::P2p { kind, height });
    }
    None
}

/// Map a P2P condition to a failure using what the handshake knows about the node.
pub fn p2p_failure(
    kind: P2pTrouble,
    height: Option<u64>,
    node_tip: Option<u64>,
    pruned: bool,
) -> Failure {
    match kind {
        P2pTrouble::Connect => Failure::PeerUnreachable,
        P2pTrouble::BlockUnavailable if pruned => Failure::NodePruned,
        P2pTrouble::BlockUnavailable => match (height, node_tip) {
            (Some(h), Some(tip)) if h > tip => Failure::NodeSyncing,
            (_, None) => Failure::NodeSyncing,
            // The node has the height but does not serve the block.
            _ => Failure::PeerUnreachable,
        },
    }
}

/// Classify `GetStatus.last_error` (not filled at the pin; kept for BR4).
pub fn classify_last_error(err: &str) -> Failure {
    let e = err.to_ascii_lowercase();
    if e.contains("oracle") || e.contains("transport error") || e.contains("status {") {
        Failure::OracleDropped
    } else if e.contains("notfound") || e.contains("gave up waiting") || e.contains("peer closed") {
        Failure::NodeSyncing
    } else if e.contains("p2p") || e.contains("connection refused") || e.contains("peer") {
        Failure::PeerUnreachable
    } else {
        Failure::ScanStopped
    }
}

fn field_u64(line: &str, key: &str) -> Option<u64> {
    let start = line.find(key)? + key.len();
    let digits: String = line[start..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    // Golden lines: exact text friglet prints at the pin (blindbit-rs 93f6a92).
    const ORACLE_DOWN_AT_START: &str = "Error: transport error";
    const HTTP_PORT_TAKEN: &str = "Failed to bind HTTP server: Os { code: 98, kind: AddrInUse, message: \"Address already in use\" }";
    const ELECTRUM_PORT_TAKEN: &str = "2026-10-01T12:00:00.000000Z ERROR Electrum server terminated with error error=Address already in use (os error 98)";
    const STREAM_PANIC_1: &str =
        "thread 'tokio-runtime-worker' panicked at blindbit-lib/src/scanner/scanning.rs:95:66:";
    const STREAM_PANIC_2: &str = "called `Result::unwrap()` on an `Err` value: Status { code: Unknown, message: \"h2 protocol error\", source: None }";
    const ORACLE_RETRY: &str = "2026-10-01T12:00:00.000000Z ERROR failed to reach oracle, will retry error=status: 'The service is currently unavailable', self: \"tcp connect error\"";
    const P2P_OPEN_FAIL: &str = "2026-10-01T12:00:00.000000Z  WARN failed to open P2P connection peer=10.0.0.1:8333 attempt=1 error=Connection refused (os error 111)";
    const NOTFOUND: &str = "2026-10-01T12:00:00.000000Z  WARN block fetch failed; dropping connection and retrying peer=10.0.0.1:8333 height=900100 block_hash=00 attempt=1 max_attempts=4 error=peer 10.0.0.1:8333 replied notfound for block 00 (1 inv item(s)) — peer may be pruned or not serving this block";
    const GAVE_UP: &str = "2026-10-01T12:00:00.000000Z ERROR scan_block_range failed, will retry on next poll error=gave up waiting for block 00 from 10.0.0.1:8333 after 90.1s";
    const PROGRESS: &str = "2026-10-01T12:00:00.000000Z  INFO scan progress height=900100 scanned=100 total=200 pct=50";

    #[test]
    fn exits_are_classified_from_status_and_logs() {
        assert_eq!(
            classify_exit(ExitKind::Code(1), &lines(&["x", ORACLE_DOWN_AT_START])),
            Failure::OracleUnreachable
        );
        assert_eq!(
            classify_exit(
                ExitKind::Signal(6),
                &lines(&[
                    "thread 'main' panicked at friglet/src/main.rs:218:14:",
                    HTTP_PORT_TAKEN
                ])
            ),
            Failure::PortInUse
        );
        assert_eq!(
            classify_exit(
                ExitKind::Code(0),
                &lines(&[ELECTRUM_PORT_TAKEN, "INFO Electrum server stopped"])
            ),
            Failure::PortInUse
        );
        assert_eq!(
            classify_exit(
                ExitKind::Code(0),
                &lines(&[
                    "ERROR control socket server failed error=another friglet daemon is already listening on /data/friglet.sock"
                ])
            ),
            Failure::PortInUse
        );
        assert_eq!(
            classify_exit(
                ExitKind::Signal(6),
                &lines(&[STREAM_PANIC_1, STREAM_PANIC_2])
            ),
            Failure::OracleDropped
        );
        assert_eq!(
            classify_exit(ExitKind::Signal(9), &lines(&["whatever"])),
            Failure::Unknown
        );
        assert_eq!(classify_exit(ExitKind::Code(0), &[]), Failure::Unknown);
        assert_eq!(
            classify_exit(ExitKind::StartupDeadline, &[]),
            Failure::OracleUnreachable
        );
    }

    #[test]
    fn running_conditions_from_log_lines() {
        assert_eq!(
            classify_line(ORACLE_RETRY),
            Some(LogCondition::OracleRetrying)
        );
        assert_eq!(classify_line(PROGRESS), Some(LogCondition::Progress));
        assert_eq!(
            classify_line(P2P_OPEN_FAIL),
            Some(LogCondition::P2p {
                kind: P2pTrouble::Connect,
                height: None
            })
        );
        assert_eq!(
            classify_line(NOTFOUND),
            Some(LogCondition::P2p {
                kind: P2pTrouble::BlockUnavailable,
                height: Some(900_100)
            })
        );
        assert!(matches!(
            classify_line(GAVE_UP),
            Some(LogCondition::P2p {
                kind: P2pTrouble::BlockUnavailable,
                ..
            })
        ));
        assert_eq!(
            classify_line("INFO HTTP server listening addr=127.0.0.1:8095"),
            None
        );
    }

    #[test]
    fn p2p_trouble_uses_handshake_height_and_service_bits() {
        use P2pTrouble::*;
        assert_eq!(
            p2p_failure(Connect, None, Some(10), false),
            Failure::PeerUnreachable
        );
        assert_eq!(
            p2p_failure(BlockUnavailable, Some(100), Some(90), false),
            Failure::NodeSyncing
        );
        assert_eq!(
            p2p_failure(BlockUnavailable, Some(80), Some(90), false),
            Failure::PeerUnreachable
        );
        assert_eq!(
            p2p_failure(BlockUnavailable, Some(80), Some(90), true),
            Failure::NodePruned
        );
    }

    #[test]
    fn last_error_classification() {
        assert_eq!(
            classify_last_error("transport error"),
            Failure::OracleDropped
        );
        assert_eq!(
            classify_last_error("peer replied notfound"),
            Failure::NodeSyncing
        );
        assert_eq!(
            classify_last_error("P2P send failed"),
            Failure::PeerUnreachable
        );
        assert_eq!(classify_last_error("something"), Failure::ScanStopped);
    }
}
