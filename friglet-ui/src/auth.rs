//! Web UI authentication: password mode (argon2 hash in the data dir, 12 h sessions,
//! 5 login attempts per minute per client), gateway mode (the platform's login in front)
//! and none (compose only, with a banner).

use std::collections::{HashMap, VecDeque};
use std::io;
use std::sync::{Mutex, RwLock};
use std::time::{Duration, Instant};

use argon2::Argon2;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};

use crate::config::AuthMode;
use crate::files::{self, DataDir};
use crate::util::random_hex;

pub const SESSION_COOKIE: &str = "friglet_session";
pub const CSRF_COOKIE: &str = "friglet_csrf";
pub const SESSION_TTL: Duration = Duration::from_secs(12 * 3600);
pub const LOGIN_WINDOW: Duration = Duration::from_secs(60);
pub const LOGIN_ATTEMPTS: usize = 5;
pub const MIN_PASSWORD_LEN: usize = 8;

pub fn missing_password_message() -> String {
    "refusing to start: FRIGLET_UI_AUTH=password needs a UI password and none is stored yet. \
     Fix: set FRIGLET_UI_PASSWORD (compose) or FRIGLET_UI_PASSWORD_FILE (native installs) for \
     the first start, or run `friglet-ui password set` with the password on stdin. \
     See docs/troubleshooting.md#missing-ui-password"
        .to_string()
}

/// Drop exactly one trailing newline (`\n` or `\r\n`).
pub fn trim_one_newline(s: &str) -> &str {
    s.strip_suffix("\r\n")
        .or_else(|| s.strip_suffix('\n'))
        .unwrap_or(s)
}

pub fn hash_password(password: &str) -> io::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| io::Error::other(e.to_string()))
}

pub fn verify_password(hash: &str, password: &str) -> bool {
    PasswordHash::new(hash)
        .map(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
        .unwrap_or(false)
}

/// Write the hash file (0600, atomic).
pub fn store_password(dd: &DataDir, password: &str) -> io::Result<String> {
    let hash = hash_password(password)?;
    files::write_atomic(&dd.password(), format!("{hash}\n").as_bytes())?;
    Ok(hash)
}

pub fn load_hash(dd: &DataDir) -> io::Result<Option<String>> {
    match std::fs::read_to_string(dd.password()) {
        Ok(s) if !s.trim().is_empty() => Ok(Some(s.trim().to_string())),
        Ok(_) => Ok(None),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginError {
    RateLimited { retry_after_seconds: u64 },
    BadPassword,
    NotPasswordMode,
}

pub struct Auth {
    mode: AuthMode,
    dd: DataDir,
    hash: RwLock<Option<String>>,
    sessions: Mutex<HashMap<String, Instant>>,
    attempts: Mutex<HashMap<String, VecDeque<Instant>>>,
}

impl Auth {
    /// Load or create the password hash. In password mode a missing hash with no initial
    /// password is a startup error.
    ///
    /// - `FRIGLET_UI_PASSWORD_FILE` is authoritative: read on every start (one trailing
    ///   newline trimmed); when it differs from the stored hash the hash is replaced, which
    ///   ends all sessions. Platform password resets rely on this.
    /// - `FRIGLET_UI_PASSWORD` is hashed only on the first start (no stored hash yet).
    pub fn init(
        mode: AuthMode,
        dd: &DataDir,
        initial: Option<String>,
        password_file: Option<&std::path::Path>,
    ) -> Result<Self, String> {
        let from_file = match password_file {
            Some(p) => {
                let text = std::fs::read_to_string(p).map_err(|e| {
                    format!("cannot read FRIGLET_UI_PASSWORD_FILE {}: {e}", p.display())
                })?;
                let pw = trim_one_newline(&text).to_string();
                if pw.is_empty() {
                    return Err(format!("FRIGLET_UI_PASSWORD_FILE {} is empty", p.display()));
                }
                Some(pw)
            }
            None => None,
        };
        let mut hash =
            load_hash(dd).map_err(|e| format!("cannot read the UI password hash: {e}"))?;
        if let Some(pw) = from_file {
            if !hash.as_deref().is_some_and(|h| verify_password(h, &pw)) {
                hash = Some(
                    store_password(dd, &pw)
                        .map_err(|e| format!("cannot store the UI password: {e}"))?,
                );
                tracing::info!(
                    "UI password taken from FRIGLET_UI_PASSWORD_FILE and stored (argon2)"
                );
            }
        } else if let Some(pw) = initial.filter(|p| !p.is_empty()) {
            match &hash {
                None => {
                    hash = Some(
                        store_password(dd, &pw)
                            .map_err(|e| format!("cannot store the UI password: {e}"))?,
                    );
                    tracing::info!(
                        "UI password from FRIGLET_UI_PASSWORD stored (argon2) in the data directory"
                    );
                }
                Some(h) if !verify_password(h, &pw) => tracing::warn!(
                    "FRIGLET_UI_PASSWORD differs from the stored password and is ignored (it is only used on the first start); use `friglet-ui password set` to change it"
                ),
                Some(_) => {}
            }
        }
        if mode == AuthMode::Password && hash.is_none() {
            return Err(missing_password_message());
        }
        Ok(Self {
            mode,
            dd: dd.clone(),
            hash: RwLock::new(hash),
            sessions: Mutex::new(HashMap::new()),
            attempts: Mutex::new(HashMap::new()),
        })
    }

    pub fn mode(&self) -> AuthMode {
        self.mode
    }

    /// Replace the password and end every session.
    pub fn set_password(&self, password: &str) -> io::Result<()> {
        let hash = store_password(&self.dd, password)?;
        *self.hash.write().unwrap() = Some(hash);
        self.sessions.lock().unwrap().clear();
        Ok(())
    }

    pub fn end_all_sessions(&self) {
        self.sessions.lock().unwrap().clear();
    }

    pub fn login(&self, client: &str, password: &str) -> Result<String, LoginError> {
        if self.mode != AuthMode::Password {
            return Err(LoginError::NotPasswordMode);
        }
        let now = Instant::now();
        {
            let mut attempts = self.attempts.lock().unwrap();
            let q = attempts.entry(client.to_string()).or_default();
            while q
                .front()
                .is_some_and(|t| now.duration_since(*t) >= LOGIN_WINDOW)
            {
                q.pop_front();
            }
            if q.len() >= LOGIN_ATTEMPTS {
                let oldest = *q.front().expect("non-empty");
                let wait = LOGIN_WINDOW.saturating_sub(now.duration_since(oldest));
                return Err(LoginError::RateLimited {
                    retry_after_seconds: wait.as_secs().max(1),
                });
            }
            q.push_back(now);
        }
        let ok = self
            .hash
            .read()
            .unwrap()
            .as_deref()
            .is_some_and(|h| verify_password(h, password));
        if !ok {
            return Err(LoginError::BadPassword);
        }
        let token = random_hex(32);
        let mut sessions = self.sessions.lock().unwrap();
        sessions.retain(|_, exp| *exp > now);
        sessions.insert(token.clone(), now + SESSION_TTL);
        Ok(token)
    }

    pub fn logout(&self, token: &str) {
        self.sessions.lock().unwrap().remove(token);
    }

    /// Whether a request with this session cookie is authenticated.
    pub fn is_authenticated(&self, session: Option<&str>) -> bool {
        match self.mode {
            AuthMode::Gateway | AuthMode::None => true,
            AuthMode::Password => {
                let Some(token) = session else { return false };
                let mut sessions = self.sessions.lock().unwrap();
                match sessions.get(token) {
                    Some(exp) if *exp > Instant::now() => true,
                    Some(_) => {
                        sessions.remove(token);
                        false
                    }
                    None => false,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dd() -> (tempfile::TempDir, DataDir) {
        let t = tempfile::tempdir().unwrap();
        let dd = DataDir::create(t.path().join("d")).unwrap();
        (t, dd)
    }

    #[test]
    fn password_mode_needs_a_password_on_first_start() {
        let (_t, dd) = dd();
        let err = Auth::init(AuthMode::Password, &dd, None, None)
            .err()
            .unwrap();
        assert!(err.contains("FRIGLET_UI_PASSWORD"));
        assert!(Auth::init(AuthMode::Gateway, &dd, None, None).is_ok());
        let auth = Auth::init(AuthMode::Password, &dd, Some("correct horse".into()), None).unwrap();
        let stored = std::fs::read_to_string(dd.password()).unwrap();
        assert!(stored.starts_with("$argon2"), "{stored}");
        assert!(!stored.contains("correct horse"));
        // Next start needs no env password.
        assert!(Auth::init(AuthMode::Password, &dd, None, None).is_ok());
        drop(auth);
    }

    #[test]
    fn password_file_is_authoritative_on_every_start() {
        let (t, dd) = dd();
        let f = t.path().join("pw");
        std::fs::write(&f, "from-file-pw\n").unwrap();
        let auth = Auth::init(AuthMode::Password, &dd, None, Some(&f)).unwrap();
        assert!(auth.login("c", "from-file-pw").is_ok());
        let before = std::fs::read_to_string(dd.password()).unwrap();
        // Same file: hash untouched.
        Auth::init(AuthMode::Password, &dd, None, Some(&f)).unwrap();
        assert_eq!(std::fs::read_to_string(dd.password()).unwrap(), before);
        // Platform reset: new file content replaces the hash; only one newline is trimmed.
        std::fs::write(&f, "reset pw \n").unwrap();
        let auth = Auth::init(AuthMode::Password, &dd, None, Some(&f)).unwrap();
        assert_eq!(
            auth.login("d", "from-file-pw"),
            Err(LoginError::BadPassword)
        );
        assert!(auth.login("d", "reset pw ").is_ok());
        assert_eq!(trim_one_newline("a\n\n"), "a\n");
        assert_eq!(trim_one_newline("a\r\n"), "a");
    }

    #[test]
    fn env_password_is_first_start_only() {
        let (_t, dd) = dd();
        Auth::init(AuthMode::Password, &dd, Some("first-password".into()), None).unwrap();
        let auth = Auth::init(
            AuthMode::Password,
            &dd,
            Some("second-password".into()),
            None,
        )
        .unwrap();
        assert!(auth.login("e", "first-password").is_ok());
    }

    #[test]
    fn login_sessions_and_reset() {
        let (_t, dd) = dd();
        let auth =
            Auth::init(AuthMode::Password, &dd, Some("hunter2hunter2".into()), None).unwrap();
        assert!(!auth.is_authenticated(None));
        assert_eq!(auth.login("a", "wrong"), Err(LoginError::BadPassword));
        let token = auth.login("a", "hunter2hunter2").unwrap();
        assert!(auth.is_authenticated(Some(&token)));
        assert!(!auth.is_authenticated(Some("forged")));
        auth.set_password("new-password").unwrap();
        assert!(!auth.is_authenticated(Some(&token)), "reset ends sessions");
        let token = auth.login("b", "new-password").unwrap();
        auth.logout(&token);
        assert!(!auth.is_authenticated(Some(&token)));
    }

    #[test]
    fn five_attempts_per_minute_per_client() {
        let (_t, dd) = dd();
        let auth = Auth::init(AuthMode::Password, &dd, Some("pw-pw-pw-pw".into()), None).unwrap();
        for _ in 0..5 {
            assert_eq!(auth.login("1.2.3.4", "nope"), Err(LoginError::BadPassword));
        }
        match auth.login("1.2.3.4", "pw-pw-pw-pw") {
            Err(LoginError::RateLimited {
                retry_after_seconds,
            }) => assert!(retry_after_seconds <= 60),
            other => panic!("expected rate limit, got {other:?}"),
        }
        // Another client is not affected.
        assert!(auth.login("5.6.7.8", "pw-pw-pw-pw").is_ok());
    }

    #[test]
    fn gateway_and_none_are_always_authenticated() {
        let (_t, dd) = dd();
        assert!(
            Auth::init(AuthMode::Gateway, &dd, None, None)
                .unwrap()
                .is_authenticated(None)
        );
        assert!(
            Auth::init(AuthMode::None, &dd, None, None)
                .unwrap()
                .is_authenticated(None)
        );
    }
}
