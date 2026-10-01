//! Wallet key input: BIP-392 `sp()` descriptors and the `hex:<scan>:<spend>` form.
//!
//! friglet scans for one wallet given its scan secret and spend public key. This module turns
//! what the user pastes into those two keys, checks the BIP-380 checksum and the network, and
//! derives the wallet's Silent Payments address (BIP-352 v0) and a short key fingerprint.
//!
//! Accepted:
//! - `sp(spscan1q…)` / `sp(tspscan1q…)`, optionally with key origin `[fingerprint/path]`
//!   and checksum `#xxxxxxxx` (what Sparrow shows);
//! - `sp(SCAN,SPEND)` with a compressed WIF or `xprv` (plus path) scan key and a compressed
//!   hex, `xpub` (plus unhardened path) spend key;
//! - `hex:<scan_secret_64hex>:<spend_pubkey_66hex>` (no network information).
//!
//! Refused on purpose: anything carrying the **spend private key** (`spspend`, a WIF or xprv in
//! the spend position). friglet only needs the watch-only keys, and friglet-ui never wants to
//! hold the key that can move funds.

use std::fmt;
use std::str::FromStr;

use bech32::primitives::decode::CheckedHrpstring;
use bech32::{Bech32m, ByteIterExt, Fe32, Fe32IterExt, Hrp};
use bitcoin::NetworkKind;
use bitcoin::bip32::{ChildNumber, DerivationPath, Xpriv, Xpub};
use bitcoin::hashes::{Hash, sha256};
use bitcoin::secp256k1::{PublicKey, Secp256k1, SecretKey};
use serde::Serialize;

use crate::network::{KeyFamily, Network};

/// The scan secret and spend public key of one wallet.
///
/// `Debug` never prints the scan secret.
#[derive(Clone, PartialEq, Eq)]
pub struct WalletKeys {
    scan_secret: SecretKey,
    spend_pubkey: PublicKey,
    /// Network family the input declared (`None` for `hex:` entry).
    family: Option<KeyFamily>,
}

impl fmt::Debug for WalletKeys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WalletKeys")
            .field("scan_secret", &"<redacted>")
            .field("spend_pubkey", &self.spend_pubkey.to_string())
            .field("family", &self.family)
            .finish()
    }
}

/// Public identity of a wallet on one network: what the UI shows and what is compared with
/// friglet's reported `sp_address`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WalletIdentity {
    pub sp_address: String,
    pub key_fingerprint: String,
    pub network: Network,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DescriptorError {
    Empty,
    Checksum(String),
    Syntax(String),
    Encoding(String),
    UnknownPrefix(String),
    UnsupportedVersion(char),
    InvalidKey(String),
    UncompressedKey,
    SpendPrivateKey,
    ScanKeyNotPrivate,
    Unsupported(String),
    MixedNetworks,
    NetworkMismatch {
        descriptor: KeyFamily,
        configured: Network,
    },
}

impl DescriptorError {
    /// Machine id for API errors.
    pub fn code(&self) -> &'static str {
        match self {
            DescriptorError::Checksum(_) => "descriptor-checksum",
            DescriptorError::NetworkMismatch { .. } => "network-mismatch",
            DescriptorError::SpendPrivateKey => "spend-private-key",
            _ => "descriptor-invalid",
        }
    }
}

impl fmt::Display for DescriptorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DescriptorError::Empty => f.write_str("paste the sp(…) descriptor from your wallet"),
            DescriptorError::Checksum(m) => write!(f, "descriptor checksum: {m}"),
            DescriptorError::Syntax(m) => write!(f, "not a valid sp() descriptor: {m}"),
            DescriptorError::Encoding(m) => write!(f, "key encoding is damaged: {m}"),
            DescriptorError::UnknownPrefix(p) => write!(
                f,
                "unknown key prefix `{p}`: expected spscan (mainnet) or tspscan (test networks)"
            ),
            DescriptorError::UnsupportedVersion(v) => write!(
                f,
                "Silent Payments key version `{v}` is not supported (only version 0, `q`)"
            ),
            DescriptorError::InvalidKey(m) => write!(f, "invalid key: {m}"),
            DescriptorError::UncompressedKey => {
                f.write_str("uncompressed keys are not allowed in sp() descriptors (BIP-352)")
            }
            DescriptorError::SpendPrivateKey => f.write_str(
                "this descriptor contains your spend private key; paste the watch-only \
                 descriptor (sp(spscan1…)) instead. friglet never needs the key that can spend",
            ),
            DescriptorError::ScanKeyNotPrivate => f.write_str(
                "the first key of sp(SCAN,SPEND) must be the private scan key (WIF or xprv)",
            ),
            DescriptorError::Unsupported(m) => write!(f, "not supported: {m}"),
            DescriptorError::MixedNetworks => {
                f.write_str("the scan and spend keys belong to different networks")
            }
            DescriptorError::NetworkMismatch {
                descriptor,
                configured,
            } => write!(
                f,
                "this descriptor is for {}, but this node runs {configured}",
                descriptor.describe()
            ),
        }
    }
}

impl std::error::Error for DescriptorError {}

impl WalletKeys {
    pub fn new(scan_secret: SecretKey, spend_pubkey: PublicKey, family: Option<KeyFamily>) -> Self {
        Self {
            scan_secret,
            spend_pubkey,
            family,
        }
    }

    pub fn family(&self) -> Option<KeyFamily> {
        self.family
    }

    pub fn scan_pubkey(&self) -> PublicKey {
        PublicKey::from_secret_key(&Secp256k1::signing_only(), &self.scan_secret)
    }

    pub fn spend_pubkey(&self) -> PublicKey {
        self.spend_pubkey
    }

    /// Hex of the scan secret, for the key file only.
    pub fn scan_secret_hex(&self) -> String {
        hex::encode(self.scan_secret.secret_bytes())
    }

    pub fn spend_pubkey_hex(&self) -> String {
        self.spend_pubkey.to_string()
    }

    /// Refuse a descriptor whose network family differs from the node's network.
    pub fn check_network(&self, network: Network) -> Result<(), DescriptorError> {
        match self.family {
            Some(f) if f != network.family() => Err(DescriptorError::NetworkMismatch {
                descriptor: f,
                configured: network,
            }),
            _ => Ok(()),
        }
    }

    pub fn identity(&self, network: Network) -> WalletIdentity {
        let scan = self.scan_pubkey();
        WalletIdentity {
            sp_address: sp_address(&scan, &self.spend_pubkey, network),
            key_fingerprint: key_fingerprint(&scan, &self.spend_pubkey),
            network,
        }
    }

    /// Canonical `sp(spscan…)#checksum` form for these keys.
    pub fn to_descriptor(&self, family: KeyFamily) -> String {
        let key = encode_spscan(&self.scan_secret, &self.spend_pubkey, family);
        with_checksum(&format!("sp({key})"))
    }
}

/// BIP-352 v0 address `hrp1q…` of `B_scan || B_spend`, with friglet's prefix per network.
pub fn sp_address(scan: &PublicKey, spend: &PublicKey, network: Network) -> String {
    let hrp = Hrp::parse(network.sp_hrp()).expect("static hrp");
    let mut data = Vec::with_capacity(66);
    data.extend_from_slice(&scan.serialize());
    data.extend_from_slice(&spend.serialize());
    data.iter()
        .copied()
        .bytes_to_fes()
        .with_checksum::<Bech32m>(&hrp)
        .with_witness_version(Fe32::Q)
        .chars()
        .collect()
}

/// Short, stable, non-secret label for a wallet: first 4 bytes of
/// SHA-256(`B_scan || B_spend`) in hex.
pub fn key_fingerprint(scan: &PublicKey, spend: &PublicKey) -> String {
    let mut data = Vec::with_capacity(66);
    data.extend_from_slice(&scan.serialize());
    data.extend_from_slice(&spend.serialize());
    let digest = sha256::Hash::hash(&data);
    hex::encode(&digest.as_byte_array()[..4])
}

/// BIP-392 `spscan`/`tspscan` encoding of `b_scan || B_spend`, version 0.
pub fn encode_spscan(scan: &SecretKey, spend: &PublicKey, family: KeyFamily) -> String {
    let hrp = Hrp::parse(match family {
        KeyFamily::Mainnet => "spscan",
        KeyFamily::Test => "tspscan",
    })
    .expect("static hrp");
    let mut data = Vec::with_capacity(65);
    data.extend_from_slice(&scan.secret_bytes());
    data.extend_from_slice(&spend.serialize());
    data.iter()
        .copied()
        .bytes_to_fes()
        .with_checksum::<Bech32m>(&hrp)
        .with_witness_version(Fe32::Q)
        .chars()
        .collect()
}

/// Parse what the user pasted: an `sp()` descriptor or `hex:<scan>:<spend>`.
pub fn parse(input: &str) -> Result<WalletKeys, DescriptorError> {
    let s: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    if s.is_empty() {
        return Err(DescriptorError::Empty);
    }
    if let Some(rest) = s.strip_prefix("hex:") {
        return parse_hex(rest);
    }
    let body = verify_checksum(&s)?;
    let Some(inner) = body.strip_prefix("sp(").and_then(|r| r.strip_suffix(')')) else {
        if body.contains("sp(") {
            return Err(DescriptorError::Syntax(
                "sp() is only allowed as the top-level expression".into(),
            ));
        }
        return Err(DescriptorError::Syntax(
            "expected a descriptor of the form sp(…)".into(),
        ));
    };
    let args = split_top_level(inner)?;
    match args.as_slice() {
        [] => Err(DescriptorError::Syntax(
            "sp() requires at least one key expression".into(),
        )),
        [one] => parse_single(one),
        [scan, spend] => parse_pair(scan, spend),
        _ => Err(DescriptorError::Syntax(
            "sp() takes one or two key expressions".into(),
        )),
    }
}

fn parse_hex(rest: &str) -> Result<WalletKeys, DescriptorError> {
    let Some((scan_hex, spend_hex)) = rest.split_once(':') else {
        return Err(DescriptorError::Syntax(
            "hex entry is hex:<scan_secret_64hex>:<spend_pubkey_66hex>".into(),
        ));
    };
    if scan_hex.len() != 64 || !scan_hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(DescriptorError::InvalidKey(
            "scan secret must be 64 hex characters".into(),
        ));
    }
    let scan = SecretKey::from_str(scan_hex).map_err(|_| {
        DescriptorError::InvalidKey("scan secret is not a valid secp256k1 key".into())
    })?;
    let spend = parse_hex_pubkey(spend_hex)?;
    Ok(WalletKeys::new(scan, spend, None))
}

fn parse_hex_pubkey(s: &str) -> Result<PublicKey, DescriptorError> {
    if !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(DescriptorError::InvalidKey(
            "spend public key must be hex".into(),
        ));
    }
    match s.len() {
        66 if s.starts_with("02") || s.starts_with("03") => PublicKey::from_str(s).map_err(|_| {
            DescriptorError::InvalidKey("spend public key is not on the curve".into())
        }),
        130 if s.starts_with("04") => Err(DescriptorError::UncompressedKey),
        _ => Err(DescriptorError::InvalidKey(
            "spend public key must be 66 hex characters (compressed)".into(),
        )),
    }
}

// ---------------------------------------------------------------------------------------------
// BIP-380 checksum
// ---------------------------------------------------------------------------------------------

const INPUT_CHARSET: &str = "0123456789()[],'/*abcdefgh@:$%{}IJKLMNOPQRSTUVWXYZ&+-.;<=>?!^_|~ijklmnopqrstuvwxyzABCDEFGH`#\"\\ ";
const CHECKSUM_CHARSET: &[u8; 32] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";

fn polymod(c: u64, val: u64) -> u64 {
    let c0 = c >> 35;
    let mut c = ((c & 0x7_ffff_ffff) << 5) ^ val;
    if c0 & 1 != 0 {
        c ^= 0xf5_dee5_1989;
    }
    if c0 & 2 != 0 {
        c ^= 0xa9_fdca_3312;
    }
    if c0 & 4 != 0 {
        c ^= 0x1b_ab10_e32d;
    }
    if c0 & 8 != 0 {
        c ^= 0x37_06b1_677a;
    }
    if c0 & 16 != 0 {
        c ^= 0x64_4d62_6ffd;
    }
    c
}

/// BIP-380 descriptor checksum of `desc` (without `#`), or `None` for characters outside
/// the descriptor character set.
pub fn checksum(desc: &str) -> Option<String> {
    let mut c = 1u64;
    let mut cls = 0u64;
    let mut clscount = 0;
    for ch in desc.chars() {
        let pos = INPUT_CHARSET.find(ch)? as u64;
        c = polymod(c, pos & 31);
        cls = cls * 3 + (pos >> 5);
        clscount += 1;
        if clscount == 3 {
            c = polymod(c, cls);
            cls = 0;
            clscount = 0;
        }
    }
    if clscount > 0 {
        c = polymod(c, cls);
    }
    for _ in 0..8 {
        c = polymod(c, 0);
    }
    c ^= 1;
    Some(
        (0..8)
            .map(|j| CHECKSUM_CHARSET[((c >> (5 * (7 - j))) & 31) as usize] as char)
            .collect(),
    )
}

/// `desc#checksum`.
pub fn with_checksum(desc: &str) -> String {
    match checksum(desc) {
        Some(c) => format!("{desc}#{c}"),
        None => desc.to_string(),
    }
}

/// Check the optional `#checksum` suffix and return the descriptor body.
pub fn verify_checksum(s: &str) -> Result<&str, DescriptorError> {
    let Some((body, sum)) = s.split_once('#') else {
        if checksum(s).is_none() {
            return Err(DescriptorError::Checksum(
                "invalid characters in descriptor".into(),
            ));
        }
        return Ok(s);
    };
    if sum.len() != 8 {
        return Err(DescriptorError::Checksum(format!(
            "expected 8 checksum characters after #, found {}",
            sum.len()
        )));
    }
    if !sum.bytes().all(|b| CHECKSUM_CHARSET.contains(&b)) {
        return Err(DescriptorError::Checksum(
            "checksum has invalid characters".into(),
        ));
    }
    let expected = checksum(body)
        .ok_or_else(|| DescriptorError::Checksum("invalid characters in descriptor".into()))?;
    if expected != sum {
        return Err(DescriptorError::Checksum(
            "checksum does not match; the descriptor was changed or mistyped".into(),
        ));
    }
    Ok(body)
}

// ---------------------------------------------------------------------------------------------
// Key expressions
// ---------------------------------------------------------------------------------------------

fn split_top_level(inner: &str) -> Result<Vec<&str>, DescriptorError> {
    if inner.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, ch) in inner.char_indices() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => {
                depth -= 1;
                if depth < 0 {
                    return Err(DescriptorError::Syntax("unbalanced brackets".into()));
                }
            }
            ',' if depth == 0 => {
                out.push(&inner[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err(DescriptorError::Syntax("unbalanced brackets".into()));
    }
    out.push(&inner[start..]);
    if out.iter().any(|a| a.is_empty()) {
        return Err(DescriptorError::Syntax("empty key expression".into()));
    }
    Ok(out)
}

/// Strip and validate an optional BIP-380 key origin `[fingerprint/path]`.
fn strip_origin(expr: &str) -> Result<&str, DescriptorError> {
    let Some(rest) = expr.strip_prefix('[') else {
        if expr.contains(']') || expr.contains('[') {
            return Err(DescriptorError::Syntax(
                "misplaced key origin brackets".into(),
            ));
        }
        return Ok(expr);
    };
    let Some((origin, key)) = rest.split_once(']') else {
        return Err(DescriptorError::Syntax("key origin is not closed".into()));
    };
    if key.is_empty() {
        return Err(DescriptorError::Syntax("key origin without a key".into()));
    }
    if key.contains('[') || key.contains(']') {
        return Err(DescriptorError::Syntax("multiple key origins".into()));
    }
    let mut parts = origin.split('/');
    let fp = parts.next().unwrap_or_default();
    if fp.len() != 8 || !fp.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(DescriptorError::Syntax(
            "key origin fingerprint must be 8 hex characters".into(),
        ));
    }
    for step in parts {
        parse_path_step(step, true)?;
    }
    Ok(key)
}

/// One derivation step: `n`, `nh` or `n'`.
fn parse_path_step(step: &str, allow_hardened: bool) -> Result<ChildNumber, DescriptorError> {
    let (num, hardened) = match step.strip_suffix('h').or_else(|| step.strip_suffix('\'')) {
        Some(n) => (n, true),
        None => (step, false),
    };
    if num.is_empty() || !num.bytes().all(|b| b.is_ascii_digit()) {
        return Err(DescriptorError::Syntax(format!(
            "invalid derivation step `{step}`"
        )));
    }
    let index: u32 = num
        .parse()
        .map_err(|_| DescriptorError::Syntax(format!("derivation index `{step}` out of range")))?;
    if index >= 0x8000_0000 {
        return Err(DescriptorError::Syntax(format!(
            "derivation index `{step}` out of range"
        )));
    }
    if hardened {
        if !allow_hardened {
            return Err(DescriptorError::Syntax(
                "hardened derivation needs a private key".into(),
            ));
        }
        Ok(ChildNumber::Hardened { index })
    } else {
        Ok(ChildNumber::Normal { index })
    }
}

/// Split `key/1/2h` into the key and its derivation path; wildcards and multipath are
/// refused because sp() keys are single keys.
fn split_path(expr: &str, allow_hardened: bool) -> Result<(&str, DerivationPath), DescriptorError> {
    let mut parts = expr.split('/');
    let key = parts.next().unwrap_or_default();
    let mut steps = Vec::new();
    for step in parts {
        if step.contains('*') || step.contains('<') {
            return Err(DescriptorError::Syntax(
                "wildcards and multipath are not allowed in sp() key expressions".into(),
            ));
        }
        steps.push(parse_path_step(step, allow_hardened)?);
    }
    Ok((key, DerivationPath::from(steps)))
}

fn family_of(kind: NetworkKind) -> KeyFamily {
    match kind {
        NetworkKind::Main => KeyFamily::Mainnet,
        NetworkKind::Test => KeyFamily::Test,
    }
}

fn parse_single(expr: &str) -> Result<WalletKeys, DescriptorError> {
    let key = strip_origin(expr)?;
    let lower = key.to_ascii_lowercase();
    if lower.starts_with("spspend1") || lower.starts_with("tspspend1") {
        return Err(DescriptorError::SpendPrivateKey);
    }
    if !(lower.starts_with("spscan1") || lower.starts_with("tspscan1")) {
        return Err(DescriptorError::Syntax(
            "the one-argument form sp(KEY) needs an spscan key".into(),
        ));
    }
    decode_spscan(key)
}

/// Decode an `spscan`/`tspscan` key into the wallet keys.
pub fn decode_spscan(key: &str) -> Result<WalletKeys, DescriptorError> {
    let mut checked = CheckedHrpstring::new::<Bech32m>(key)
        .map_err(|e| DescriptorError::Encoding(e.to_string()))?;
    let hrp = checked.hrp().to_lowercase();
    let family = match hrp.as_str() {
        "spscan" => KeyFamily::Mainnet,
        "tspscan" => KeyFamily::Test,
        "spspend" | "tspspend" => return Err(DescriptorError::SpendPrivateKey),
        other => return Err(DescriptorError::UnknownPrefix(other.to_string())),
    };
    let version = checked
        .remove_witness_version()
        .ok_or_else(|| DescriptorError::Encoding("missing version character".into()))?;
    if version != Fe32::Q {
        return Err(DescriptorError::UnsupportedVersion(version.to_char()));
    }
    let bytes: Vec<u8> = checked.byte_iter().collect();
    if bytes.len() != 65 {
        return Err(DescriptorError::Encoding(format!(
            "payload is {} bytes, expected 65 (scan secret and spend public key)",
            bytes.len()
        )));
    }
    let scan = SecretKey::from_slice(&bytes[..32])
        .map_err(|_| DescriptorError::InvalidKey("scan secret is not a valid key".into()))?;
    if bytes[32] != 0x02 && bytes[32] != 0x03 {
        return Err(DescriptorError::InvalidKey(
            "spend public key must be compressed".into(),
        ));
    }
    let spend = PublicKey::from_slice(&bytes[32..])
        .map_err(|_| DescriptorError::InvalidKey("spend public key is not on the curve".into()))?;
    Ok(WalletKeys::new(scan, spend, Some(family)))
}

fn parse_pair(scan_expr: &str, spend_expr: &str) -> Result<WalletKeys, DescriptorError> {
    let secp = Secp256k1::new();

    // Scan key: private, single key.
    let scan_key = strip_origin(scan_expr)?;
    let (scan, scan_family) = if scan_key.starts_with("musig(") {
        return Err(DescriptorError::ScanKeyNotPrivate);
    } else if scan_key.starts_with("xprv") || scan_key.starts_with("tprv") {
        let (base, path) = split_path(scan_key, true)?;
        let xprv = Xpriv::from_str(base)
            .map_err(|e| DescriptorError::InvalidKey(format!("scan xprv: {e}")))?;
        let derived = xprv
            .derive_priv(&secp, &path)
            .map_err(|e| DescriptorError::InvalidKey(format!("scan derivation: {e}")))?;
        (derived.private_key, Some(family_of(xprv.network)))
    } else if scan_key.starts_with("xpub")
        || scan_key.starts_with("tpub")
        || scan_key.starts_with("02")
        || scan_key.starts_with("03")
        || scan_key.starts_with("04")
    {
        return Err(DescriptorError::ScanKeyNotPrivate);
    } else if scan_key.starts_with("spscan") || scan_key.starts_with("tspscan") {
        return Err(DescriptorError::Syntax(
            "the two-argument form takes single keys, not spscan keys".into(),
        ));
    } else {
        if scan_key.contains('/') {
            return Err(DescriptorError::Syntax(
                "a WIF private key cannot have a derivation path".into(),
            ));
        }
        let wif = bitcoin::PrivateKey::from_wif(scan_key)
            .map_err(|e| DescriptorError::InvalidKey(format!("scan key: {e}")))?;
        if !wif.compressed {
            return Err(DescriptorError::UncompressedKey);
        }
        (wif.inner, Some(family_of(wif.network)))
    };

    // Spend key: public, single key.
    let spend_key = strip_origin(spend_expr)?;
    let (spend, spend_family) = if spend_key.starts_with("musig(") {
        return Err(DescriptorError::Unsupported(
            "musig() spend keys (friglet scans single spend keys only)".into(),
        ));
    } else if spend_key.starts_with("xpub") || spend_key.starts_with("tpub") {
        let (base, path) = split_path(spend_key, false)?;
        let xpub = Xpub::from_str(base)
            .map_err(|e| DescriptorError::InvalidKey(format!("spend xpub: {e}")))?;
        let derived = xpub
            .derive_pub(&secp, &path)
            .map_err(|e| DescriptorError::InvalidKey(format!("spend derivation: {e}")))?;
        (derived.public_key, Some(family_of(xpub.network)))
    } else if spend_key.starts_with("xprv")
        || spend_key.starts_with("tprv")
        || bitcoin::PrivateKey::from_wif(spend_key.split('/').next().unwrap_or_default()).is_ok()
    {
        return Err(DescriptorError::SpendPrivateKey);
    } else {
        if spend_key.contains('/') {
            return Err(DescriptorError::Syntax(
                "a hex public key cannot have a derivation path".into(),
            ));
        }
        (parse_hex_pubkey(spend_key)?, None)
    };

    let family = match (scan_family, spend_family) {
        (Some(a), Some(b)) if a != b => return Err(DescriptorError::MixedNetworks),
        (a, b) => a.or(b),
    };
    Ok(WalletKeys::new(scan, spend, family))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::path::PathBuf;

    fn upstream(path: &str) -> String {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../upstream/bips")
            .join(path);
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("read {}: {e}", p.display()))
    }

    /// `<tt>…</tt>` contents of the bullet lines following `heading` up to the next blank
    /// paragraph, with the bullet label.
    fn tt_items(text: &str, heading: &str) -> Vec<(String, String)> {
        let start = text.find(heading).expect("heading present") + heading.len();
        let mut out = Vec::new();
        let mut seen_any = false;
        for line in text[start..].lines() {
            let line = line.trim();
            if line.is_empty() {
                if seen_any {
                    break;
                }
                continue;
            }
            if !line.starts_with('*') {
                if seen_any {
                    break;
                }
                continue;
            }
            seen_any = true;
            let (label, rest) = line
                .trim_start_matches('*')
                .split_once(':')
                .unwrap_or_default();
            let label = label.trim().to_string();
            let content = rest
                .split("<tt>")
                .nth(1)
                .and_then(|r| r.split("</tt>").next())
                .unwrap_or_default()
                .to_string();
            out.push((label, content));
        }
        out
    }

    pub(crate) fn random_keys(seed: u8) -> (SecretKey, PublicKey) {
        let secp = Secp256k1::new();
        let mut scan = [seed; 32];
        scan[0] = 0x11;
        let mut spend = [seed.wrapping_add(7); 32];
        spend[0] = 0x22;
        let scan = SecretKey::from_slice(&scan).unwrap();
        let spend = SecretKey::from_slice(&spend).unwrap();
        (scan, PublicKey::from_secret_key(&secp, &spend))
    }

    #[test]
    fn bip380_checksum_vectors() {
        let text = upstream("bip-0380.mediawiki");
        let items = tt_items(
            &text,
            "The following tests cover the checksum and character set:",
        );
        assert_eq!(
            items.len(),
            8,
            "BIP-380 lists 8 checksum vectors: {items:?}"
        );
        for (label, vector) in items {
            let result = verify_checksum(&vector);
            match label.as_str() {
                "Valid checksum" => {
                    assert_eq!(result, Ok("raw(deadbeef)"));
                    assert_eq!(checksum("raw(deadbeef)").unwrap(), "89f8spxm");
                }
                "No checksum" => assert_eq!(result, Ok("raw(deadbeef)")),
                _ => assert!(result.is_err(), "{label}: {vector} must fail"),
            }
        }
    }

    #[test]
    fn bip380_key_origin_vectors_in_spend_position() {
        let text = upstream("bip-0380.mediawiki");
        let wif = "L4rK1yDtCWekvXuE6oXD9jCYfFNV2cWRpVuPLBcCU2z8TrisoyY1";
        let valid = tt_items(&text, "Valid expressions:");
        for (label, expr) in valid {
            if !label.starts_with("Compressed public key")
                && !label.starts_with("Public key with key origin")
            {
                continue;
            }
            let desc = format!("sp({wif},{expr})");
            parse(&desc).unwrap_or_else(|e| panic!("{label}: {desc}: {e}"));
        }
        let invalid = tt_items(&text, "Invalid expression:");
        let mut checked = 0;
        for (label, expr) in invalid {
            let desc = format!("sp({wif},{expr})");
            assert!(parse(&desc).is_err(), "{label}: {desc} must fail");
            checked += 1;
        }
        assert!(checked >= 10);
    }

    #[derive(serde::Deserialize)]
    struct Vector {
        comment: String,
        receiving: Vec<Receiving>,
    }
    #[derive(serde::Deserialize)]
    struct Receiving {
        given: Given,
        expected: Expected,
    }
    #[derive(serde::Deserialize)]
    struct Given {
        key_material: KeyMaterial,
        labels: Vec<u32>,
    }
    #[derive(serde::Deserialize)]
    struct KeyMaterial {
        scan_priv_key: String,
        spend_priv_key: String,
    }
    #[derive(serde::Deserialize)]
    struct Expected {
        addresses: Vec<String>,
    }

    pub(crate) fn bip352_vectors() -> Vec<(SecretKey, PublicKey, String)> {
        let json = upstream("bip-0352/send_and_receive_test_vectors.json");
        let vectors: Vec<Vector> = serde_json::from_str(&json).unwrap();
        let secp = Secp256k1::new();
        let mut out = Vec::new();
        for v in vectors {
            for r in v.receiving {
                let scan = SecretKey::from_str(&r.given.key_material.scan_priv_key).unwrap();
                let spend_sk = SecretKey::from_str(&r.given.key_material.spend_priv_key).unwrap();
                let spend = PublicKey::from_secret_key(&secp, &spend_sk);
                // The unlabelled address is always listed; with labels the list also holds
                // the labelled ones.
                let base = sp_address(
                    &PublicKey::from_secret_key(&secp, &scan),
                    &spend,
                    Network::Bitcoin,
                );
                assert!(
                    r.expected.addresses.contains(&base),
                    "{}: base address not among expected (labels {:?})",
                    v.comment,
                    r.given.labels
                );
                out.push((scan, spend, base));
            }
        }
        out
    }

    #[test]
    fn bip352_receiving_vectors_through_spscan_and_hex() {
        let json = upstream("bip-0352/send_and_receive_test_vectors.json");
        let vectors: Vec<Vector> = serde_json::from_str(&json).unwrap();
        let secp = Secp256k1::new();
        let mut n = 0;
        for v in vectors {
            for r in v.receiving {
                let scan = SecretKey::from_str(&r.given.key_material.scan_priv_key).unwrap();
                let spend_sk = SecretKey::from_str(&r.given.key_material.spend_priv_key).unwrap();
                let spend = PublicKey::from_secret_key(&secp, &spend_sk);
                let expected = &r.expected.addresses[0];
                if !r.given.labels.is_empty() {
                    // First address of a labelled vector is the base address too.
                    assert!(expected.starts_with("sp1q"));
                }
                let desc = WalletKeys::new(scan, spend, Some(KeyFamily::Mainnet))
                    .to_descriptor(KeyFamily::Mainnet);
                let keys = parse(&desc).unwrap();
                assert_eq!(
                    &keys.identity(Network::Bitcoin).sp_address,
                    expected,
                    "{}",
                    v.comment
                );
                let hex_entry = format!("hex:{}:{}", r.given.key_material.scan_priv_key, spend);
                let keys = parse(&hex_entry).unwrap();
                assert_eq!(&keys.identity(Network::Bitcoin).sp_address, expected);
                assert_eq!(keys.family(), None);
                n += 1;
            }
        }
        assert!(n >= 20, "expected the full BIP-352 receiving set, got {n}");
    }

    #[test]
    fn bip392_examples() {
        let text = upstream("bip-0392.mediawiki");
        let wif_example = "sp(L4rK1yDtCWekvXuE6oXD9jCYfFNV2cWRpVuPLBcCU2z8TrisoyY1,0260b2003c386519fc9eadf2b5cf124dd8eea4c4e68d5e154050a9346ea98ce600)";
        assert!(text.contains(wif_example));
        let keys = parse(wif_example).unwrap();
        assert_eq!(keys.family(), Some(KeyFamily::Mainnet));
        assert!(
            keys.identity(Network::Bitcoin)
                .sp_address
                .starts_with("sp1qq")
        );

        let uncompressed = "sp(5KYZdUEo39z3FPrtuX2QbbwGnNP5zTd7yyr2SC1j299sBCnWjss,0260b2003c386519fc9eadf2b5cf124dd8eea4c4e68d5e154050a9346ea98ce600)";
        assert!(text.contains(uncompressed));
        assert_eq!(parse(uncompressed), Err(DescriptorError::UncompressedKey));

        // The examples elide keys with `…`; substitute real ones.
        let (scan, spend) = random_keys(3);
        let spscan = encode_spscan(&scan, &spend, KeyFamily::Mainnet);
        let xpub = "xpub6ERApfZwUNrhLCkDtcHTcxd75RbzS1ed54G1LkBUHQVHQKqhMkhgbmJbZRkrgZw4koxb5JaHWkY4ALHY2grBGRjaDMzQLcgJvLJuZZvRcEL";
        let xprv = "xprvA1RpRA33e1JQ7ifknakTFpgNXPmW2YvmhqLQYMmrj4xJXXWYpDPS3xz7iAxn8L39njGVyuoseXzU6rcxFLJ8HFsTjSyQbLYnMpCqE2VbFWc";
        let expand = |s: &str| {
            s.replace("spscan1q...", &spscan)
                .replace("xpub...", xpub)
                .replace("xprv...", xprv)
        };
        assert!(text.contains("<tt>sp(spscan1q...)</tt>"));
        assert!(parse(&expand("sp(spscan1q...)")).is_ok());
        assert!(parse(&expand("sp([deadbeef/352h/0h/0h]spscan1q...)")).is_ok());
        // The BIP's example derives the xpub along a hardened step (`xpub.../0h`), which no
        // public key can do; friglet-ui refuses it with a clear message and accepts the
        // unhardened form.
        let hardened_xpub = "sp([deadbeef/352h/0h/0h]xprv.../0h,xpub.../0h)";
        assert!(text.contains(hardened_xpub));
        assert!(
            matches!(parse(&expand(hardened_xpub)), Err(DescriptorError::Syntax(m)) if m.contains("hardened"))
        );
        assert!(parse(&expand("sp([deadbeef/352h/0h/0h]xprv.../0h,xpub.../0)")).is_ok());
        assert_eq!(
            parse(&expand("sp([deadbeef/352h/0h/0h]xprv.../0h,xprv.../0h)")),
            Err(DescriptorError::SpendPrivateKey)
        );
        assert!(matches!(
            parse(
                "sp(L4rK1yDtCWekvXuE6oXD9jCYfFNV2cWRpVuPLBcCU2z8TrisoyY1,musig(03dff1d77f2a671c5f36183726db2341be58feae1da2deced843240f7b502ba659,023590a94e768f8e1815c2f24b4d80a8e3149316c3518ce7b7ad338368d038ca66))"
            ),
            Err(DescriptorError::Unsupported(_))
        ));

        for invalid in [
            "sp()",
            "sp(xpub...)",
            "sp(xpub...,xpub...)",
            "sp(spscan1q...,spscan1q...)",
            "sh(sp(spscan1q...))",
            "wsh(sp(spscan1q...))",
        ] {
            assert!(
                text.contains(&format!("<tt>{invalid}</tt>")),
                "{invalid} listed in BIP-392"
            );
            assert!(
                parse(&expand(invalid)).is_err(),
                "{invalid} must be refused"
            );
        }
    }

    #[test]
    fn spscan_roundtrip_checksum_and_network_detection() {
        let (scan, spend) = random_keys(9);
        for family in [KeyFamily::Mainnet, KeyFamily::Test] {
            let keys = WalletKeys::new(scan, spend, Some(family));
            let desc = keys.to_descriptor(family);
            assert!(desc.starts_with(match family {
                KeyFamily::Mainnet => "sp(spscan1q",
                KeyFamily::Test => "sp(tspscan1q",
            }));
            let parsed = parse(&desc).unwrap();
            assert_eq!(parsed, keys);
            // Without checksum, upper-case key and surrounding whitespace are fine too.
            let body = desc.split('#').next().unwrap();
            assert_eq!(parse(&format!("  {body}\n")).unwrap(), keys);
            // One flipped character breaks the checksum.
            let mut broken = desc.clone().into_bytes();
            let i = 12;
            broken[i] = if broken[i] == b'q' { b'p' } else { b'q' };
            let broken = String::from_utf8(broken).unwrap();
            assert!(matches!(parse(&broken), Err(DescriptorError::Checksum(_))));
            // ...and without a checksum the bech32m checksum still catches it.
            let broken_body = broken.split('#').next().unwrap();
            assert!(matches!(
                parse(broken_body),
                Err(DescriptorError::Encoding(_))
            ));
        }
    }

    #[test]
    fn network_mismatch_is_detected() {
        let (scan, spend) = random_keys(4);
        let main =
            parse(&WalletKeys::new(scan, spend, None).to_descriptor(KeyFamily::Mainnet)).unwrap();
        let test =
            parse(&WalletKeys::new(scan, spend, None).to_descriptor(KeyFamily::Test)).unwrap();
        assert!(main.check_network(Network::Bitcoin).is_ok());
        assert!(matches!(
            main.check_network(Network::Signet),
            Err(DescriptorError::NetworkMismatch { .. })
        ));
        for n in [Network::Signet, Network::Testnet4, Network::Regtest] {
            assert!(test.check_network(n).is_ok());
        }
        assert!(test.check_network(Network::Bitcoin).is_err());
        let hex = parse(&format!(
            "hex:{}:{}",
            hex::encode(scan.secret_bytes()),
            spend
        ))
        .unwrap();
        for n in Network::ALL {
            assert!(hex.check_network(n).is_ok());
        }
    }

    #[test]
    fn spend_secrets_are_refused() {
        let (scan, spend) = random_keys(5);
        let mut data = scan.secret_bytes().to_vec();
        data.extend_from_slice(&scan.secret_bytes());
        let hrp = Hrp::parse("spspend").unwrap();
        let spspend: String = data
            .iter()
            .copied()
            .bytes_to_fes()
            .with_checksum::<Bech32m>(&hrp)
            .with_witness_version(Fe32::Q)
            .chars()
            .collect();
        assert_eq!(
            parse(&format!("sp({spspend})")),
            Err(DescriptorError::SpendPrivateKey)
        );
        let wif = bitcoin::PrivateKey::new(scan, bitcoin::Network::Bitcoin).to_wif();
        assert_eq!(
            parse(&format!("sp({wif},{wif})")),
            Err(DescriptorError::SpendPrivateKey)
        );
        let _ = spend;
    }

    #[test]
    fn hex_entry_validation() {
        let (scan, spend) = random_keys(6);
        let s = hex::encode(scan.secret_bytes());
        assert!(parse(&format!("hex:{s}:{spend}")).is_ok());
        assert!(parse(&format!("hex:{s}")).is_err());
        assert!(parse(&format!("hex:{}:{spend}", &s[..62])).is_err());
        assert!(parse(&format!("hex:{s}:{}", "04".repeat(65))).is_err());
        assert!(parse(&format!("hex:{}:{spend}", "00".repeat(32))).is_err());
    }

    #[test]
    fn debug_never_prints_the_scan_secret() {
        let (scan, spend) = random_keys(7);
        let keys = WalletKeys::new(scan, spend, None);
        let dbg = format!("{keys:?}");
        assert!(!dbg.contains(&hex::encode(scan.secret_bytes())));
        assert!(dbg.contains("<redacted>"));
    }

    #[test]
    fn fingerprint_is_short_and_stable() {
        let (scan, spend) = random_keys(8);
        let keys = WalletKeys::new(scan, spend, None);
        let a = keys.identity(Network::Signet).key_fingerprint;
        let b = keys.identity(Network::Bitcoin).key_fingerprint;
        assert_eq!(a.len(), 8);
        assert_eq!(a, b, "fingerprint does not depend on the network");
    }

    /// friglet derives `sp_address` with bdk_sp's `SilentPaymentCode::new_v0(..).to_string()`
    /// (blindbit-lib scanner.rs, bdk_sp at the friglet pin). friglet-ui must produce the same
    /// string on every network, or the identity check would compare unlike things.
    #[test]
    fn address_matches_friglets_bdk_sp_derivation() {
        use bdk_sp::encoding::SilentPaymentCode;
        let secp = Secp256k1::new();
        for seed in 1..=16u8 {
            let (scan, spend) = random_keys(seed);
            let scan_pub = PublicKey::from_secret_key(&secp, &scan);
            for n in Network::ALL {
                let theirs = SilentPaymentCode::new_v0(scan_pub, spend, n.bitcoin()).to_string();
                assert_eq!(sp_address(&scan_pub, &spend, n), theirs, "{n}");
            }
        }
        for (scan, spend, expected) in bip352_vectors() {
            let scan_pub = PublicKey::from_secret_key(&secp, &scan);
            let theirs =
                SilentPaymentCode::new_v0(scan_pub, spend, bitcoin::Network::Bitcoin).to_string();
            assert_eq!(theirs, expected);
        }
    }
}
