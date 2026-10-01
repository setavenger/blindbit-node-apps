//! Best-effort redaction of friglet log lines before friglet-ui forwards them (until
//! friglet stops logging these at `info`, BR8): hashes and txids, keys, amounts, IP
//! addresses, Silent Payments addresses and descriptors.
//!
//! The classifier reads the raw lines from an in-memory ring; only redacted lines leave the
//! process.

use std::sync::LazyLock;

use regex::Regex;

struct Rule {
    re: Regex,
    with: &'static str,
}

static RULES: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    let r = |pattern: &str, with: &'static str| Rule {
        re: Regex::new(pattern).expect("static regex"),
        with,
    };
    vec![
        // Descriptors and keys in bech32 form (spscan/spspend, any network).
        r(
            r"(?i)\bt?sp(?:scan|spend)1[02-9ac-hj-np-z]{20,}",
            "<descriptor-key>",
        ),
        // Silent Payments addresses.
        r(
            r"(?i)\b(?:sp|tsp|sprt)1[02-9ac-hj-np-z]{20,}",
            "<sp-address>",
        ),
        // Ordinary bech32 addresses.
        r(r"(?i)\b(?:bc|tb|bcrt)1[02-9ac-hj-np-z]{20,}", "<address>"),
        // Hex runs of 40+ characters: txids, block hashes, keys, scripts.
        r(r"\b[0-9a-fA-F]{40,}\b", "<hex>"),
        // Amounts: "0.00012345 BTC", "1234 sat(s)".
        r(r"-?\b\d+(?:\.\d+)?\s?(?:BTC|btc|sats?|SAT)\b", "<amount>"),
        // Balance and value fields whatever their unit.
        r(
            r"\b(balance|value|amount|fee)=(?:[^\s,]+(?: BTC| sats?)?)",
            "$1=<amount>",
        ),
        // IPv6, bracketed with port.
        r(r"\[[0-9a-fA-F:.]{2,}\](?::\d+)?", "<ip>"),
        // IPv4 with optional port.
        r(r"\b(?:\d{1,3}\.){3}\d{1,3}(?::\d+)?\b", "<ip>"),
        // Bare IPv6 (at least three groups with ::, or 8 groups).
        r(
            r"\b(?:[0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}\b|\b(?:[0-9a-fA-F]{1,4}:){1,6}:[0-9a-fA-F]{1,4}\b",
            "<ip>",
        ),
    ]
});

/// Redact one log line.
pub fn redact(line: &str) -> String {
    let mut out = line.to_string();
    for rule in RULES.iter() {
        if rule.re.is_match(&out) {
            out = rule.re.replace_all(&out, rule.with).into_owned();
        }
    }
    out
}

/// Strip ANSI escape sequences (friglet colours its logs when its stderr is a TTY).
pub fn strip_ansi(line: &str) -> String {
    static ANSI: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"\x1b\[[0-9;?]*[ -/]*[@-~]").expect("static regex"));
    ANSI.replace_all(line, "").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_friglet_info_lines() {
        let cases = [
            (
                "2026-10-01T12:00:00.123456Z  INFO starting friglet oracle_url=https://signet.oracle.setor.dev p2p_peer=152.53.151.148:38333 network=signet start_height=270000",
                "p2p_peer=<ip>",
            ),
            (
                "2026-10-01T12:00:00Z  INFO balance after scan total=0.00012345 BTC confirmed=0.00012345 BTC trusted_pending=0 BTC untrusted_pending=0 BTC",
                "total=<amount>",
            ),
            (
                "2026-10-01T12:00:00Z  INFO probable match found txid=4f2a2d6b5c3e1f0a9b8c7d6e5f4a3b2c1d0e9f8a7b6c5d4e3f2a1b0c9d8e7f6a",
                "txid=<hex>",
            ),
            (
                "2026-10-01T12:00:00Z  INFO Electrum client connected peer=[2001:db8::1]:50001",
                "peer=<ip>",
            ),
            (
                "2026-10-01T12:00:00Z  INFO Electrum client connected peer=192.168.1.20:51234",
                "peer=<ip>",
            ),
        ];
        for (line, expect) in cases {
            let r = redact(line);
            assert!(r.contains(expect), "{line}\n=> {r}");
            assert!(!r.contains(" BTC"), "{r}");
            assert!(!r.contains("152.53.151.148"));
            assert!(!r.contains("0.00012345"));
            assert!(!r.contains("4f2a2d6b5c3e"));
            assert!(!r.contains("192.168.1.20"));
            assert!(!r.contains("2001:db8"));
        }
    }

    #[test]
    fn redacts_keys_addresses_and_descriptors() {
        let secret = "5ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7";
        let r = redact(&format!("key {secret} pubkey=02{secret}"));
        assert!(!r.contains("5ec7e7"), "{r}");
        let r = redact(
            "addr tsp1qqgste7k9hx0qftg6qmwlkqtwuy6cycyavzmzj85c6qdfhjdpdjtdgqjuexzk6murw56suy3e0rd2cgqvycxttddwsvgxe2usfpxumr70xc9pkqwv end",
        );
        assert_eq!(r, "addr <sp-address> end");
        let r = redact(
            "sp(spscan1qqgste7k9hx0qftg6qmwlkqtwuy6cycyavzmzj85c6qdfhjdpdjtdgqjuexzk6murw56suy3e0rd2cgqv)",
        );
        assert!(r.contains("<descriptor-key>"), "{r}");
    }

    #[test]
    fn keeps_harmless_text() {
        let line = "2026-10-01T12:00:00.123456Z  INFO new blocks available, scanning from=270001 to=270050";
        assert_eq!(redact(line), line);
        let line =
            "2026-10-01T12:00:00Z  INFO scan progress height=324500 scanned=25 total=31 pct=80";
        assert_eq!(redact(line), line, "block counts are not amounts");
        let line = "2026-10-01T12:00:00Z  INFO control socket listening path=/data/friglet.sock";
        assert_eq!(redact(line), line);
    }

    #[test]
    fn strips_ansi() {
        assert_eq!(
            strip_ansi("\x1b[2m2026\x1b[0m \x1b[32m INFO\x1b[0m x"),
            "2026  INFO x"
        );
    }
}
