//! Command line: the daemon (`friglet-ui` / `friglet-ui run`) and the subcommands of
//! docs/contract.md (`setup`, `password set|reset`, `status`, `health`, `self-test`).

use std::io::{IsTerminal, Read};
use std::net::SocketAddr;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use clap::{Args, Parser, Subcommand};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::auth::{self, Auth};
use crate::config::{self, AuthMode, RunArgs, UiConfig};
use crate::control::{self, CtlRequest, CtlResponse};
use crate::electrum::ElectrumFront;
use crate::files::{DataDir, DataDirLock};
use crate::network::Network;
use crate::setup::{self, Outcome};
use crate::states::{Catalog, StateId};
use crate::supervisor::{self, Shared, Supervisor};
use crate::wallet::{self, SetupPlan, TxKind};

pub const LONG_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (friglet ",
    env!("FRIGLET_UI_FRIGLET_REV"),
    ")"
);

#[derive(Parser)]
#[command(name = "friglet-ui", version = LONG_VERSION, about = "Setup, supervision, status and Electrum front for friglet")]
pub struct Cli {
    /// Data directory (config, key, state, certificates, UI secrets).
    #[arg(
        long,
        env = "FRIGLET_UI_DATA_DIR",
        default_value = "/data",
        global = true
    )]
    pub data_dir: PathBuf,
    #[command(subcommand)]
    pub command: Option<Cmd>,
}

#[derive(Subcommand)]
#[allow(clippy::large_enum_variant)] // parsed once at startup
pub enum Cmd {
    /// Run the daemon (the default without a subcommand).
    Run(RunArgs),
    /// Configure the wallet from a descriptor on stdin or in a 0600 file.
    Setup(SetupArgs),
    /// Set or reset the web UI password (read from stdin); ends all sessions.
    Password {
        #[command(subcommand)]
        action: PasswordAction,
    },
    /// Print the state; exit 0 healthy, 1 needs attention, 2 not configured.
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Exit 0 only while scanning or up to date (container health checks).
    Health,
    /// Offline check of the embedded assets and crypto.
    SelfTest,
}

#[derive(Subcommand, Clone, Copy)]
pub enum PasswordAction {
    /// Set the password read from stdin.
    Set,
    /// Like set; with empty stdin a random password is generated and printed once.
    Reset,
}

#[derive(Args, Debug, Clone)]
pub struct SetupArgs {
    /// bitcoin, signet, testnet4 or regtest (default: FRIGLET_UI_NETWORK).
    #[arg(long, env = "FRIGLET_UI_NETWORK")]
    pub network: Option<String>,
    /// Check this Bitcoin P2P peer (host:port): pruned, reachable, tip for the birthday bound.
    #[arg(long)]
    pub node: Option<String>,
    /// Wallet birthday: a block height, `taproot` (activation height) or `tip` (needs --node).
    #[arg(long)]
    pub birthday: String,
    /// File holding the descriptor (must be mode 0600); default: stdin.
    #[arg(long)]
    pub descriptor_file: Option<PathBuf>,
    /// Replace a different wallet that is already configured.
    #[arg(long)]
    pub replace: bool,
    /// Validate only; change nothing.
    #[arg(long)]
    pub dry_run: bool,
    /// Print the result as JSON.
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser)]
struct RunOnly {
    #[command(flatten)]
    run: RunArgs,
}

/// Entry point; returns the process exit code.
pub fn main() -> i32 {
    // Owner-only files for everything friglet-ui (and friglet, which inherits it) creates.
    // SAFETY: umask has no preconditions.
    unsafe {
        libc::umask(0o077);
    }
    let cli = Cli::parse();
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(r) => r,
        Err(e) => {
            eprintln!("friglet-ui: cannot start the runtime: {e}");
            return 1;
        }
    };
    match cli.command {
        None => {
            let run = match RunOnly::try_parse_from(["friglet-ui"]) {
                Ok(r) => r.run,
                Err(e) => {
                    eprintln!("friglet-ui: {e}");
                    return 2;
                }
            };
            runtime.block_on(run_daemon(cli.data_dir, run))
        }
        Some(Cmd::Run(run)) => runtime.block_on(run_daemon(cli.data_dir, run)),
        Some(Cmd::Setup(args)) => runtime.block_on(cmd_setup(&cli.data_dir, args)),
        Some(Cmd::Password { action }) => runtime.block_on(cmd_password(&cli.data_dir, action)),
        Some(Cmd::Status { json }) => runtime.block_on(cmd_status(&cli.data_dir, json)),
        Some(Cmd::Health) => runtime.block_on(cmd_health(&cli.data_dir)),
        Some(Cmd::SelfTest) => cmd_self_test(),
    }
}

fn init_logging() {
    let level = std::env::var("FRIGLET_UI_LOG").unwrap_or_else(|_| "info".into());
    let filter = tracing_subscriber::EnvFilter::try_new(&level)
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .with_ansi(std::io::stderr().is_terminal())
        .try_init();
}

fn check_env_or_exit() -> Result<(), i32> {
    config::check_environment(std::env::vars_os().map(|(k, _)| k)).map_err(|m| {
        eprintln!("friglet-ui: {m}");
        2
    })
}

// ---------------------------------------------------------------------------------------------
// Daemon
// ---------------------------------------------------------------------------------------------

async fn run_daemon(data_dir: PathBuf, run: RunArgs) -> i32 {
    if let Err(code) = check_env_or_exit() {
        return code;
    }
    init_logging();
    let password_env = std::env::var(config::PASSWORD_ENV).ok();
    let cfg = match UiConfig::resolve(&run, data_dir, password_env) {
        Ok(c) => c,
        Err(m) => {
            tracing::error!("{m}");
            eprintln!("friglet-ui: {m}");
            return 2;
        }
    };
    let dd = match DataDir::create(&cfg.data_dir) {
        Ok(d) => d,
        Err(e) => {
            eprintln!(
                "friglet-ui: cannot use data directory {}: {e}",
                cfg.data_dir.display()
            );
            return 1;
        }
    };
    for sock in [dd.ui_socket(), dd.friglet_socket()] {
        if sock.as_os_str().len() > 100 {
            eprintln!(
                "friglet-ui: data directory path is too long for a Unix socket ({}); use a shorter FRIGLET_UI_DATA_DIR",
                sock.display()
            );
            return 1;
        }
    }
    let Ok(Some(lock)) = DataDirLock::try_acquire(&dd) else {
        eprintln!(
            "friglet-ui: another friglet-ui process uses {} (lock held)",
            dd.root().display()
        );
        return 1;
    };
    let auth = match Auth::init(
        cfg.auth,
        &dd,
        cfg.password.clone(),
        cfg.password_file.as_deref(),
    ) {
        Ok(a) => a,
        Err(m) => {
            tracing::error!("{m}");
            eprintln!("friglet-ui: {m}");
            return 2;
        }
    };
    if cfg.auth == AuthMode::None {
        tracing::warn!(
            "FRIGLET_UI_AUTH=none: the web UI has no login; anyone who reaches it can change the wallet"
        );
    }
    if let Some(m) = &cfg.unsupported_network {
        tracing::error!("{m}; friglet will not be started");
    }

    let (tls_cfg, tls_fingerprint) = match cfg.electrum_tls {
        Some(_) => {
            let hosts: Vec<String> = cfg.public_host.iter().cloned().collect();
            match crate::tls::ensure(&dd, &hosts) {
                Ok(m) => (Some(m.config), Some(m.fingerprint_sha256)),
                Err(e) => {
                    eprintln!("friglet-ui: cannot prepare the Electrum TLS certificate: {e}");
                    return 1;
                }
            }
        }
        None => (None, None),
    };
    let upstream: SocketAddr = crate::FRIGLET_ELECTRUM_ADDR
        .parse()
        .expect("static address");
    let electrum = ElectrumFront::start(cfg.electrum_tcp, cfg.electrum_tls.zip(tls_cfg), upstream);

    let catalog = Catalog::load();
    let (cmd_tx, cmd_rx) = mpsc::channel(16);
    let listen = cfg.listen;
    let shared = Arc::new(Shared {
        view: RwLock::new(supervisor::initial_view(&cfg, &catalog)),
        cfg,
        dd: dd.clone(),
        catalog,
        auth,
        electrum,
        tls_fingerprint,
        cmd: cmd_tx,
        last_peer: RwLock::new(None),
    });

    let listener = match tokio::net::TcpListener::bind(listen).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("friglet-ui: cannot listen on {listen}: {e}");
            return 1;
        }
    };
    let ctl = match control::bind(&dd.ui_socket()) {
        Ok(l) => l,
        Err(e) => {
            eprintln!(
                "friglet-ui: cannot create {}: {e}",
                dd.ui_socket().display()
            );
            return 1;
        }
    };
    tokio::spawn(control::serve(ctl, shared.clone()));

    let shutdown = CancellationToken::new();
    let sup = Supervisor::new(shared.clone(), cmd_rx);
    let sup_task = tokio::spawn(sup.run(shutdown.clone()));

    {
        let shutdown = shutdown.clone();
        tokio::spawn(async move {
            use tokio::signal::unix::{SignalKind, signal};
            let (Ok(mut term), Ok(mut int)) = (
                signal(SignalKind::terminate()),
                signal(SignalKind::interrupt()),
            ) else {
                return;
            };
            tokio::select! {
                _ = term.recv() => tracing::info!("SIGTERM received; shutting down"),
                _ = int.recv() => tracing::info!("SIGINT received; shutting down"),
            }
            shutdown.cancel();
        });
    }

    tracing::info!(
        version = crate::VERSION,
        friglet = crate::friglet_short_rev(),
        %listen,
        platform = shared.cfg.platform.as_str(),
        network = shared.cfg.network.as_str(),
        auth = shared.cfg.auth.as_str(),
        assets = crate::assets::ASSETS_SOURCE,
        "friglet-ui listening"
    );
    let app = crate::web::router(shared.clone());
    let server = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown({
        let s = shutdown.clone();
        async move { s.cancelled().await }
    });
    let server_task = tokio::spawn(async move { server.await });

    shutdown.cancelled().await;
    let _ = sup_task.await;
    let _ = tokio::time::timeout(Duration::from_secs(5), server_task).await;
    let _ = std::fs::remove_file(dd.ui_socket());
    drop(lock);
    tracing::info!("friglet-ui stopped");
    0
}

// ---------------------------------------------------------------------------------------------
// setup
// ---------------------------------------------------------------------------------------------

fn print_outcome(o: &Outcome, json: bool) -> i32 {
    if json {
        println!("{}", serde_json::to_string(o).unwrap_or_default());
    } else if o.ok {
        match &o.sp_address {
            Some(a) => println!("{}\nSilent Payments address: {a}", o.message),
            None => println!("{}", o.message),
        }
    } else {
        eprintln!("friglet-ui setup: {}", o.message);
    }
    o.code
}

fn read_descriptor(args: &SetupArgs) -> Result<String, Box<Outcome>> {
    let text = match &args.descriptor_file {
        Some(path) => {
            let meta = std::fs::metadata(path).map_err(|e| {
                Box::new(Outcome::err(
                    setup::EXIT_USAGE,
                    "usage",
                    Some("descriptor"),
                    format!("cannot read {}: {e}", path.display()),
                ))
            })?;
            if meta.mode() & 0o077 != 0 {
                return Err(Box::new(Outcome::err(
                    setup::EXIT_USAGE,
                    "usage",
                    Some("descriptor"),
                    format!(
                        "{} is readable by others (mode {:o}); chmod 600 it first",
                        path.display(),
                        meta.mode() & 0o777
                    ),
                )));
            }
            std::fs::read_to_string(path).map_err(|e| {
                Box::new(Outcome::err(
                    setup::EXIT_USAGE,
                    "usage",
                    Some("descriptor"),
                    format!("cannot read {}: {e}", path.display()),
                ))
            })?
        }
        None => {
            if std::io::stdin().is_terminal() {
                eprintln!("Paste the descriptor, then press Ctrl-D:");
            }
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s).map_err(|e| {
                Box::new(Outcome::err(
                    setup::EXIT_USAGE,
                    "usage",
                    Some("descriptor"),
                    format!("cannot read stdin: {e}"),
                ))
            })?;
            s
        }
    };
    if text.trim().is_empty() {
        return Err(Box::new(Outcome::err(
            setup::EXIT_DESCRIPTOR,
            "descriptor-invalid",
            Some("descriptor"),
            "no descriptor given: pass it on stdin or with --descriptor-file (never as an argument)",
        )));
    }
    Ok(text)
}

async fn cmd_setup(data_dir: &Path, args: SetupArgs) -> i32 {
    if let Err(code) = check_env_or_exit() {
        return code;
    }
    let json = args.json;
    match setup_inner(data_dir, &args).await {
        Ok(o) | Err(o) => print_outcome(&o, json),
    }
}

#[allow(clippy::result_large_err)] // one CLI outcome per process
async fn setup_inner(data_dir: &Path, args: &SetupArgs) -> Result<Outcome, Outcome> {
    let network: Network = match args.network.as_deref() {
        None => {
            return Err(Outcome::err(
                setup::EXIT_USAGE,
                "usage",
                None,
                "--network (or FRIGLET_UI_NETWORK) is required",
            ));
        }
        Some(n) => n.parse().map_err(|m: String| {
            Outcome::err(setup::EXIT_NETWORK, "unsupported-network", None, m)
        })?,
    };
    let descriptor = read_descriptor(args).map_err(|o| *o)?;

    // Optional node check.
    let mut node_tip = None;
    if let Some(node) = &args.node {
        config::validate_node(node)
            .map_err(|m| Outcome::err(setup::EXIT_USAGE, "usage", None, m))?;
        match crate::p2p::check_node(node, network).await {
            Ok(peer) if peer.is_pruned() => {
                return Err(Outcome::err(
                    setup::EXIT_NODE,
                    "node-pruned",
                    None,
                    format!(
                        "the node at {node} is pruned; friglet needs an unpruned node. See docs/troubleshooting.md#node-pruned"
                    ),
                ));
            }
            Ok(peer) => node_tip = peer.tip(),
            Err(e) => {
                return Err(Outcome::err(
                    setup::EXIT_NODE,
                    "node-check",
                    None,
                    format!("node check failed: {e}"),
                ));
            }
        }
    }
    let birthday = match args.birthday.as_str() {
        "taproot" => network.min_birthday(),
        "tip" => node_tip.ok_or_else(|| {
            Outcome::err(
                setup::EXIT_USAGE,
                "usage",
                None,
                "--birthday tip needs --node",
            )
        })?,
        h => h.parse::<u64>().map_err(|_| {
            Outcome::err(
                setup::EXIT_BIRTHDAY,
                "birthday-invalid",
                Some("birthday_height"),
                format!("invalid birthday `{h}`: use a block height"),
            )
        })?,
    };
    let (keys, identity) =
        setup::validate(&descriptor, birthday, network, node_tip).map_err(|o| *o)?;
    if args.dry_run {
        return Ok(Outcome::ok(
            None,
            Some(&identity),
            "valid; nothing changed (--dry-run)",
        ));
    }

    let dd = DataDir::create(data_dir).map_err(|e| {
        Outcome::err(
            setup::EXIT_IO,
            "io",
            None,
            format!("cannot use data directory {}: {e}", data_dir.display()),
        )
    })?;
    match DataDirLock::try_acquire(&dd) {
        Ok(Some(_lock)) => {
            // No daemon: stage under the lock; the daemon verifies at its next start.
            let _ = wallet::recover(&dd);
            let plan = wallet::plan_setup(&dd, &identity, birthday).map_err(|e| {
                Outcome::err(
                    setup::EXIT_IO,
                    "io",
                    None,
                    format!("cannot read the wallet files: {e}"),
                )
            })?;
            let kind = match plan {
                SetupPlan::NoOp => {
                    return Ok(Outcome::ok(
                        Some("unchanged"),
                        Some(&identity),
                        "this wallet is already set up; nothing changed",
                    ));
                }
                SetupPlan::Replace { current } if !args.replace => {
                    return Err(Outcome::err(
                        setup::EXIT_EXISTS,
                        "wallet-exists",
                        None,
                        setup::exists_message(&current),
                    ));
                }
                SetupPlan::Replace { .. } => TxKind::Replace,
                SetupPlan::Fresh => TxKind::Setup,
            };
            wallet::stage(&dd, &keys, network, birthday, kind, true).map_err(|e| {
                Outcome::err(
                    setup::EXIT_IO,
                    "io",
                    None,
                    format!("cannot write the wallet files: {e}"),
                )
            })?;
            Ok(Outcome::ok(
                Some("staged"),
                Some(&identity),
                "staged, verification pending: friglet-ui checks the wallet identity when it starts",
            ))
        }
        Ok(None) => {
            let req = CtlRequest::Setup {
                descriptor,
                birthday,
                replace: args.replace,
                network: network.as_str().to_string(),
            };
            match control::call(&dd.ui_socket(), &req, Duration::from_secs(200)).await {
                Ok(CtlResponse::Outcome(o)) if o.ok => Ok(o),
                Ok(CtlResponse::Outcome(o)) => Err(o),
                Ok(_) => Err(Outcome::err(
                    setup::EXIT_IO,
                    "io",
                    None,
                    "unexpected answer from friglet-ui",
                )),
                Err(e) => Err(Outcome::err(
                    setup::EXIT_IO,
                    "io",
                    None,
                    format!(
                        "the data directory is locked by a running friglet-ui, but its control socket does not answer: {e}"
                    ),
                )),
            }
        }
        Err(e) => Err(Outcome::err(
            setup::EXIT_IO,
            "io",
            None,
            format!("cannot lock the data directory: {e}"),
        )),
    }
}

// ---------------------------------------------------------------------------------------------
// password, status, health
// ---------------------------------------------------------------------------------------------

fn generate_password() -> String {
    use rand::Rng;
    const CHARS: &[u8] = b"abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut rng = rand::thread_rng();
    (0..20)
        .map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char)
        .collect()
}

async fn cmd_password(data_dir: &Path, action: PasswordAction) -> i32 {
    if let Err(code) = check_env_or_exit() {
        return code;
    }
    let mut input = String::new();
    if !std::io::stdin().is_terminal() || matches!(action, PasswordAction::Set) {
        if std::io::stdin().is_terminal() {
            eprintln!("New password, then Enter and Ctrl-D:");
        }
        if let Err(e) = std::io::stdin().read_to_string(&mut input) {
            eprintln!("friglet-ui password: cannot read stdin: {e}");
            return 2;
        }
    }
    let mut password = auth::trim_one_newline(&input).to_string();
    let mut generated = false;
    if password.is_empty() && matches!(action, PasswordAction::Reset) {
        password = generate_password();
        generated = true;
    }
    if password.chars().count() < auth::MIN_PASSWORD_LEN {
        eprintln!(
            "friglet-ui password: the password needs at least {} characters",
            auth::MIN_PASSWORD_LEN
        );
        return 2;
    }
    let dd = match DataDir::create(data_dir) {
        Ok(d) => d,
        Err(e) => {
            eprintln!(
                "friglet-ui password: cannot use {}: {e}",
                data_dir.display()
            );
            return 8;
        }
    };
    let result = match DataDirLock::try_acquire(&dd) {
        Ok(Some(_lock)) => auth::store_password(&dd, &password)
            .map(|_| "password stored".to_string())
            .map_err(|e| e.to_string()),
        Ok(None) => match control::call(
            &dd.ui_socket(),
            &CtlRequest::PasswordSet {
                password: password.clone(),
            },
            Duration::from_secs(30),
        )
        .await
        {
            Ok(CtlResponse::Done { ok: true, message }) => Ok(message),
            Ok(CtlResponse::Done { ok: false, message }) => Err(message),
            Ok(_) => Err("unexpected answer from friglet-ui".into()),
            Err(e) => Err(format!("friglet-ui does not answer: {e}")),
        },
        Err(e) => Err(format!("cannot lock the data directory: {e}")),
    };
    match result {
        Ok(msg) => {
            if generated {
                println!("{password}");
                eprintln!("friglet-ui password: new password generated (printed above); {msg}");
            } else {
                eprintln!("friglet-ui password: {msg}");
            }
            0
        }
        Err(m) => {
            eprintln!("friglet-ui password: {m}");
            8
        }
    }
}

async fn daemon_state(dd: &DataDir) -> Option<crate::view::StateResponse> {
    match control::call(
        &dd.ui_socket(),
        &CtlRequest::Status,
        Duration::from_secs(10),
    )
    .await
    {
        Ok(CtlResponse::State(s)) => Some(*s),
        _ => None,
    }
}

async fn cmd_status(data_dir: &Path, json: bool) -> i32 {
    let dd = DataDir::at(data_dir);
    match daemon_state(&dd).await {
        Some(s) => {
            let code = match (s.state.id, s.state.severity.as_str()) {
                (StateId::NeedsSetup, _) => 2,
                (_, "warn" | "error") => 1,
                _ => 0,
            };
            if json {
                println!("{}", serde_json::to_string(&s).unwrap_or_default());
            } else {
                let h = &s.heights;
                println!(
                    "{}: {} (scanned {}, node tip {}, oracle tip {})",
                    s.state.id.as_str(),
                    s.state.headline,
                    h.scanned
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".into()),
                    h.node_tip
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".into()),
                    h.oracle_tip
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".into()),
                );
            }
            code
        }
        None => {
            let configured = dd.has_wallet();
            if json {
                println!(
                    "{}",
                    serde_json::json!({"configured": configured, "running": false, "state": null})
                );
            } else if configured {
                println!("friglet-ui is not running (wallet configured)");
            } else {
                println!("friglet-ui is not running; no wallet configured");
            }
            if configured { 1 } else { 2 }
        }
    }
}

async fn cmd_health(data_dir: &Path) -> i32 {
    match daemon_state(&DataDir::at(data_dir)).await {
        Some(s) if s.state.id.is_healthy() => 0,
        Some(s) => {
            eprintln!("{}", s.state.id.as_str());
            1
        }
        None => {
            eprintln!("friglet-ui does not answer");
            1
        }
    }
}

// ---------------------------------------------------------------------------------------------
// self-test
// ---------------------------------------------------------------------------------------------

/// BIP-352 receiving vector 1 (`bip-0352/send_and_receive_test_vectors.json`).
const ST_SCAN: &str = "0f694e068028a717f8af6b9411f9a133dd3565258714cc226594b34db90c1f2c";
const ST_SPEND: &str = "9d6ad855ce3417ef84e836892e5a56392bfba05fa5d97ccea30e266f540e08b3";
const ST_ADDRESS: &str = "sp1qqgste7k9hx0qftg6qmwlkqtwuy6cycyavzmzj85c6qdfhjdpdjtdgqjuexzk6murw56suy3e0rd2cgqvycxttddwsvgxe2usfpxumr70xc9pkqwv";

pub fn self_test_checks() -> Vec<(&'static str, Result<String, String>)> {
    use bitcoin::secp256k1::{PublicKey, Secp256k1, SecretKey};
    use std::str::FromStr;
    let mut out: Vec<(&'static str, Result<String, String>)> = Vec::new();

    out.push(("embedded web assets", {
        match crate::assets::get("index.html") {
            Some(b) if !b.is_empty() => Ok(format!(
                "{} files from {}",
                crate::assets::ASSETS.len(),
                crate::assets::ASSETS_SOURCE
            )),
            _ => Err("index.html missing".into()),
        }
    }));
    out.push(("state catalog", {
        crate::states::Catalog::parse(crate::states::CATALOG_TOML)
            .map(|c| format!("{} states", c.ids().count()))
    }));
    out.push(("BIP-380 checksum", {
        match crate::descriptor::checksum("raw(deadbeef)").as_deref() {
            Some("89f8spxm") => Ok("raw(deadbeef)#89f8spxm".into()),
            other => Err(format!("got {other:?}")),
        }
    }));
    out.push(("BIP-352/392 address derivation", {
        (|| {
            let secp = Secp256k1::new();
            let scan = SecretKey::from_str(ST_SCAN).map_err(|e| e.to_string())?;
            let spend = PublicKey::from_secret_key(
                &secp,
                &SecretKey::from_str(ST_SPEND).map_err(|e| e.to_string())?,
            );
            let desc = crate::descriptor::WalletKeys::new(scan, spend, None)
                .to_descriptor(crate::network::KeyFamily::Mainnet);
            let keys = crate::descriptor::parse(&desc).map_err(|e| e.to_string())?;
            let addr = keys.identity(Network::Bitcoin).sp_address;
            if addr == ST_ADDRESS {
                Ok("BIP-352 vector 1 matches".to_string())
            } else {
                Err(format!("derived {addr}"))
            }
        })()
    }));
    out.push(("argon2 password hash", {
        auth::hash_password("self-test-password")
            .map_err(|e| e.to_string())
            .and_then(|h| {
                if auth::verify_password(&h, "self-test-password")
                    && !auth::verify_password(&h, "other")
                {
                    Ok("hash and verify".into())
                } else {
                    Err("verify failed".into())
                }
            })
    }));
    out.push(("TLS certificate (rcgen + rustls/ring)", {
        crate::tls::generate(&["self-test".into()])
            .and_then(|(c, k)| crate::tls::load(c.as_bytes(), k.as_bytes()))
            .map(|m| format!("SHA-256 {}…", &m.fingerprint_sha256[..11]))
            .map_err(|e| e.to_string())
    }));
    out.push(("P2P message encoding", {
        use bitcoin::consensus::encode;
        use bitcoin::p2p::message::{NetworkMessage, RawNetworkMessage};
        let raw = RawNetworkMessage::new(Network::Signet.magic(), NetworkMessage::Verack);
        let bytes = encode::serialize(&raw);
        match encode::deserialize::<RawNetworkMessage>(&bytes) {
            Ok(back) if back == raw => Ok(format!("{} byte verack roundtrip", bytes.len())),
            Ok(_) => Err("roundtrip differs".into()),
            Err(e) => Err(e.to_string()),
        }
    }));
    out
}

fn cmd_self_test() -> i32 {
    let mut failed = false;
    println!(
        "friglet-ui {LONG_VERSION} self-test ({})",
        std::env::consts::ARCH
    );
    for (name, result) in self_test_checks() {
        match result {
            Ok(detail) => println!("ok      {name}: {detail}"),
            Err(e) => {
                failed = true;
                println!("FAILED  {name}: {e}");
            }
        }
    }
    if failed {
        println!("self-test FAILED");
        1
    } else {
        println!("self-test ok");
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn self_test_passes() {
        for (name, r) in self_test_checks() {
            assert!(r.is_ok(), "{name}: {r:?}");
        }
    }

    #[test]
    fn cli_parses_contract_forms() {
        let c = Cli::try_parse_from([
            "friglet-ui",
            "setup",
            "--network",
            "signet",
            "--node",
            "h:1",
            "--birthday",
            "100",
            "--descriptor-file",
            "/x",
            "--replace",
            "--dry-run",
            "--json",
        ])
        .unwrap();
        assert!(matches!(c.command, Some(Cmd::Setup(_))));
        assert!(Cli::try_parse_from(["friglet-ui", "password", "set"]).is_ok());
        assert!(Cli::try_parse_from(["friglet-ui", "password", "reset"]).is_ok());
        assert!(Cli::try_parse_from(["friglet-ui", "status", "--json"]).is_ok());
        assert!(Cli::try_parse_from(["friglet-ui", "health"]).is_ok());
        assert!(Cli::try_parse_from(["friglet-ui", "self-test"]).is_ok());
        assert!(
            Cli::try_parse_from(["friglet-ui"])
                .unwrap()
                .command
                .is_none()
        );
        // The descriptor is never an argument.
        assert!(
            Cli::try_parse_from([
                "friglet-ui",
                "setup",
                "--birthday",
                "1",
                "--descriptor",
                "sp(x)"
            ])
            .is_err()
        );
        let e = Cli::try_parse_from(["friglet-ui", "setup"]).err().unwrap();
        assert_eq!(e.exit_code(), 2);
    }

    #[test]
    fn generated_passwords_are_long_enough() {
        let p = generate_password();
        assert!(p.len() >= auth::MIN_PASSWORD_LEN);
    }
}
