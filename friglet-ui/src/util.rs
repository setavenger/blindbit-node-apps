//! Small shared helpers.

use std::time::{Duration, SystemTime};

use rand::RngCore;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

pub fn rfc3339(t: SystemTime) -> String {
    OffsetDateTime::from(t)
        .replace_nanosecond(0)
        .unwrap_or_else(|_| OffsetDateTime::from(t))
        .format(&Rfc3339)
        .unwrap_or_default()
}

pub fn now_rfc3339() -> String {
    rfc3339(SystemTime::now())
}

pub fn parse_rfc3339(s: &str) -> Option<SystemTime> {
    OffsetDateTime::parse(s, &Rfc3339)
        .ok()
        .map(SystemTime::from)
}

/// `n` random bytes as lowercase hex.
pub fn random_hex(n: usize) -> String {
    let mut buf = vec![0u8; n];
    rand::thread_rng().fill_bytes(&mut buf);
    hex::encode(buf)
}

/// Constant-time byte comparison.
pub fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// `base` scaled by ±`spread` (0.0–1.0) at random.
pub fn jitter(base: Duration, spread: f64) -> Duration {
    let f = 1.0 + spread * (rand::random::<f64>() * 2.0 - 1.0);
    base.mul_f64(f.max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc3339_roundtrip() {
        let s = now_rfc3339();
        assert!(s.ends_with('Z'), "{s}");
        assert!(parse_rfc3339(&s).is_some());
    }

    #[test]
    fn ct_eq_works() {
        assert!(ct_eq(b"abc", b"abc"));
        assert!(!ct_eq(b"abc", b"abd"));
        assert!(!ct_eq(b"abc", b"ab"));
    }

    #[test]
    fn jitter_stays_in_range() {
        for _ in 0..100 {
            let d = jitter(Duration::from_secs(100), 0.25);
            assert!(d >= Duration::from_secs(75) && d <= Duration::from_secs(125));
        }
    }
}
