//! A stand-in for friglet used by friglet-ui's integration tests (`tests/supervisor.rs`).
//!
//! It is started exactly like friglet (`fake_friglet scan --config <file>`), reads the same
//! config and key file, serves the friglet-ipc control socket and writes a state file. Its
//! behaviour comes from `<data>/fake-mode` (the environment is cleared by friglet-ui):
//!
//! - `ok`: scan from the birthday to birthday+20, then stay up to date;
//! - `mismatch`: report another wallet's `sp_address`;
//! - `exit1`: print friglet's oracle-down error and exit 1;
//! - `port`: print friglet's HTTP bind panic and abort;
//! - `stop-scan`: the scan task stops after one second until `Start` arrives.
//!
//! It records what it saw in `<data>/fake-observed.json` and appends events to
//! `<data>/fake-events.log`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bitcoin::secp256k1::{PublicKey, Secp256k1, SecretKey};
use friglet_ipc::{Request, Response, StatusInfo};
use friglet_ui::network::Network;

struct State {
    scanning: bool,
    scanned: u64,
    tip: u64,
    stopped_at: Option<Instant>,
}

fn event(dir: &Path, what: &str) {
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("fake-events.log"))
    {
        let _ = writeln!(f, "{what}");
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let config_path = PathBuf::from(
        args.iter()
            .position(|a| a == "--config")
            .and_then(|i| args.get(i + 1))
            .expect("--config"),
    );
    let cfg = friglet_ipc::read_config_toml(&config_path).expect("config");
    let dir = config_path.parent().unwrap().to_path_buf();
    let mode = std::fs::read_to_string(dir.join("fake-mode"))
        .unwrap_or_else(|_| "ok".into())
        .trim()
        .to_string();
    // SAFETY: umask has no preconditions; restored right away.
    let umask = unsafe {
        let m = libc::umask(0o077);
        libc::umask(m);
        m
    };
    let observed = serde_json::json!({
        "args": args[1..],
        "env": std::env::vars().map(|(k, _)| k).collect::<Vec<_>>(),
        "umask": format!("{umask:o}"),
        "pid": std::process::id(),
    });
    std::fs::write(dir.join("fake-observed.json"), observed.to_string()).unwrap();
    event(&dir, &format!("start {mode}"));
    println!(
        "2026-10-01T12:00:00.000000Z  INFO starting friglet oracle_url={} p2p_peer={} network={} start_height={}",
        cfg.oracle_url,
        cfg.p2p_node_addr.clone().unwrap_or_default(),
        cfg.network,
        cfg.start_height.unwrap_or_default()
    );

    match mode.as_str() {
        "exit1" => {
            eprintln!("Error: transport error");
            std::process::exit(1);
        }
        "port" => {
            eprintln!("thread 'main' (1) panicked at friglet/src/main.rs:218:14:");
            eprintln!(
                "Failed to bind HTTP server: Os {{ code: 98, kind: AddrInUse, message: \"Address already in use\" }}"
            );
            std::process::abort();
        }
        _ => {}
    }

    let key_hex = std::fs::read_to_string(cfg.key_file.clone().unwrap()).expect("key file");
    let scan = SecretKey::from_str(key_hex.trim()).expect("scan key");
    let spend = PublicKey::from_str(cfg.spend_pubkey.as_deref().unwrap()).expect("spend key");
    let network: Network = cfg.network.parse().expect("network");
    let scan_pub = PublicKey::from_secret_key(&Secp256k1::new(), &scan);
    let address = if mode == "mismatch" {
        let other = SecretKey::from_slice(&[7u8; 32]).unwrap();
        friglet_ui::descriptor::sp_address(
            &PublicKey::from_secret_key(&Secp256k1::new(), &other),
            &spend,
            network,
        )
    } else {
        friglet_ui::descriptor::sp_address(&scan_pub, &spend, network)
    };
    let birthday = cfg.start_height.unwrap_or(1);
    let state = Arc::new(Mutex::new(State {
        scanning: true,
        scanned: birthday.saturating_sub(1),
        tip: birthday + 20,
        stopped_at: None,
    }));

    // Scan loop: one block per 50 ms, a checkpoint per block.
    {
        let state = state.clone();
        let dir = dir.clone();
        let state_file = cfg.state_file.clone();
        let scan_hex = key_hex.trim().to_string();
        let spend_hex = cfg.spend_pubkey.clone().unwrap();
        let mode = mode.clone();
        let started = Instant::now();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(50)).await;
                let mut s = state.lock().unwrap();
                if mode == "stop-scan"
                    && s.scanning
                    && s.stopped_at.is_none()
                    && started.elapsed() > Duration::from_secs(1)
                {
                    s.scanning = false;
                    s.stopped_at = Some(Instant::now());
                    event(&dir, "scan-stopped");
                    continue;
                }
                if s.scanning && s.scanned < s.tip {
                    s.scanned += 1;
                    let json = serde_json::json!({
                        "secret_scan_hex": scan_hex,
                        "public_spend_hex": spend_hex,
                        "last_scanned_block_height": s.scanned,
                    });
                    let _ = std::fs::write(&state_file, json.to_string());
                    if s.scanned.is_multiple_of(10) {
                        println!(
                            "2026-10-01T12:00:00.000000Z  INFO scan progress height={} scanned=1 total=2 pct=50",
                            s.scanned
                        );
                    }
                }
            }
        });
    }

    let socket = cfg.control_socket.clone().expect("control_socket");
    let _ = std::fs::remove_file(&socket);
    let listener = friglet_ipc::listen(&socket).expect("listen");
    println!("2026-10-01T12:00:00.000000Z  INFO control socket listening path={socket}");
    let shutdown = Arc::new(tokio::sync::Notify::new());
    let mut term =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
    loop {
        tokio::select! {
            conn = friglet_ipc::accept(&listener) => {
                let Ok(mut conn) = conn else { continue };
                let state = state.clone();
                let address = address.clone();
                let network = cfg.network.clone();
                let dir = dir.clone();
                let shutdown = shutdown.clone();
                tokio::spawn(async move {
                    while let Ok(Some(req)) = conn.next_request().await {
                        let resp = match req {
                            Request::GetStatus => {
                                let s = state.lock().unwrap();
                                Response::Status(StatusInfo {
                                    scanning: s.scanning,
                                    scanned_height: s.scanned,
                                    tip_height: Some(s.tip),
                                    scan_progress: 0.0,
                                    network: network.clone(),
                                    electrum_clients: 0,
                                    oracle_connected: true,
                                    last_error: None,
                                    sp_address: Some(address.clone()),
                                    tx_count: 0,
                                    outputs_found: 0,
                                    label_addresses: vec![],
                                    version: "fake".into(),
                                    spawned_by_tray: false,
                                })
                            }
                            Request::Start => {
                                event(&dir, "start-request");
                                state.lock().unwrap().scanning = true;
                                Response::Ok
                            }
                            Request::Stop => {
                                event(&dir, "stop-request");
                                state.lock().unwrap().scanning = false;
                                Response::Ok
                            }
                            Request::Shutdown => {
                                event(&dir, "shutdown-request");
                                shutdown.notify_one();
                                Response::Ok
                            }
                            _ => Response::Error("not supported by the fake".into()),
                        };
                        if conn.respond(&resp).await.is_err() {
                            break;
                        }
                    }
                });
            }
            _ = shutdown.notified() => break,
            _ = term.recv() => {
                event(&dir, "sigterm");
                break;
            }
        }
    }
    // Let the Shutdown answer reach the client, as friglet does.
    tokio::time::sleep(Duration::from_millis(100)).await;
    let _ = std::fs::remove_file(&socket);
    println!("2026-10-01T12:00:00.000000Z  INFO friglet stopped");
    event(&dir, "stopped");
}
