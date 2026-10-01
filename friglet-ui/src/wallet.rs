//! Wallet files and wallet changes as a durable transaction.
//!
//! A change (first setup, replace, restore-previous, forget) writes `intent.json` first, then
//! snapshots the live wallet files into `prev/`, then writes the new set. friglet-ui then
//! starts friglet and verifies the identity; only then is the intent removed. On boot an
//! unfinished intent is rolled back (crash before the new set was complete) or resumed
//! (new set written, verification pending). The offline `setup` CLI stops at "written" and
//! reports "staged, verification pending".

use std::fs;
use std::io;
use std::net::SocketAddr;
use std::os::unix::fs::DirBuilderExt;
use std::str::FromStr;

use bitcoin::secp256k1::{PublicKey, SecretKey};
use serde::{Deserialize, Serialize};

use crate::descriptor::{WalletIdentity, WalletKeys};
use crate::files::{self, DataDir, SNAPSHOT_FILES};
use crate::network::Network;
use crate::util::now_rfc3339;

/// `wallet.json`: the wallet's public identity. No secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletMeta {
    pub sp_address: String,
    pub key_fingerprint: String,
    pub network: Network,
    pub birthday_height: u64,
    pub spend_pubkey: String,
    pub created_at: String,
    #[serde(default)]
    pub verified_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TxKind {
    Setup,
    Replace,
    Restore,
    Forget,
}

impl TxKind {
    /// Kind name for the API (`replace|forget|restore`; a first setup is a replace).
    pub fn api_kind(self) -> &'static str {
        match self {
            TxKind::Forget => "forget",
            TxKind::Restore => "restore",
            TxKind::Setup | TxKind::Replace => "replace",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TxStep {
    Begin,
    Snapshotted,
    Written,
    Verified,
}

impl TxStep {
    pub fn describe(self) -> &'static str {
        match self {
            TxStep::Begin => "preparing",
            TxStep::Snapshotted => "previous wallet saved",
            TxStep::Written => "new wallet written, verifying identity",
            TxStep::Verified => "identity verified, waiting for the first saved scan",
        }
    }
}

/// `intent.json`. No secret: the new key is already in `scan.key` once step is `written`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Intent {
    pub kind: TxKind,
    pub step: TxStep,
    pub started_at: String,
    #[serde(default)]
    pub sp_address: Option<String>,
    #[serde(default)]
    pub birthday_height: Option<u64>,
    /// Written by the offline CLI; verification happens at the next daemon start.
    #[serde(default)]
    pub staged: bool,
}

pub fn load_wallet(dd: &DataDir) -> io::Result<Option<WalletMeta>> {
    files::read_json(&dd.wallet())
}

pub fn load_intent(dd: &DataDir) -> io::Result<Option<Intent>> {
    files::read_json(&dd.intent())
}

fn save_intent(dd: &DataDir, intent: &Intent) -> io::Result<()> {
    files::write_json(&dd.intent(), intent)
}

/// The configured wallet's keys (key file plus `wallet.json`).
pub fn load_keys(dd: &DataDir) -> io::Result<Option<(WalletKeys, WalletMeta)>> {
    let Some(meta) = load_wallet(dd)? else {
        return Ok(None);
    };
    let text = match fs::read_to_string(dd.key()) {
        Ok(t) => t,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let secret = SecretKey::from_str(text.trim()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "scan.key does not hold a valid key",
        )
    })?;
    let spend = PublicKey::from_str(&meta.spend_pubkey).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "wallet.json spend_pubkey is invalid",
        )
    })?;
    Ok(Some((WalletKeys::new(secret, spend, None), meta)))
}

/// Birthday bounds: taproot activation for the network up to the node tip when known.
pub fn validate_birthday(
    network: Network,
    birthday: u64,
    node_tip: Option<u64>,
) -> Result<(), String> {
    let min = network.min_birthday();
    if birthday < min {
        return Err(format!(
            "birthday {birthday} is before taproot activation on {network} ({min}); Silent Payments cannot exist before it"
        ));
    }
    if let Some(tip) = node_tip
        && birthday > tip
    {
        return Err(format!(
            "birthday {birthday} is after your node's tip ({tip}); use the birthday height your wallet shows"
        ));
    }
    Ok(())
}

/// What a setup request means against the current wallet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupPlan {
    /// No wallet yet.
    Fresh,
    /// Same keys and birthday: nothing to do.
    NoOp,
    /// A different wallet or birthday: needs `replace`.
    Replace { current: WalletMeta },
}

pub fn plan_setup(dd: &DataDir, identity: &WalletIdentity, birthday: u64) -> io::Result<SetupPlan> {
    match load_wallet(dd)? {
        None => Ok(SetupPlan::Fresh),
        Some(cur)
            if cur.sp_address == identity.sp_address
                && cur.birthday_height == birthday
                && dd.key().exists() =>
        {
            Ok(SetupPlan::NoOp)
        }
        Some(cur) => Ok(SetupPlan::Replace { current: cur }),
    }
}

fn snapshot_live_into(dd: &DataDir, dir: &std::path::Path) -> io::Result<()> {
    if dir.exists() {
        fs::remove_dir_all(dir)?;
    }
    fs::DirBuilder::new().mode(0o700).create(dir)?;
    for name in SNAPSHOT_FILES {
        let src = dd.path(name);
        if src.is_file() {
            files::copy_atomic(&src, &dir.join(name))?;
        }
    }
    files::fsync_dir(dir)?;
    files::fsync_dir(dd.root())
}

fn replace_prev_with(dd: &DataDir, tmp: &std::path::Path) -> io::Result<()> {
    let prev = dd.prev();
    if prev.exists() {
        fs::remove_dir_all(&prev)?;
    }
    fs::rename(tmp, &prev)?;
    files::fsync_dir(dd.root())
}

/// Remove the live wallet's scanner state so friglet starts fresh.
fn remove_state_files(dd: &DataDir) -> io::Result<()> {
    for name in [
        files::STATE_FILE,
        files::HEADERS_FILE,
        files::LKG_FILE,
        files::CONFIG_FILE,
    ] {
        files::remove_if_exists(&dd.path(name))?;
    }
    files::fsync_dir(dd.root())
}

/// Run a setup or replace up to `written`. friglet must not be running.
pub fn stage(
    dd: &DataDir,
    keys: &WalletKeys,
    network: Network,
    birthday: u64,
    kind: TxKind,
    staged: bool,
) -> io::Result<Intent> {
    // A change on top of an unfinished one keeps the snapshot of the last good wallet.
    let keep_prev = matches!(load_intent(dd), Ok(Some(i)) if i.step >= TxStep::Snapshotted && i.kind != TxKind::Forget);
    let identity = keys.identity(network);
    let mut intent = Intent {
        kind,
        step: TxStep::Begin,
        started_at: now_rfc3339(),
        sp_address: Some(identity.sp_address.clone()),
        birthday_height: Some(birthday),
        staged,
    };
    save_intent(dd, &intent)?;

    if !keep_prev {
        let tmp = dd.prev_tmp();
        snapshot_live_into(dd, &tmp)?;
        replace_prev_with(dd, &tmp)?;
    }
    intent.step = TxStep::Snapshotted;
    save_intent(dd, &intent)?;

    let mut key = keys.scan_secret_hex();
    key.push('\n');
    files::write_atomic(&dd.key(), key.as_bytes())?;
    let meta = WalletMeta {
        sp_address: identity.sp_address,
        key_fingerprint: identity.key_fingerprint,
        network,
        birthday_height: birthday,
        spend_pubkey: keys.spend_pubkey_hex(),
        created_at: now_rfc3339(),
        verified_at: None,
    };
    files::write_json(&dd.wallet(), &meta)?;
    remove_state_files(dd)?;
    intent.step = TxStep::Written;
    save_intent(dd, &intent)?;
    Ok(intent)
}

/// Swap the `prev/` snapshot back in, keeping the current wallet as the new `prev/`, up to
/// `written`. friglet must not be running.
pub fn stage_restore(dd: &DataDir) -> io::Result<Option<Intent>> {
    let prev = dd.prev();
    let Some(prev_meta): Option<WalletMeta> = files::read_json(&prev.join(files::WALLET_FILE))?
    else {
        return Ok(None);
    };
    if !prev.join(files::KEY_FILE).is_file() {
        return Ok(None);
    }
    let mut intent = Intent {
        kind: TxKind::Restore,
        step: TxStep::Begin,
        started_at: now_rfc3339(),
        sp_address: Some(prev_meta.sp_address.clone()),
        birthday_height: Some(prev_meta.birthday_height),
        staged: false,
    };
    save_intent(dd, &intent)?;
    let tmp = dd.prev_tmp();
    snapshot_live_into(dd, &tmp)?;
    intent.step = TxStep::Snapshotted;
    save_intent(dd, &intent)?;
    for name in SNAPSHOT_FILES {
        let src = prev.join(name);
        let dst = dd.path(name);
        if src.is_file() {
            files::copy_atomic(&src, &dst)?;
        } else {
            files::remove_if_exists(&dst)?;
        }
    }
    let mut meta = prev_meta;
    meta.verified_at = None;
    files::write_json(&dd.wallet(), &meta)?;
    replace_prev_with(dd, &tmp)?;
    intent.step = TxStep::Written;
    save_intent(dd, &intent)?;
    Ok(Some(intent))
}

/// Restore the live wallet files from `prev/` (rollback of an interrupted replace).
fn rollback_from_prev(dd: &DataDir) -> io::Result<()> {
    let prev = dd.prev();
    for name in SNAPSHOT_FILES {
        let src = prev.join(name);
        let dst = dd.path(name);
        if src.is_file() {
            files::copy_atomic(&src, &dst)?;
        } else {
            files::remove_if_exists(&dst)?;
        }
    }
    if prev.exists() {
        fs::remove_dir_all(&prev)?;
    }
    files::fsync_dir(dd.root())
}

pub fn mark_step(dd: &DataDir, step: TxStep) -> io::Result<()> {
    if let Some(mut intent) = load_intent(dd)? {
        intent.step = step;
        save_intent(dd, &intent)?;
    }
    Ok(())
}

/// Identity verified and first progress persisted: record it and drop the intent.
/// `prev/` stays (deleted later); an empty `prev/` (first setup) is removed now.
pub fn commit(dd: &DataDir) -> io::Result<()> {
    if let Some(mut meta) = load_wallet(dd)?
        && meta.verified_at.is_none()
    {
        meta.verified_at = Some(now_rfc3339());
        files::write_json(&dd.wallet(), &meta)?;
    }
    let prev = dd.prev();
    if prev.is_dir() && fs::read_dir(&prev)?.next().is_none() {
        fs::remove_dir(&prev)?;
    }
    files::remove_if_exists(&dd.intent())?;
    files::fsync_dir(dd.root())
}

/// Mark the configured wallet verified without a transaction (normal restarts).
pub fn mark_verified(dd: &DataDir) -> io::Result<()> {
    if let Some(mut meta) = load_wallet(dd)?
        && meta.verified_at.is_none()
    {
        meta.verified_at = Some(now_rfc3339());
        files::write_json(&dd.wallet(), &meta)?;
    }
    Ok(())
}

/// Forget: delete the whole inventory (intent written first so a crash finishes the job).
pub fn forget(dd: &DataDir) -> io::Result<()> {
    let intent = Intent {
        kind: TxKind::Forget,
        step: TxStep::Begin,
        started_at: now_rfc3339(),
        sp_address: None,
        birthday_height: None,
        staged: false,
    };
    save_intent(dd, &intent)?;
    dd.forget().map(|_| ())
}

/// Outcome of boot recovery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Recovery {
    Clean,
    Forgot,
    RolledBack(TxKind),
    /// New wallet written; verify it on this start.
    Resume(Intent),
}

pub fn recover(dd: &DataDir) -> io::Result<Recovery> {
    let intent = match load_intent(dd) {
        Ok(Some(i)) => i,
        Ok(None) => {
            // A leftover partial snapshot without an intent is garbage.
            if dd.prev_tmp().exists() {
                fs::remove_dir_all(dd.prev_tmp())?;
            }
            return Ok(Recovery::Clean);
        }
        Err(e) if e.kind() == io::ErrorKind::InvalidData => {
            // A torn intent can only come from a crash in `Begin`: nothing changed yet.
            files::remove_if_exists(&dd.intent())?;
            return Ok(Recovery::RolledBack(TxKind::Replace));
        }
        Err(e) => return Err(e),
    };
    if intent.kind == TxKind::Forget {
        dd.forget()?;
        return Ok(Recovery::Forgot);
    }
    match (intent.kind, intent.step) {
        (_, TxStep::Written | TxStep::Verified) => Ok(Recovery::Resume(intent)),
        (TxKind::Restore, _) => {
            // Live files untouched until `written` (the copy into place happens after
            // `snapshotted` but is idempotent from prev/, which is intact): finish it.
            if intent.step == TxStep::Snapshotted
                && let Some(i) = stage_restore_finish(dd)?
            {
                return Ok(Recovery::Resume(i));
            }
            if dd.prev_tmp().exists() {
                fs::remove_dir_all(dd.prev_tmp())?;
            }
            files::remove_if_exists(&dd.intent())?;
            Ok(Recovery::RolledBack(TxKind::Restore))
        }
        (kind, TxStep::Begin) => {
            if dd.prev_tmp().exists() {
                fs::remove_dir_all(dd.prev_tmp())?;
            }
            files::remove_if_exists(&dd.intent())?;
            Ok(Recovery::RolledBack(kind))
        }
        (kind, TxStep::Snapshotted) => {
            rollback_from_prev(dd)?;
            files::remove_if_exists(&dd.intent())?;
            Ok(Recovery::RolledBack(kind))
        }
    }
}

/// Finish a restore interrupted after the live set was saved to `prev.tmp/`.
fn stage_restore_finish(dd: &DataDir) -> io::Result<Option<Intent>> {
    let Some(mut intent) = load_intent(dd)? else {
        return Ok(None);
    };
    let tmp = dd.prev_tmp();
    let prev = dd.prev();
    if !tmp.is_dir() || !prev.join(files::KEY_FILE).is_file() {
        return Ok(None);
    }
    for name in SNAPSHOT_FILES {
        let src = prev.join(name);
        let dst = dd.path(name);
        if src.is_file() {
            files::copy_atomic(&src, &dst)?;
        } else {
            files::remove_if_exists(&dst)?;
        }
    }
    if let Some(mut meta) = load_wallet(dd)? {
        meta.verified_at = None;
        files::write_json(&dd.wallet(), &meta)?;
    }
    replace_prev_with(dd, &tmp)?;
    intent.step = TxStep::Written;
    save_intent(dd, &intent)?;
    Ok(Some(intent))
}

/// Delete `prev/` (after the user confirmed, or 24 h after verified progress).
pub fn delete_prev(dd: &DataDir) -> io::Result<()> {
    if dd.prev().exists() {
        fs::remove_dir_all(dd.prev())?;
        files::fsync_dir(dd.root())?;
    }
    Ok(())
}

/// friglet's `config.toml` for this wallet. Rewritten before every start (the peer is
/// re-resolved each time). Holds no secret: the scan key is in `scan.key`.
pub fn render_friglet_config(
    dd: &DataDir,
    meta: &WalletMeta,
    oracle_url: &str,
    peer: SocketAddr,
) -> Result<String, String> {
    let cfg = friglet_ipc::DaemonConfig {
        network: meta.network.as_str().to_string(),
        oracle_url: oracle_url.to_string(),
        p2p_node_addr: Some(peer.to_string()),
        start_height: Some(meta.birthday_height),
        spend_pubkey: Some(meta.spend_pubkey.clone()),
        max_label_num: 0,
        http_addr: crate::FRIGLET_HTTP_ADDR.to_string(),
        electrum_addr: crate::FRIGLET_ELECTRUM_ADDR.to_string(),
        state_file: dd.state(),
        log_level: "info".to_string(),
        key_file: Some(dd.key()),
        control_socket: Some(dd.friglet_socket().to_string_lossy().into_owned()),
    };
    toml::to_string_pretty(&cfg).map_err(|e| e.to_string())
}

/// The fields of friglet's state file friglet-ui reads (it embeds the scan secret; never
/// logged or returned).
#[derive(Debug, Default, Deserialize)]
struct StateFileView {
    #[serde(default)]
    secret_scan_hex: Option<String>,
    #[serde(default)]
    public_spend_hex: Option<String>,
    #[serde(default)]
    last_scanned_block_height: Option<u64>,
}

/// What the state file says, compared with the configured wallet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateFileCheck {
    pub persisted_height: Option<u64>,
    /// Keys in the state file are the configured keys.
    pub matches_wallet: bool,
}

pub fn check_state_file(dd: &DataDir, keys: &WalletKeys) -> io::Result<Option<StateFileCheck>> {
    let bytes = match fs::read(dd.state()) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    let view: StateFileView = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => {
            return Ok(Some(StateFileCheck {
                persisted_height: None,
                matches_wallet: false,
            }));
        }
    };
    let scan_ok = view
        .secret_scan_hex
        .as_deref()
        .is_none_or(|h| h.eq_ignore_ascii_case(&keys.scan_secret_hex()));
    let spend_ok = view
        .public_spend_hex
        .as_deref()
        .is_none_or(|h| h.eq_ignore_ascii_case(&keys.spend_pubkey_hex()));
    Ok(Some(StateFileCheck {
        persisted_height: view.last_scanned_block_height,
        matches_wallet: scan_ok && spend_ok,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptor::tests::random_keys;

    fn dd() -> (tempfile::TempDir, DataDir) {
        let t = tempfile::tempdir().unwrap();
        let dd = DataDir::create(t.path().join("data")).unwrap();
        (t, dd)
    }

    fn keys(seed: u8) -> WalletKeys {
        let (s, p) = random_keys(seed);
        WalletKeys::new(s, p, None)
    }

    #[test]
    fn first_setup_writes_key_0600_and_identity_and_commits() {
        let (_t, dd) = dd();
        let k = keys(1);
        let id = k.identity(Network::Signet);
        assert_eq!(plan_setup(&dd, &id, 100).unwrap(), SetupPlan::Fresh);
        let intent = stage(&dd, &k, Network::Signet, 100, TxKind::Setup, false).unwrap();
        assert_eq!(intent.step, TxStep::Written);
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(dd.key()).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(dd.prev()).unwrap().permissions().mode() & 0o777,
            0o700
        );
        let (loaded, meta) = load_keys(&dd).unwrap().unwrap();
        assert_eq!(loaded.identity(Network::Signet), id);
        assert_eq!(meta.birthday_height, 100);
        assert!(
            !fs::read_to_string(dd.wallet())
                .unwrap()
                .contains(&k.scan_secret_hex())
        );
        assert!(
            !fs::read_to_string(dd.intent())
                .unwrap()
                .contains(&k.scan_secret_hex())
        );
        assert_eq!(plan_setup(&dd, &id, 100).unwrap(), SetupPlan::NoOp);
        assert!(matches!(
            plan_setup(&dd, &id, 99).unwrap(),
            SetupPlan::Replace { .. }
        ));
        commit(&dd).unwrap();
        assert!(!dd.intent().exists());
        assert!(
            !dd.prev().exists(),
            "empty prev/ of a first setup is removed"
        );
        assert!(load_wallet(&dd).unwrap().unwrap().verified_at.is_some());
    }

    #[test]
    fn replace_snapshots_previous_and_restore_swaps_back() {
        let (_t, dd) = dd();
        let a = keys(1);
        let b = keys(2);
        stage(&dd, &a, Network::Signet, 100, TxKind::Setup, false).unwrap();
        commit(&dd).unwrap();
        files::write_atomic(&dd.state(), b"{\"last_scanned_block_height\":150}").unwrap();

        stage(&dd, &b, Network::Signet, 200, TxKind::Replace, false).unwrap();
        assert!(!dd.state().exists(), "new wallet starts from a fresh state");
        assert!(dd.prev().join(files::STATE_FILE).exists());
        assert_eq!(
            load_keys(&dd).unwrap().unwrap().0.identity(Network::Signet),
            b.identity(Network::Signet)
        );
        commit(&dd).unwrap();
        assert!(
            dd.prev().exists(),
            "prev/ kept after commit until confirmed or 24 h"
        );

        let intent = stage_restore(&dd).unwrap().unwrap();
        assert_eq!(intent.kind, TxKind::Restore);
        assert_eq!(
            load_keys(&dd).unwrap().unwrap().0.identity(Network::Signet),
            a.identity(Network::Signet)
        );
        assert!(dd.state().exists(), "restore brings the old state back");
        // ...and the replaced wallet is now the previous one.
        let prev_meta: WalletMeta = files::read_json(&dd.prev().join(files::WALLET_FILE))
            .unwrap()
            .unwrap();
        assert_eq!(prev_meta.sp_address, b.identity(Network::Signet).sp_address);
    }

    #[test]
    fn boot_recovery_rolls_back_before_written_and_resumes_after() {
        let (_t, dd) = dd();
        let a = keys(1);
        stage(&dd, &a, Network::Signet, 100, TxKind::Setup, false).unwrap();
        commit(&dd).unwrap();
        let live_key = fs::read(dd.key()).unwrap();

        // Crash right after the snapshot, with a half-written new key.
        let b = keys(2);
        stage(&dd, &b, Network::Signet, 200, TxKind::Replace, false).unwrap();
        mark_step(&dd, TxStep::Snapshotted).unwrap();
        // prev/ holds wallet a; live holds (part of) wallet b.
        assert_eq!(recover(&dd).unwrap(), Recovery::RolledBack(TxKind::Replace));
        assert_eq!(fs::read(dd.key()).unwrap(), live_key);
        assert!(!dd.intent().exists());

        // Crash after the new set was written: resume verification.
        stage(&dd, &b, Network::Signet, 200, TxKind::Replace, true).unwrap();
        match recover(&dd).unwrap() {
            Recovery::Resume(i) => assert!(i.staged),
            other => panic!("expected resume, got {other:?}"),
        }

        // Crash in Begin: nothing changed, intent dropped.
        commit(&dd).unwrap();
        let before = fs::read(dd.key()).unwrap();
        save_intent(
            &dd,
            &Intent {
                kind: TxKind::Replace,
                step: TxStep::Begin,
                started_at: now_rfc3339(),
                sp_address: None,
                birthday_height: None,
                staged: false,
            },
        )
        .unwrap();
        fs::create_dir(dd.prev_tmp()).unwrap();
        assert_eq!(recover(&dd).unwrap(), Recovery::RolledBack(TxKind::Replace));
        assert_eq!(fs::read(dd.key()).unwrap(), before);
        assert!(!dd.prev_tmp().exists());
        assert_eq!(recover(&dd).unwrap(), Recovery::Clean);
    }

    #[test]
    fn a_change_on_top_of_an_unverified_one_keeps_the_good_snapshot() {
        let (_t, dd) = dd();
        let a = keys(1);
        stage(&dd, &a, Network::Signet, 100, TxKind::Setup, false).unwrap();
        commit(&dd).unwrap();
        stage(&dd, &keys(2), Network::Signet, 100, TxKind::Replace, false).unwrap();
        // Not verified (e.g. mismatch); the user enters another wallet.
        stage(&dd, &keys(3), Network::Signet, 100, TxKind::Replace, false).unwrap();
        let prev_meta: WalletMeta = files::read_json(&dd.prev().join(files::WALLET_FILE))
            .unwrap()
            .unwrap();
        assert_eq!(prev_meta.sp_address, a.identity(Network::Signet).sp_address);
    }

    #[test]
    fn first_setup_rollback_returns_to_unconfigured() {
        let (_t, dd) = dd();
        stage(&dd, &keys(3), Network::Signet, 100, TxKind::Setup, false).unwrap();
        mark_step(&dd, TxStep::Snapshotted).unwrap();
        assert_eq!(recover(&dd).unwrap(), Recovery::RolledBack(TxKind::Setup));
        assert!(!dd.has_wallet());
        assert!(load_wallet(&dd).unwrap().is_none());
    }

    #[test]
    fn interrupted_forget_finishes_on_boot() {
        let (_t, dd) = dd();
        let k = keys(4);
        stage(&dd, &k, Network::Signet, 100, TxKind::Setup, false).unwrap();
        commit(&dd).unwrap();
        save_intent(
            &dd,
            &Intent {
                kind: TxKind::Forget,
                step: TxStep::Begin,
                started_at: now_rfc3339(),
                sp_address: None,
                birthday_height: None,
                staged: false,
            },
        )
        .unwrap();
        assert_eq!(recover(&dd).unwrap(), Recovery::Forgot);
        assert!(
            files::dir_contains(dd.root(), k.scan_secret_hex().as_bytes())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn birthday_bounds() {
        assert!(validate_birthday(Network::Bitcoin, 709_631, None).is_err());
        assert!(validate_birthday(Network::Bitcoin, 709_632, Some(900_000)).is_ok());
        assert!(validate_birthday(Network::Bitcoin, 900_001, Some(900_000)).is_err());
        assert!(validate_birthday(Network::Signet, 0, None).is_err());
        assert!(validate_birthday(Network::Regtest, 0, Some(0)).is_ok());
    }

    #[test]
    fn friglet_config_has_explicit_paths_and_no_secret() {
        let (_t, dd) = dd();
        let k = keys(5);
        stage(&dd, &k, Network::Signet, 270_000, TxKind::Setup, false).unwrap();
        let meta = load_wallet(&dd).unwrap().unwrap();
        let toml = render_friglet_config(
            &dd,
            &meta,
            "https://signet.oracle.setor.dev",
            "10.0.0.1:38333".parse().unwrap(),
        )
        .unwrap();
        let cfg: friglet_ipc::DaemonConfig = toml::from_str(&toml).unwrap();
        assert_eq!(cfg.network, "signet");
        assert_eq!(cfg.electrum_addr, "127.0.0.1:50001");
        assert_eq!(cfg.http_addr, "127.0.0.1:8095");
        assert_eq!(cfg.start_height, Some(270_000));
        assert_eq!(cfg.p2p_node_addr.as_deref(), Some("10.0.0.1:38333"));
        assert_eq!(cfg.state_file, dd.state());
        assert_eq!(cfg.key_file, Some(dd.key()));
        assert_eq!(
            cfg.control_socket.as_deref(),
            Some(dd.friglet_socket().to_str().unwrap())
        );
        assert!(!toml.contains(&k.scan_secret_hex()));
    }

    #[test]
    fn state_file_check_detects_other_keys() {
        let (_t, dd) = dd();
        let k = keys(6);
        assert_eq!(check_state_file(&dd, &k).unwrap(), None);
        let json = serde_json::json!({
            "secret_scan_hex": k.scan_secret_hex(),
            "public_spend_hex": k.spend_pubkey_hex(),
            "last_scanned_block_height": 123,
            "other": [1, 2, 3]
        });
        files::write_json(&dd.state(), &json).unwrap();
        let c = check_state_file(&dd, &k).unwrap().unwrap();
        assert!(c.matches_wallet);
        assert_eq!(c.persisted_height, Some(123));
        assert!(
            !check_state_file(&dd, &keys(7))
                .unwrap()
                .unwrap()
                .matches_wallet
        );
    }
}
