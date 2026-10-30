//! End-to-end tests of the friglet-ui binary against `examples/fake_friglet.rs` (a friglet
//! stand-in speaking the real friglet-ipc protocol) and a fake Bitcoin P2P peer. No network.
//!
//! `cargo test` builds the example; `cargo test --test supervisor` alone needs
//! `cargo build --examples` first.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use bitcoin::consensus::encode;
use bitcoin::p2p::ServiceFlags;
use bitcoin::p2p::address::Address;
use bitcoin::p2p::message::{NetworkMessage, RawNetworkMessage};
use bitcoin::p2p::message_network::VersionMessage;
use bitcoin::secp256k1::{PublicKey, Secp256k1, SecretKey};
use friglet_ui::descriptor::WalletKeys;
use friglet_ui::network::KeyFamily;
use serde_json::Value;

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_friglet-ui"))
}

fn fake_friglet() -> PathBuf {
    let p = bin().parent().unwrap().join("examples/fake_friglet");
    assert!(
        p.is_file(),
        "{} missing: run `cargo test` (builds examples) or `cargo build --examples`",
        p.display()
    );
    p
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// A regtest peer answering the version handshake.
fn fake_peer(height: i32, services: u64) -> SocketAddr {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = l.local_addr().unwrap();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            std::thread::spawn(move || {
                let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
                let mut header = [0u8; 24];
                if s.read_exact(&mut header).is_err() {
                    return;
                }
                let len =
                    u32::from_le_bytes([header[16], header[17], header[18], header[19]]) as usize;
                let mut payload = vec![0u8; len];
                if s.read_exact(&mut payload).is_err() {
                    return;
                }
                let magic = bitcoin::Network::Regtest.magic();
                let mut v = VersionMessage::new(
                    ServiceFlags::from(services),
                    0,
                    Address::new(&addr, ServiceFlags::NONE),
                    Address::new(&addr, ServiceFlags::from(services)),
                    1,
                    "/Satoshi:31.1.0/".into(),
                    height,
                );
                v.version = 70016;
                for m in [NetworkMessage::Version(v), NetworkMessage::Verack] {
                    let _ = s.write_all(&encode::serialize(&RawNetworkMessage::new(magic, m)));
                }
                let mut sink = [0u8; 256];
                let _ = s.read(&mut sink);
            });
        }
    });
    addr
}

struct Wallet {
    descriptor_file: PathBuf,
    secret_hex: String,
    descriptor: String,
}

fn new_wallet(dir: &Path, seed: u8) -> Wallet {
    let secp = Secp256k1::new();
    let mut s = rand::random::<[u8; 32]>();
    s[0] = seed;
    let scan = SecretKey::from_slice(&s).unwrap();
    let spend = PublicKey::from_secret_key(
        &secp,
        &SecretKey::from_slice(&rand::random::<[u8; 32]>()).unwrap(),
    );
    let keys = WalletKeys::new(scan, spend, None);
    let descriptor = keys.to_descriptor(KeyFamily::Test);
    let descriptor_file = dir.join(format!("desc-{seed}"));
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&descriptor_file)
        .unwrap();
    writeln!(f, "{descriptor}").unwrap();
    Wallet {
        descriptor_file,
        secret_hex: keys.scan_secret_hex(),
        descriptor,
    }
}

struct Daemon {
    _tmp: tempfile::TempDir,
    root: PathBuf,
    data: PathBuf,
    log: PathBuf,
    port: u16,
    peer: SocketAddr,
    child: Option<Child>,
}

impl Daemon {
    fn new(mode: &str, peer_services: u64) -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        let data = root.join("d");
        std::fs::create_dir(&data).unwrap();
        std::fs::write(data.join("fake-mode"), mode).unwrap();
        Daemon {
            log: root.join("friglet-ui.log"),
            root,
            data,
            port: free_port(),
            peer: fake_peer(500, peer_services),
            child: None,
            _tmp: tmp,
        }
    }

    fn command(&self) -> Command {
        let mut c = Command::new(bin());
        c.env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("FRIGLET_UI_DATA_DIR", &self.data)
            .env("FRIGLET_UI_NETWORK", "regtest")
            .env("FRIGLET_UI_NODE", self.peer.to_string())
            .env("FRIGLET_UI_ORACLE_URL", "http://127.0.0.1:9")
            .env("FRIGLET_UI_LISTEN", format!("127.0.0.1:{}", self.port))
            .env("FRIGLET_UI_AUTH", "none")
            .env("FRIGLET_UI_ELECTRUM_TCP", "off")
            .env("FRIGLET_UI_ELECTRUM_TLS", "off")
            .env("FRIGLET_UI_FRIGLET_BIN", fake_friglet())
            .env("FRIGLET_UI_LOG", "info");
        c
    }

    fn start(&mut self) {
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log)
            .unwrap();
        let child = self
            .command()
            .stdin(Stdio::null())
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .spawn()
            .unwrap();
        self.child = Some(child);
        let deadline = Instant::now() + Duration::from_secs(20);
        while TcpStream::connect(("127.0.0.1", self.port)).is_err() {
            assert!(
                Instant::now() < deadline,
                "friglet-ui did not listen:\n{}",
                self.log_text()
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn cli(&self, args: &[&str]) -> Output {
        self.command()
            .args(args)
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    fn stage(&self, w: &Wallet, extra: &[&str]) -> Value {
        let mut args = vec!["setup", "--birthday", "100", "--json", "--descriptor-file"];
        args.push(w.descriptor_file.to_str().unwrap());
        args.extend_from_slice(extra);
        let out = self.cli(&args);
        serde_json::from_slice(&out.stdout).unwrap_or_else(|e| {
            panic!(
                "setup output: {e}: {}",
                String::from_utf8_lossy(&out.stderr)
            )
        })
    }

    fn state(&self) -> Value {
        let (status, body) = self.http("GET", "/api/state", None);
        assert_eq!(status, 200, "{body}");
        serde_json::from_str(&body).unwrap()
    }

    fn wait_for(&self, what: &str, timeout: Duration, f: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + timeout;
        loop {
            let s = self.state();
            if f(&s) {
                return s;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {what}; last state: {s}\nlog:\n{}",
                self.log_text()
            );
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    fn wait_state(&self, id: &str, timeout: Duration) -> Value {
        self.wait_for(id, timeout, |s| s["state"]["id"] == id)
    }

    fn events(&self) -> String {
        std::fs::read_to_string(self.data.join("fake-events.log")).unwrap_or_default()
    }

    fn log_text(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }

    fn fake_pid(&self) -> i32 {
        let v: Value = serde_json::from_str(
            &std::fs::read_to_string(self.data.join("fake-observed.json")).unwrap(),
        )
        .unwrap();
        v["pid"].as_i64().unwrap() as i32
    }

    /// Minimal HTTP/1.1 client: GET, or POST with the CSRF double-submit and Origin.
    fn http(&self, method: &str, path: &str, body: Option<&str>) -> (u16, String) {
        let mut extra = String::new();
        if method != "GET" {
            let (_, session_headers, session_body) =
                raw_http(self.port, "GET", "/api/session", "", None);
            let token: Value = serde_json::from_str(&session_body).unwrap();
            let token = token["csrf_token"].as_str().unwrap().to_string();
            let cookie = session_headers
                .get("set-cookie")
                .and_then(|c| c.split(';').next())
                .map(str::to_string)
                .unwrap_or_default();
            extra = format!(
                "Cookie: {cookie}\r\nX-CSRF-Token: {token}\r\nOrigin: http://127.0.0.1:{}\r\n",
                self.port
            );
        }
        let (status, _, body) = raw_http(self.port, method, path, &extra, body);
        (status, body)
    }

    fn sigterm(&mut self) -> std::process::ExitStatus {
        let mut child = self.child.take().unwrap();
        unsafe {
            libc::kill(child.id() as i32, libc::SIGTERM);
        }
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "friglet-ui did not exit on SIGTERM"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        if let Some(mut c) = self.child.take() {
            unsafe {
                libc::kill(c.id() as i32, libc::SIGTERM);
            }
            let deadline = Instant::now() + Duration::from_secs(10);
            while c.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(50));
            }
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

fn raw_http(
    port: u16,
    method: &str,
    path: &str,
    extra: &str,
    body: Option<&str>,
) -> (u16, HashMap<String, String>, String) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(60))).unwrap();
    let body = body.unwrap_or("");
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{extra}\r\n{body}",
        body.len()
    );
    s.write_all(req.as_bytes()).unwrap();
    let mut resp = String::new();
    s.read_to_string(&mut resp).unwrap();
    let (head, rest) = resp.split_once("\r\n\r\n").unwrap_or((&resp, ""));
    let mut lines = head.lines();
    let status: u16 = lines
        .next()
        .unwrap()
        .split(' ')
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let headers = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string()))
        .collect();
    (status, headers, rest.to_string())
}

const LONG: Duration = Duration::from_secs(40);

#[test]
fn staged_setup_is_verified_and_friglet_runs_isolated() {
    let mut d = Daemon::new("ok", 1 | 8 | 1024);
    let w = new_wallet(&d.root, 1);
    let out = d.stage(&w, &[]);
    assert_eq!(out["code"], 0, "{out}");
    assert_eq!(out["state"], "staged");
    d.start();
    let s = d.wait_state("up-to-date", LONG);
    assert_eq!(s["wallet"]["identity_verified"], true);
    assert_eq!(s["heights"]["scanned"], 120);
    assert_eq!(s["heights"]["node_tip"], 500);
    assert_eq!(s["network"], "regtest");
    assert!(s["transaction"].is_null(), "transaction committed: {s}");

    // friglet's process: cleared environment, umask 077, explicit --config.
    let observed: Value =
        serde_json::from_str(&std::fs::read_to_string(d.data.join("fake-observed.json")).unwrap())
            .unwrap();
    let mut env: Vec<String> = serde_json::from_value(observed["env"].clone()).unwrap();
    env.sort();
    assert_eq!(env, ["FRIGLET_LOG_FILE", "HOME", "PATH", "TZ"]);
    assert_eq!(observed["umask"], "77");
    assert_eq!(observed["args"][0], "scan");
    assert_eq!(observed["args"][1], "--config");
    assert_eq!(
        observed["args"][2],
        d.data.join("config.toml").to_str().unwrap()
    );
    let cfg = std::fs::read_to_string(d.data.join("config.toml")).unwrap();
    assert!(
        cfg.contains(&format!("p2p_node_addr = \"{}\"", d.peer)),
        "{cfg}"
    );
    assert!(cfg.contains("electrum_addr = \"127.0.0.1:50001\""));
    assert!(!cfg.contains("http_addr"), "{cfg}");
    assert!(!cfg.contains(&w.secret_hex));

    assert_eq!(d.cli(&["health"]).status.code(), Some(0));
    let st = d.cli(&["status", "--json"]);
    assert_eq!(st.status.code(), Some(0));
    let widget = raw_http(d.port, "GET", "/api/widget", "", None).2;
    let widget: Value = serde_json::from_str(&widget).unwrap();
    assert_eq!(widget["type"], "text-with-progress");
    assert_eq!(widget["state"], "up-to-date");
    assert!(widget.get("wallet").is_none());

    // SIGTERM to friglet-ui stops friglet through the control socket.
    let status = d.sigterm();
    assert!(status.success(), "{status}");
    assert!(d.events().contains("shutdown-request"), "{}", d.events());
    assert!(d.events().contains("stopped"));
}

#[test]
fn friglet_is_restarted_after_kill_9() {
    let mut d = Daemon::new("ok", 1 | 1024);
    let w = new_wallet(&d.root, 2);
    d.stage(&w, &[]);
    d.start();
    d.wait_state("up-to-date", LONG);
    let pid = d.fake_pid();
    unsafe {
        libc::kill(pid, libc::SIGKILL);
    }
    let s = d.wait_for("a restart", LONG, |s| {
        s["restarts"].as_u64() >= Some(1) && s["state"]["id"] == "up-to-date"
    });
    assert_ne!(d.fake_pid(), pid);
    assert_eq!(s["wallet"]["identity_verified"], true);
    assert!(
        d.log_text().contains("killed by signal 9"),
        "{}",
        d.log_text()
    );
}

#[test]
fn identity_mismatch_stops_friglet_and_reentry_recovers() {
    let mut d = Daemon::new("mismatch", 1);
    let w = new_wallet(&d.root, 3);
    d.stage(&w, &[]);
    d.start();
    let s = d.wait_state("identity-mismatch", LONG);
    assert_ne!(
        s["wallet"]["reported_sp_address"],
        s["wallet"]["sp_address"]
    );
    assert!(
        s["state"]["detail"]
            .as_str()
            .unwrap()
            .contains(s["wallet"]["sp_address"].as_str().unwrap())
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while !d.events().contains("stopped") {
        assert!(
            Instant::now() < deadline,
            "friglet not stopped: {}",
            d.events()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    // No restart: still one start after a while.
    std::thread::sleep(Duration::from_secs(3));
    assert_eq!(
        d.events().matches("start mismatch").count(),
        1,
        "{}",
        d.events()
    );
    assert_eq!(d.cli(&["health"]).status.code(), Some(1));

    // Re-entering the same descriptor (handed over to the daemon) starts it fresh.
    std::fs::write(d.data.join("fake-mode"), "ok").unwrap();
    let out = d.stage(&w, &[]);
    assert_eq!(out["code"], 0, "{out}");
    d.wait_state("up-to-date", LONG);
}

#[test]
fn oracle_down_at_start_is_retried_with_backoff() {
    let mut d = Daemon::new("exit1", 1);
    let w = new_wallet(&d.root, 4);
    d.stage(&w, &[]);
    d.start();
    let s = d.wait_state("oracle-unreachable", LONG);
    assert!(s["state"]["next_retry_at"].is_string(), "{s}");
    std::fs::write(d.data.join("fake-mode"), "ok").unwrap();
    d.wait_state("up-to-date", LONG);
}

#[test]
fn port_in_use_is_permanent_until_retry() {
    let mut d = Daemon::new("port", 1);
    let w = new_wallet(&d.root, 5);
    d.stage(&w, &[]);
    d.start();
    let s = d.wait_state("port-in-use", LONG);
    assert!(s["state"]["next_retry_at"].is_null());
    std::thread::sleep(Duration::from_secs(4));
    assert_eq!(
        d.events().matches("start port").count(),
        1,
        "no automatic retry: {}",
        d.events()
    );
    std::fs::write(d.data.join("fake-mode"), "ok").unwrap();
    let (status, body) = d.http("POST", "/api/scan/resume", None);
    assert_eq!(status, 202, "{body}");
    let full: Value = serde_json::from_str(&body).unwrap();
    assert!(
        full.get("configured").is_some(),
        "202 body is the full state document"
    );
    d.wait_state("up-to-date", LONG);
}

#[test]
fn pruned_node_halts_before_friglet_starts() {
    let mut d = Daemon::new("ok", 1024);
    let w = new_wallet(&d.root, 6);
    d.stage(&w, &[]);
    d.start();
    d.wait_state("node-pruned", LONG);
    assert!(
        d.events().is_empty(),
        "friglet never started: {}",
        d.events()
    );
}

#[test]
fn stopped_scan_task_gets_start() {
    let mut d = Daemon::new("stop-scan", 1);
    let w = new_wallet(&d.root, 7);
    d.stage(&w, &[]);
    d.start();
    d.wait_state("up-to-date", LONG);
    assert!(d.events().contains("scan-stopped"), "{}", d.events());
    assert!(d.events().contains("start-request"), "{}", d.events());
}

#[test]
fn pause_resume_and_csrf() {
    let mut d = Daemon::new("ok", 1);
    let w = new_wallet(&d.root, 8);
    d.stage(&w, &[]);
    d.start();
    d.wait_state("up-to-date", LONG);
    // Without the CSRF token and with a foreign Origin.
    let (status, body, _) = {
        let (s, _, b) = raw_http(d.port, "POST", "/api/scan/pause", "", None);
        (s, b, ())
    };
    assert_eq!(status, 403);
    assert!(body.contains("csrf-invalid"), "{body}");
    let (status, _, body) = raw_http(
        d.port,
        "POST",
        "/api/scan/pause",
        "Origin: http://evil.example\r\nX-CSRF-Token: x\r\nCookie: friglet_csrf=x\r\n",
        None,
    );
    assert_eq!(status, 403, "{body}");

    let (status, _) = d.http("POST", "/api/scan/pause", None);
    assert_eq!(status, 202);
    d.wait_state("paused", LONG);
    assert!(d.events().contains("stop-request"));
    let (status, _) = d.http("POST", "/api/scan/resume", None);
    assert_eq!(status, 202);
    d.wait_state("up-to-date", LONG);
}

#[test]
fn web_setup_replace_restore_and_forget_leave_no_secret() {
    let mut d = Daemon::new("ok", 1);
    d.start();
    d.wait_state("needs-setup", LONG);
    assert_eq!(d.cli(&["status", "--json"]).status.code(), Some(2));
    let a = new_wallet(&d.root, 9);
    let b = new_wallet(&d.root, 10);
    let body = |w: &Wallet, replace: bool| {
        serde_json::json!({"descriptor": w.descriptor, "birthday_height": 100, "replace": replace})
            .to_string()
    };
    let (status, resp) = d.http("POST", "/api/setup/validate", Some(&body(&a, false)));
    assert_eq!(status, 200, "{resp}");
    let (status, resp) = d.http("POST", "/api/setup", Some(&body(&a, false)));
    assert_eq!(status, 202, "{resp}");
    d.wait_for("wallet a verified", LONG, |s| {
        s["state"]["id"] == "up-to-date" && s["transaction"].is_null()
    });
    let (status, resp) = d.http("POST", "/api/setup", Some(&body(&b, false)));
    assert_eq!(status, 409, "{resp}");
    assert!(resp.contains("wallet-exists"));
    let (status, _) = d.http("POST", "/api/setup", Some(&body(&b, true)));
    assert_eq!(status, 202);
    let s = d.wait_for("wallet b verified", LONG, |s| {
        s["state"]["id"] == "up-to-date" && s["transaction"].is_null()
    });
    assert_eq!(s["previous"]["exists"], true);
    let addr_b = s["wallet"]["sp_address"].clone();
    let (status, _) = d.http("POST", "/api/wallet/restore-previous", None);
    assert_eq!(status, 202);
    let s = d.wait_for("wallet a restored", LONG, |s| {
        s["state"]["id"] == "up-to-date"
            && s["transaction"].is_null()
            && s["wallet"]["sp_address"] != addr_b
    });
    assert_eq!(s["wallet"]["identity_verified"], true);

    let (status, _) = d.http("POST", "/api/wallet/forget", None);
    assert_eq!(status, 202);
    let s = d.wait_state("needs-setup", LONG);
    assert!(s["wallet"].is_null());
    let (status, _) = d.http("POST", "/api/wallet/restore-previous", None);
    assert_eq!(status, 404);
    d.sigterm();

    // Neither wallet's scan secret nor descriptor is left in the data dir or the logs.
    for w in [&a, &b] {
        for needle in [w.secret_hex.as_bytes(), w.descriptor.as_bytes()] {
            let hits = friglet_ui::files::dir_contains(&d.data, needle).unwrap();
            assert!(hits.is_empty(), "secret left in {hits:?}");
            let log = std::fs::read(&d.log).unwrap();
            assert!(
                !log.windows(needle.len()).any(|x| x == needle),
                "secret in the logs"
            );
        }
    }
}

#[test]
fn password_mode_login_and_cli_reset() {
    let d = {
        let mut d = Daemon::new("ok", 1);
        // Password mode for this one.
        let pw_file = d.root.join("pw");
        std::fs::write(&pw_file, "first-password\n").unwrap();
        let log = std::fs::File::create(&d.log).unwrap();
        let child = d
            .command()
            .env("FRIGLET_UI_AUTH", "password")
            .env("FRIGLET_UI_PASSWORD_FILE", &pw_file)
            .stdin(Stdio::null())
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .spawn()
            .unwrap();
        d.child = Some(child);
        let deadline = Instant::now() + Duration::from_secs(20);
        while TcpStream::connect(("127.0.0.1", d.port)).is_err() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(50));
        }
        d
    };
    let (status, _) = d.http("GET", "/api/state", None);
    assert_eq!(status, 401);
    let (status, _, _) = raw_http(d.port, "GET", "/api/widget", "", None);
    assert_eq!(status, 200);
    let (status, _, body) = raw_http(d.port, "GET", "/healthz", "", None);
    assert_eq!((status, body.as_str()), (200, "ok"));

    // Log in (CSRF + Origin), then use the session.
    let (_, h, session) = raw_http(d.port, "GET", "/api/session", "", None);
    let token = serde_json::from_str::<Value>(&session).unwrap()["csrf_token"]
        .as_str()
        .unwrap()
        .to_string();
    let csrf_cookie = h["set-cookie"].split(';').next().unwrap().to_string();
    let extra = format!(
        "Cookie: {csrf_cookie}\r\nX-CSRF-Token: {token}\r\nOrigin: http://127.0.0.1:{}\r\n",
        d.port
    );
    let (status, _, _) = raw_http(
        d.port,
        "POST",
        "/api/login",
        &extra,
        Some(r#"{"password":"wrong"}"#),
    );
    assert_eq!(status, 401);
    let (status, h, _) = raw_http(
        d.port,
        "POST",
        "/api/login",
        &extra,
        Some(r#"{"password":"first-password"}"#),
    );
    assert_eq!(status, 204);
    let session_cookie = h["set-cookie"].split(';').next().unwrap().to_string();
    assert!(h["set-cookie"].contains("HttpOnly") && h["set-cookie"].contains("SameSite=Strict"));
    let auth = format!("Cookie: {session_cookie}\r\n");
    let (status, _, _) = raw_http(d.port, "GET", "/api/state", &auth, None);
    assert_eq!(status, 200);

    // `password set` through the daemon ends the session.
    let mut child = d
        .command()
        .args(["password", "set"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"second-password\n")
        .unwrap();
    assert!(child.wait().unwrap().success());
    let (status, _, _) = raw_http(d.port, "GET", "/api/state", &auth, None);
    assert_eq!(status, 401, "sessions end on password change");
}

#[test]
fn stray_friglet_variable_is_refused() {
    let d = Daemon::new("ok", 1);
    let out = d
        .command()
        .env("FRIGLET_ORACLE_URL", "x")
        .args(["run"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("FRIGLET_ORACLE_URL"));
}
