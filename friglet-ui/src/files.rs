//! The data directory: its layout, atomic writes, the exclusive lock, and the one inventory
//! of secret-bearing wallet files.
//!
//! Every wallet file friglet-ui or friglet writes is listed in [`DataDir::wallet_inventory`].
//! `forget` deletes exactly that list; the test for `forget` greps the directory for the
//! scan secret afterwards. Files that are not wallet data (UI password hash, TLS key,
//! friglet-ui's own non-secret state) stay.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

pub const CONFIG_FILE: &str = "config.toml";
pub const KEY_FILE: &str = "scan.key";
pub const STATE_FILE: &str = "scanner_state.json";
/// friglet's block-header sidecar (`blockheader::sidecar_path`: `with_extension("headers.json")`).
pub const HEADERS_FILE: &str = "scanner_state.headers.json";
/// friglet's record of the lowest birthday the state was scanned from (`with_extension("birthday.json")`).
pub const BIRTHDAY_FILE: &str = "scanner_state.birthday.json";
/// Transactions broadcast through friglet's Electrum server, until they confirm
/// (`with_extension("pending.json")`).
pub const PENDING_FILE: &str = "scanner_state.pending.json";
/// Where friglet moves a state file it could not restore (embeds the scan secret).
pub const UNREADABLE_STATE_PREFIX: &str = "scanner_state.json.unreadable-";
pub const LKG_FILE: &str = "scanner_state.json.lkg";
pub const UPGRADE_BACKUP_PREFIX: &str = "scanner_state.json.upgrade-";
pub const WALLET_FILE: &str = "wallet.json";
pub const INTENT_FILE: &str = "intent.json";
pub const PREV_DIR: &str = "prev";
pub const PREV_TMP_DIR: &str = "prev.tmp";
/// Temporary files of atomic writes start with this prefix.
pub const TMP_PREFIX: &str = ".tmp-";

pub const FRIGLET_SOCKET: &str = "friglet.sock";
pub const UI_SOCKET: &str = "friglet-ui.sock";
pub const LOCK_FILE: &str = "friglet-ui.lock";
pub const UI_STATE_FILE: &str = "ui-state.json";
pub const PASSWORD_FILE: &str = "ui-password.argon2";
pub const TLS_CERT_FILE: &str = "electrum-tls.crt";
pub const TLS_KEY_FILE: &str = "electrum-tls.key";

/// Not wallet data: UI credentials, the Electrum TLS identity and friglet-ui's own
/// bookkeeping. Reserved names that `forget`, wallet replacement and restore never touch.
pub const RESERVED_UI_FILES: [&str; 6] = [
    PASSWORD_FILE,
    TLS_CERT_FILE,
    TLS_KEY_FILE,
    UI_STATE_FILE,
    LOCK_FILE,
    UI_SOCKET,
];

/// Files copied into `prev/` by a wallet change, and restored from it.
pub const SNAPSHOT_FILES: [&str; 7] = [
    CONFIG_FILE,
    KEY_FILE,
    STATE_FILE,
    HEADERS_FILE,
    BIRTHDAY_FILE,
    PENDING_FILE,
    WALLET_FILE,
];

#[derive(Debug, Clone)]
pub struct DataDir {
    root: PathBuf,
}

/// One item of the secret-bearing inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryItem {
    pub path: PathBuf,
    pub what: &'static str,
    pub is_dir: bool,
}

impl DataDir {
    /// Use `root` without touching the filesystem.
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Create the directory if needed (0700) and tighten its mode.
    pub fn create(root: impl Into<PathBuf>) -> io::Result<Self> {
        let dd = Self::at(root);
        if !dd.root.exists() {
            fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(&dd.root)?;
        }
        let meta = fs::metadata(&dd.root)?;
        if !meta.is_dir() {
            return Err(io::Error::other(format!(
                "{} is not a directory",
                dd.root.display()
            )));
        }
        // SAFETY: getuid has no preconditions.
        let uid = unsafe { libc::getuid() };
        if meta.uid() != uid && uid != 0 {
            return Err(io::Error::other(format!(
                "data directory {} belongs to uid {}, friglet-ui runs as uid {uid}",
                dd.root.display(),
                meta.uid()
            )));
        }
        if meta.permissions().mode() & 0o777 != 0o700 {
            fs::set_permissions(&dd.root, fs::Permissions::from_mode(0o700))?;
        }
        Ok(dd)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    pub fn config(&self) -> PathBuf {
        self.path(CONFIG_FILE)
    }
    pub fn key(&self) -> PathBuf {
        self.path(KEY_FILE)
    }
    pub fn state(&self) -> PathBuf {
        self.path(STATE_FILE)
    }
    pub fn wallet(&self) -> PathBuf {
        self.path(WALLET_FILE)
    }
    pub fn intent(&self) -> PathBuf {
        self.path(INTENT_FILE)
    }
    pub fn prev(&self) -> PathBuf {
        self.path(PREV_DIR)
    }
    pub fn prev_tmp(&self) -> PathBuf {
        self.path(PREV_TMP_DIR)
    }
    pub fn friglet_socket(&self) -> PathBuf {
        self.path(FRIGLET_SOCKET)
    }
    pub fn ui_socket(&self) -> PathBuf {
        self.path(UI_SOCKET)
    }
    pub fn ui_state(&self) -> PathBuf {
        self.path(UI_STATE_FILE)
    }
    pub fn password(&self) -> PathBuf {
        self.path(PASSWORD_FILE)
    }

    /// Every secret-bearing (or wallet-identifying) item that exists now.
    ///
    /// - `scan.key`: the scan secret.
    /// - `config.toml`: friglet config (spend public key, birthday, peer).
    /// - `scanner_state.json`: friglet state; blindbit-lib embeds the scan secret in it.
    /// - `scanner_state.headers.json`: block headers friglet fetched for this wallet.
    /// - `scanner_state.birthday.json`, `scanner_state.pending.json`: friglet's birthday record
    ///   and the transactions broadcast through its Electrum server.
    /// - `scanner_state.json.lkg`, `scanner_state.json.upgrade-*`: state copies.
    /// - `scanner_state.json.bak`, `scanner_state.json.unreadable-*`, `*.tmp`: state friglet set
    ///   aside (birthday moved back, unreadable file) or is writing.
    /// - `wallet.json`: address, fingerprint, birthday.
    /// - `intent.json`: an unfinished wallet change.
    /// - `prev/`, `prev.tmp/`: the previous wallet's snapshot.
    /// - `.tmp-*`: leftovers of interrupted atomic writes (may hold any of the above).
    pub fn wallet_inventory(&self) -> io::Result<Vec<InventoryItem>> {
        let fixed: [(&str, &'static str); 13] = [
            (KEY_FILE, "scan secret"),
            (CONFIG_FILE, "friglet config"),
            (STATE_FILE, "friglet scanner state (embeds the scan secret)"),
            (HEADERS_FILE, "friglet header cache"),
            (BIRTHDAY_FILE, "friglet birthday record"),
            (PENDING_FILE, "friglet unconfirmed broadcasts"),
            (
                "scanner_state.pending.json.tmp",
                "friglet unconfirmed broadcasts temp file",
            ),
            (
                "scanner_state.json.tmp",
                "friglet scanner state temp file (embeds the scan secret)",
            ),
            (
                "scanner_state.json.bak",
                "scanner state set aside for a rescan (embeds the scan secret)",
            ),
            (LKG_FILE, "last-known-good scanner state"),
            (WALLET_FILE, "wallet identity"),
            (INTENT_FILE, "wallet change intent"),
            ("config.toml.tmp", "friglet config temp file"),
        ];
        let mut out = Vec::new();
        for (name, what) in fixed {
            let p = self.path(name);
            if p.exists() {
                out.push(InventoryItem {
                    path: p,
                    what,
                    is_dir: false,
                });
            }
        }
        for (name, what) in [
            (PREV_DIR, "previous wallet snapshot"),
            (PREV_TMP_DIR, "previous wallet snapshot (partial)"),
        ] {
            let p = self.path(name);
            if p.exists() {
                out.push(InventoryItem {
                    path: p,
                    what,
                    is_dir: true,
                });
            }
        }
        if self.root.exists() {
            for entry in fs::read_dir(&self.root)? {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with(UPGRADE_BACKUP_PREFIX) {
                    out.push(InventoryItem {
                        path: entry.path(),
                        what: "scanner state upgrade backup",
                        is_dir: false,
                    });
                } else if name.starts_with(UNREADABLE_STATE_PREFIX) {
                    out.push(InventoryItem {
                        path: entry.path(),
                        what: "unreadable scanner state set aside (embeds the scan secret)",
                        is_dir: false,
                    });
                } else if name.starts_with(TMP_PREFIX) {
                    out.push(InventoryItem {
                        path: entry.path(),
                        what: "interrupted write",
                        is_dir: entry.file_type()?.is_dir(),
                    });
                }
            }
        }
        Ok(out)
    }

    /// Delete every inventory item. friglet must not be running.
    pub fn forget(&self) -> io::Result<Vec<InventoryItem>> {
        let items = self.wallet_inventory()?;
        for item in &items {
            if item.is_dir {
                fs::remove_dir_all(&item.path)?;
            } else {
                remove_if_exists(&item.path)?;
            }
        }
        fsync_dir(&self.root)?;
        Ok(items)
    }

    /// Whether a wallet is configured (identity file and key both present).
    pub fn has_wallet(&self) -> bool {
        self.wallet().is_file() && self.key().is_file()
    }
}

pub fn remove_if_exists(path: &Path) -> io::Result<()> {
    match fs::remove_file(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

pub fn fsync_dir(dir: &Path) -> io::Result<()> {
    File::open(dir)?.sync_all()
}

/// Write `bytes` to `path` atomically: temp file in the same directory (created 0600 with
/// O_EXCL), write, fsync, rename over `path`, fsync the directory.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::other("path has no file name"))?
        .to_string_lossy();
    let tmp = dir.join(format!(
        "{TMP_PREFIX}{name}-{}-{}",
        std::process::id(),
        rand::random::<u32>()
    ));
    let result = (|| {
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, path)?;
        fsync_dir(dir)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

pub fn write_json<T: Serialize>(path: &Path, value: &T) -> io::Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(io::Error::other)?;
    bytes.push(b'\n');
    write_atomic(path, &bytes)
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> io::Result<Option<T>> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map(Some).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{}: {e}", path.display()),
            )
        }),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Copy `from` to `to` atomically with 0600.
pub fn copy_atomic(from: &Path, to: &Path) -> io::Result<()> {
    let bytes = fs::read(from)?;
    write_atomic(to, &bytes)
}

/// Exclusive advisory lock (`flock`) on `<data>/friglet-ui.lock`. Held by the daemon for
/// its lifetime and by an offline `setup` while it stages files.
#[derive(Debug)]
pub struct DataDirLock {
    _file: File,
}

impl DataDirLock {
    /// `Ok(None)` when another process holds the lock.
    pub fn try_acquire(dd: &DataDir) -> io::Result<Option<Self>> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(dd.path(LOCK_FILE))?;
        match file.try_lock() {
            Ok(()) => Ok(Some(Self { _file: file })),
            Err(fs::TryLockError::WouldBlock) => Ok(None),
            Err(fs::TryLockError::Error(e)) => Err(e),
        }
    }
}

/// Recursively grep `dir` for `needle` (used by tests and the self-check after forget).
pub fn dir_contains(dir: &Path, needle: &[u8]) -> io::Result<Vec<PathBuf>> {
    let mut hits = Vec::new();
    if !dir.exists() {
        return Ok(hits);
    }
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let ft = entry.file_type()?;
        if ft.is_dir() {
            hits.extend(dir_contains(&path, needle)?);
        } else if ft.is_file() {
            let bytes = fs::read(&path)?;
            if !needle.is_empty() && bytes.windows(needle.len()).any(|w| w == needle) {
                hits.push(path);
            }
        }
    }
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_sets_0700_and_atomic_write_is_0600() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("data");
        let dd = DataDir::create(&root).unwrap();
        assert_eq!(
            fs::metadata(&root).unwrap().permissions().mode() & 0o777,
            0o700
        );
        // Loosened modes are tightened again.
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).unwrap();
        DataDir::create(&root).unwrap();
        assert_eq!(
            fs::metadata(&root).unwrap().permissions().mode() & 0o777,
            0o700
        );

        write_atomic(&dd.key(), b"secret\n").unwrap();
        assert_eq!(fs::read(dd.key()).unwrap(), b"secret\n");
        assert_eq!(
            fs::metadata(dd.key()).unwrap().permissions().mode() & 0o777,
            0o600
        );
        write_atomic(&dd.key(), b"other\n").unwrap();
        assert_eq!(fs::read(dd.key()).unwrap(), b"other\n");
        // No temp files left behind.
        let leftovers: Vec<_> = fs::read_dir(&root)
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with(TMP_PREFIX))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn lock_is_exclusive() {
        let tmp = tempfile::tempdir().unwrap();
        let dd = DataDir::create(tmp.path().join("d")).unwrap();
        let first = DataDirLock::try_acquire(&dd).unwrap();
        assert!(first.is_some());
        assert!(DataDirLock::try_acquire(&dd).unwrap().is_none());
        drop(first);
        assert!(DataDirLock::try_acquire(&dd).unwrap().is_some());
    }

    #[test]
    fn forget_removes_the_whole_inventory_and_keeps_ui_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dd = DataDir::create(tmp.path().join("d")).unwrap();
        let secret = b"5ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7e75ec7";
        for name in [
            KEY_FILE,
            STATE_FILE,
            LKG_FILE,
            CONFIG_FILE,
            HEADERS_FILE,
            WALLET_FILE,
            INTENT_FILE,
            BIRTHDAY_FILE,
            PENDING_FILE,
            "scanner_state.pending.json.tmp",
            "scanner_state.json.tmp",
            "scanner_state.json.bak",
            "scanner_state.json.unreadable-1790000000",
            "scanner_state.json.upgrade-93f6a92",
            ".tmp-scan.key-1-2",
        ] {
            write_atomic(&dd.path(name), secret).unwrap();
        }
        fs::create_dir(dd.prev()).unwrap();
        write_atomic(&dd.prev().join(KEY_FILE), secret).unwrap();
        fs::create_dir(dd.prev_tmp()).unwrap();
        write_atomic(&dd.prev_tmp().join(STATE_FILE), secret).unwrap();
        for name in RESERVED_UI_FILES {
            write_atomic(&dd.path(name), b"ui data").unwrap();
        }

        assert!(!dir_contains(dd.root(), secret).unwrap().is_empty());
        let removed = dd.forget().unwrap();
        assert_eq!(removed.len(), 17);
        assert!(dir_contains(dd.root(), secret).unwrap().is_empty());
        for name in RESERVED_UI_FILES {
            assert!(dd.path(name).exists(), "{name} survives forget");
            assert!(
                !SNAPSHOT_FILES.contains(&name),
                "{name} is never snapshotted or restored"
            );
        }
        assert!(!dd.has_wallet());
    }
}
