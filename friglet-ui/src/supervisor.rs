//! The supervisor: owns the friglet process and the wallet transactions.
//!
//! One task runs [`Supervisor::run`]. It starts friglet once a wallet is configured (after a
//! P2P handshake that re-resolves the node to a literal IP), restarts it on any exit with
//! capped jittered backoff (permanent failures wait for the user), polls `GetStatus`, checks
//! the wallet identity, opens the Electrum front only after it matches, sends `Start` when
//! the scan task stopped, handles pause/resume, setup, replace, forget and restore, and
//! publishes the state the web API serves.

use std::collections::VecDeque;
use std::io;
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::process::{ExitStatus, Stdio};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime};

use friglet_ipc::{Request, Response, StatusInfo};
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command as ProcessCommand};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::auth::Auth;
use crate::classify::{self, ExitKind, LogCondition};
use crate::config::UiConfig;
use crate::descriptor::WalletKeys;
use crate::electrum::ElectrumFront;
use crate::files::{self, DataDir};
use crate::p2p::{self, PeerInfo};
use crate::redact;
use crate::setup::{self, Outcome};
use crate::states::{
    self, Catalog, Desired, Failure, Identity, Inputs, ModelHeights, Process, Scan, StateId,
};
use crate::util::{jitter, now_rfc3339, parse_rfc3339, rfc3339};
use crate::view::*;
use crate::wallet::{self, Intent, Recovery, SetupPlan, TxKind, TxStep, WalletMeta};

/// SIGTERM/Shutdown grace before SIGKILL.
pub const STOP_GRACE: Duration = Duration::from_secs(120);
/// friglet has no oracle connect timeout; give up on a start after this long without a
/// control socket.
pub const STARTUP_DEADLINE: Duration = Duration::from_secs(180);
/// Report "no address" after this long in `verifying-identity`.
pub const IDENTITY_NOTE_AFTER: Duration = Duration::from_secs(300);
pub const POLL_EVERY: Duration = Duration::from_secs(2);
pub const NODE_REFRESH: Duration = Duration::from_secs(600);
pub const STATE_FILE_CHECK: Duration = Duration::from_secs(30);
pub const BACKOFF_CAP: Duration = Duration::from_secs(300);
pub const HEALTHY_RESET: Duration = Duration::from_secs(600);
pub const CRASH_WINDOW: Duration = Duration::from_secs(600);
pub const CRASH_LOOP_EXITS: usize = 5;
pub const PREV_KEEP: Duration = Duration::from_secs(24 * 3600);
/// A log-reported scan condition expires after this long without repeating.
pub const CONDITION_TTL: Duration = Duration::from_secs(900);
const RING_LINES: usize = 400;
const RESCAN_SLACK: u64 = 10;

pub enum Command {
    Setup {
        keys: Box<WalletKeys>,
        birthday: u64,
        replace: bool,
        reply: oneshot::Sender<Outcome>,
    },
    Forget {
        reply: oneshot::Sender<Result<(), String>>,
    },
    RestorePrevious {
        reply: oneshot::Sender<Result<bool, String>>,
    },
    DiscardPrevious {
        reply: oneshot::Sender<Result<bool, String>>,
    },
    Pause {
        reply: oneshot::Sender<()>,
    },
    Resume {
        reply: oneshot::Sender<()>,
    },
}

/// State shared with the web server and the control socket.
pub struct Shared {
    pub cfg: UiConfig,
    pub dd: DataDir,
    pub catalog: Catalog,
    pub auth: Auth,
    pub electrum: ElectrumFront,
    pub tls_fingerprint: Option<String>,
    pub view: RwLock<StateResponse>,
    pub cmd: mpsc::Sender<Command>,
    /// Last successful handshake with the node (tip for birthday bounds).
    pub last_peer: RwLock<Option<PeerInfo>>,
}

impl Shared {
    pub fn state(&self) -> StateResponse {
        self.view.read().unwrap().clone()
    }

    pub fn widget(&self) -> Widget {
        let s = self.state();
        let text = self.catalog.get(s.state.id).widget.clone();
        Widget::from_state(&s, &text)
    }

    pub fn node_tip(&self) -> Option<u64> {
        self.last_peer
            .read()
            .unwrap()
            .as_ref()
            .and_then(PeerInfo::tip)
    }
}

/// Persisted, non-secret supervisor memory (`ui-state.json`).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct UiState {
    #[serde(default)]
    desired: Desired,
    #[serde(default)]
    last_failure: Option<FailureRecord>,
    #[serde(default)]
    prev_expires_at: Option<String>,
    /// Highest scanned height seen, per wallet address.
    #[serde(default)]
    max_scanned: Option<(String, u64)>,
    #[serde(default)]
    restarts: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FailureRecord {
    class: Failure,
    detail: String,
    at: String,
}

struct ChildHandle {
    child: Child,
    started: Instant,
    readers: Vec<JoinHandle<()>>,
    deadline_kill: Option<Instant>,
}

pub struct Supervisor {
    shared: Arc<Shared>,
    cmd_rx: mpsc::Receiver<Command>,
    log_tx: mpsc::Sender<(u64, LogCondition)>,
    log_rx: mpsc::Receiver<(u64, LogCondition)>,
    node_tx: mpsc::Sender<Result<PeerInfo, p2p::P2pError>>,
    node_rx: mpsc::Receiver<Result<PeerInfo, p2p::P2pError>>,
    ring: Arc<Mutex<VecDeque<String>>>,

    wallet: Option<(WalletKeys, WalletMeta)>,
    transaction: Option<Intent>,
    ui: UiState,

    child: Option<ChildHandle>,
    generation: u64,
    process: Process,
    socket_up: bool,
    failure: Option<(Failure, String)>,
    attempt: u32,
    next_start: Option<Instant>,
    next_start_wall: Option<SystemTime>,
    exits: VecDeque<Instant>,
    running_since: Option<Instant>,

    identity: Identity,
    identity_since: Instant,
    reported_address: Option<String>,

    status: Option<StatusInfo>,
    status_at: Option<SystemTime>,
    scan_attempt: u32,
    next_scan_start: Option<Instant>,
    condition: Option<(LogCondition, Instant, u64)>,

    peer: Option<PeerInfo>,
    peer_checked: Option<Instant>,
    node_check_running: bool,

    oracle_tip: Option<u64>,
    persisted: Option<u64>,
    state_checked: Option<Instant>,
    samples: VecDeque<(Instant, u64)>,
    last_state: Option<StateId>,
    ui_saved_at: Option<Instant>,
}

fn exit_kind(status: &ExitStatus) -> ExitKind {
    match (status.code(), status.signal()) {
        (Some(c), _) => ExitKind::Code(c),
        (None, Some(s)) => ExitKind::Signal(s),
        _ => ExitKind::Code(-1),
    }
}

fn describe_exit(kind: ExitKind) -> String {
    match kind {
        ExitKind::Code(c) => format!("friglet exited with code {c}"),
        ExitKind::Signal(6) => "friglet aborted (signal 6, a panic)".to_string(),
        ExitKind::Signal(s) => format!("friglet was killed by signal {s}"),
        ExitKind::StartupDeadline => format!(
            "friglet did not open its control socket within {} s (usually the oracle is not reachable)",
            STARTUP_DEADLINE.as_secs()
        ),
    }
}

fn backoff(attempt: u32) -> Duration {
    let secs = 1u64 << attempt.saturating_sub(1).min(9);
    jitter(Duration::from_secs(secs).min(BACKOFF_CAP), 0.2).min(BACKOFF_CAP)
}

impl Supervisor {
    pub fn new(shared: Arc<Shared>, cmd_rx: mpsc::Receiver<Command>) -> Self {
        let (log_tx, log_rx) = mpsc::channel(256);
        let (node_tx, node_rx) = mpsc::channel(4);
        let ui: UiState = files::read_json(&shared.dd.ui_state())
            .ok()
            .flatten()
            .unwrap_or_default();
        Self {
            shared,
            cmd_rx,
            log_tx,
            log_rx,
            node_tx,
            node_rx,
            ring: Arc::new(Mutex::new(VecDeque::with_capacity(RING_LINES))),
            wallet: None,
            transaction: None,
            ui,
            child: None,
            generation: 0,
            process: Process::Idle,
            socket_up: false,
            failure: None,
            attempt: 0,
            next_start: None,
            next_start_wall: None,
            exits: VecDeque::new(),
            running_since: None,
            identity: Identity::Unknown,
            identity_since: Instant::now(),
            reported_address: None,
            status: None,
            status_at: None,
            scan_attempt: 0,
            next_scan_start: None,
            condition: None,
            peer: None,
            peer_checked: None,
            node_check_running: false,
            oracle_tip: None,
            persisted: None,
            state_checked: None,
            samples: VecDeque::new(),
            last_state: None,
            ui_saved_at: None,
        }
    }

    fn dd(&self) -> &DataDir {
        &self.shared.dd
    }

    fn save_ui_state(&self) {
        if let Err(e) = files::write_json(&self.dd().ui_state(), &self.ui) {
            tracing::warn!(error = %e, "cannot write ui-state.json");
        }
    }

    /// Boot: finish or roll back an interrupted wallet change, then load the wallet.
    fn boot(&mut self) {
        match wallet::recover(self.dd()) {
            Ok(Recovery::Clean) => {}
            Ok(Recovery::Forgot) => tracing::info!("finished an interrupted forget"),
            Ok(Recovery::RolledBack(kind)) => {
                tracing::warn!(?kind, "rolled back an interrupted wallet change")
            }
            Ok(Recovery::Resume(intent)) => {
                tracing::info!(kind = ?intent.kind, staged = intent.staged, "resuming wallet change: verifying the new wallet");
                self.transaction = Some(intent);
            }
            Err(e) => tracing::error!(error = %e, "wallet recovery failed"),
        }
        self.reload_wallet();
        if self.wallet.is_some() && self.shared.cfg.unsupported_network.is_none() {
            self.next_start = Some(Instant::now());
        }
    }

    fn reload_wallet(&mut self) {
        self.wallet = match wallet::load_keys(self.dd()) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!(error = %e, "cannot read the wallet files");
                None
            }
        };
        if let Some((_, meta)) = &self.wallet
            && meta.network != self.shared.cfg.network
            && self.shared.cfg.unsupported_network.is_none()
        {
            self.halt(
                Failure::IdentityMismatch,
                format!(
                    "the wallet was set up for {} but this node runs {}; set the wallet up again",
                    meta.network, self.shared.cfg.network
                ),
            );
        }
    }

    fn expected_address(&self) -> Option<String> {
        self.wallet
            .as_ref()
            .map(|(keys, _)| keys.identity(self.shared.cfg.network).sp_address)
    }

    pub async fn run(mut self, shutdown: CancellationToken) {
        self.boot();
        self.publish();
        let mut poll = tokio::time::interval(POLL_EVERY);
        poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            if self.should_start() {
                self.start_friglet().await;
                self.publish();
                continue;
            }
            let wake = self
                .next_start
                .unwrap_or_else(|| Instant::now() + Duration::from_secs(3600));
            tokio::select! {
                _ = shutdown.cancelled() => {
                    tracing::info!("stopping friglet before exit");
                    self.stop_friglet(STOP_GRACE).await;
                    self.publish();
                    break;
                }
                Some(cmd) = self.cmd_rx.recv() => self.handle(cmd).await,
                Some((generation, cond)) = self.log_rx.recv() => self.on_log(generation, cond),
                Some(result) = self.node_rx.recv() => self.on_node_check(result),
                status = wait_child(&mut self.child) => self.on_exit(status).await,
                _ = poll.tick() => self.on_poll().await,
                _ = tokio::time::sleep_until(wake.into()) => {}
            }
            self.publish();
        }
    }

    fn should_start(&self) -> bool {
        self.child.is_none()
            && self.wallet.is_some()
            && self.shared.cfg.unsupported_network.is_none()
            && self.process != Process::Halted
            && self.next_start.is_some_and(|t| t <= Instant::now())
    }

    fn halt(&mut self, f: Failure, detail: String) {
        tracing::error!(failure = ?f, "{detail}");
        self.process = Process::Halted;
        self.next_start = None;
        self.next_start_wall = None;
        self.record_failure(f, detail);
    }

    fn record_failure(&mut self, f: Failure, detail: String) {
        self.ui.last_failure = Some(FailureRecord {
            class: f,
            detail: redact::redact(&detail),
            at: now_rfc3339(),
        });
        self.failure = Some((f, detail));
        self.save_ui_state();
    }

    fn schedule_retry(&mut self, f: Failure, detail: String) {
        if f.is_permanent() {
            self.halt(f, detail);
            return;
        }
        self.attempt += 1;
        let delay = backoff(self.attempt);
        tracing::warn!(failure = ?f, retry_in_s = delay.as_secs(), "{detail}");
        self.record_failure(f, detail);
        self.process = Process::BackingOff;
        self.next_start = Some(Instant::now() + delay);
        self.next_start_wall = Some(SystemTime::now() + delay);
    }

    // ------------------------------------------------------------------------------------
    // Start / stop
    // ------------------------------------------------------------------------------------

    async fn start_friglet(&mut self) {
        let Some((_, meta)) = self.wallet.clone() else {
            return;
        };
        self.next_start = None;
        self.next_start_wall = None;
        self.process = Process::Starting;
        self.publish();

        // Node check: re-resolve to a literal IP and handshake on every start.
        let peer = match p2p::check_node(&self.shared.cfg.node, self.shared.cfg.network).await {
            Ok(p) => p,
            Err(e) => {
                self.schedule_retry(
                    Failure::PeerUnreachable,
                    format!("{e} ({})", self.shared.cfg.node),
                );
                return;
            }
        };
        self.set_peer(peer.clone());
        if peer.is_pruned() {
            self.halt(
                Failure::NodePruned,
                format!(
                    "the node at {} is pruned (NODE_NETWORK_LIMITED without NODE_NETWORK)",
                    self.shared.cfg.node
                ),
            );
            return;
        }

        let config = match wallet::render_friglet_config(
            self.dd(),
            &meta,
            &self.shared.cfg.oracle_url,
            peer.addr,
        ) {
            Ok(c) => c,
            Err(e) => {
                self.halt(
                    Failure::Unknown,
                    format!("cannot render friglet's config: {e}"),
                );
                return;
            }
        };
        if let Err(e) = files::write_atomic(&self.dd().config(), config.as_bytes()) {
            self.halt(
                Failure::Unknown,
                format!("cannot write friglet's config: {e}"),
            );
            return;
        }

        match self.spawn() {
            Ok(handle) => {
                self.generation += 1;
                tracing::info!(generation = self.generation, peer = %peer.addr, "friglet started");
                self.child = Some(handle);
                self.socket_up = false;
                self.identity = Identity::Pending;
                self.identity_since = Instant::now();
                self.reported_address = None;
                self.status = None;
                self.condition = None;
                self.samples.clear();
            }
            Err(e) => {
                let detail = format!(
                    "cannot start friglet ({}): {e}",
                    self.shared.cfg.friglet_bin.display()
                );
                self.schedule_retry(Failure::Unknown, detail);
            }
        }
    }

    fn spawn(&mut self) -> io::Result<ChildHandle> {
        let dd = self.dd().clone();
        let mut cmd = ProcessCommand::new(&self.shared.cfg.friglet_bin);
        cmd.arg("scan")
            .arg("--config")
            .arg(dd.config())
            .env_clear()
            .env(
                "PATH",
                "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
            )
            .env("HOME", dd.root())
            .env(
                "TZ",
                std::env::var("TZ").unwrap_or_else(|_| "UTC".to_string()),
            )
            .current_dir(dd.root())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        // SAFETY: only async-signal-safe libc calls between fork and exec.
        unsafe {
            cmd.pre_exec(|| {
                libc::umask(0o077);
                // Take friglet down with friglet-ui if friglet-ui dies without cleanup.
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                Ok(())
            });
        }
        let mut child = cmd.spawn()?;
        self.ring.lock().unwrap().clear();
        let generation = self.generation + 1;
        let mut readers = Vec::new();
        if let Some(out) = child.stdout.take() {
            readers.push(spawn_reader(
                out,
                generation,
                self.ring.clone(),
                self.log_tx.clone(),
            ));
        }
        if let Some(err) = child.stderr.take() {
            readers.push(spawn_reader(
                err,
                generation,
                self.ring.clone(),
                self.log_tx.clone(),
            ));
        }
        Ok(ChildHandle {
            child,
            started: Instant::now(),
            readers,
            deadline_kill: None,
        })
    }

    /// Stop friglet: `Shutdown` over the control socket (SIGTERM when it does not answer),
    /// then SIGKILL after `grace`.
    async fn stop_friglet(&mut self, grace: Duration) {
        self.shared.electrum.close();
        let Some(mut handle) = self.child.take() else {
            return;
        };
        self.generation += 1; // drop late log events of the old process
        let mut asked = false;
        if self.socket_up
            && let Ok(Response::Ok) = crate::ipc::request(
                &self.dd().friglet_socket(),
                &Request::Shutdown,
                Duration::from_secs(5),
            )
            .await
        {
            asked = true;
        }
        if !asked && let Some(pid) = handle.child.id() {
            // SAFETY: plain kill(2) on our own child.
            unsafe {
                libc::kill(pid as i32, libc::SIGTERM);
            }
        }
        match tokio::time::timeout(grace, handle.child.wait()).await {
            Ok(Ok(status)) => tracing::info!(status = %status, "friglet stopped"),
            Ok(Err(e)) => tracing::warn!(error = %e, "waiting for friglet failed"),
            Err(_) => {
                tracing::warn!(
                    grace_s = grace.as_secs(),
                    "friglet did not stop in time; killing it"
                );
                let _ = handle.child.kill().await;
            }
        }
        for r in handle.readers {
            let _ = tokio::time::timeout(Duration::from_secs(2), r).await;
        }
        self.socket_up = false;
        self.running_since = None;
        if self.identity != Identity::Mismatch {
            self.identity = Identity::Unknown;
        }
        if self.process != Process::Halted {
            self.process = Process::Idle;
        }
    }

    async fn on_exit(&mut self, status: io::Result<ExitStatus>) {
        let Some(handle) = self.child.take() else {
            return;
        };
        for r in handle.readers {
            let _ = tokio::time::timeout(Duration::from_secs(2), r).await;
        }
        self.generation += 1;
        self.shared.electrum.close();
        self.socket_up = false;
        self.running_since = None;
        let lines: Vec<String> = self.ring.lock().unwrap().iter().cloned().collect();
        let kind = if handle.deadline_kill.is_some() {
            ExitKind::StartupDeadline
        } else {
            match &status {
                Ok(s) => exit_kind(s),
                Err(_) => ExitKind::Code(-1),
            }
        };
        self.ui.restarts += 1;
        if self.identity == Identity::Mismatch {
            self.process = Process::Halted;
            return;
        }
        self.identity = Identity::Unknown;
        let failure = classify::classify_exit(kind, &lines);
        let mut detail = describe_exit(kind);
        if let Some(which) = classify::port_in_use(&lines) {
            detail = format!("{detail}: {which} is already in use");
        } else if let Some(line) = lines
            .iter()
            .rev()
            .find(|l| l.starts_with("Error:") || l.contains("panicked at") || l.contains("ERROR"))
        {
            detail = format!("{detail}: {}", redact::redact(line.trim()));
        }
        let now = Instant::now();
        self.exits.push_back(now);
        while self
            .exits
            .front()
            .is_some_and(|t| now.duration_since(*t) > CRASH_WINDOW)
        {
            self.exits.pop_front();
        }
        self.schedule_retry(failure, detail);
    }

    // ------------------------------------------------------------------------------------
    // Polling
    // ------------------------------------------------------------------------------------

    async fn on_poll(&mut self) {
        self.maybe_refresh_node();
        let Some(started) = self.child.as_ref().map(|h| h.started) else {
            return;
        };
        let socket = self.dd().friglet_socket();
        match crate::ipc::request(&socket, &Request::GetStatus, Duration::from_secs(5)).await {
            Ok(Response::Status(status)) => {
                if !self.socket_up {
                    tracing::info!("friglet control socket answers");
                }
                self.socket_up = true;
                self.process = Process::Running;
                self.running_since.get_or_insert_with(Instant::now);
                self.on_status(status).await;
            }
            Ok(other) => tracing::debug!(?other, "unexpected GetStatus answer"),
            Err(e) => {
                if self.socket_up {
                    tracing::debug!(error = %e, "GetStatus failed");
                } else if started.elapsed() > STARTUP_DEADLINE {
                    self.kill_for_deadline();
                }
            }
        }
        // Healthy for long enough: forget the restart backoff.
        if self.identity == Identity::Verified
            && self
                .running_since
                .is_some_and(|t| t.elapsed() > HEALTHY_RESET)
            && self.attempt > 0
        {
            self.attempt = 0;
            self.failure = None;
        }
    }

    fn kill_for_deadline(&mut self) {
        let Some(handle) = self.child.as_mut() else {
            return;
        };
        match handle.deadline_kill {
            None => {
                tracing::warn!(
                    "friglet's control socket did not come up in time; stopping friglet"
                );
                handle.deadline_kill = Some(Instant::now());
                if let Some(pid) = handle.child.id() {
                    // SAFETY: kill(2) on our own child.
                    unsafe {
                        libc::kill(pid as i32, libc::SIGTERM);
                    }
                }
            }
            Some(t) if t.elapsed() > Duration::from_secs(15) => {
                let _ = handle.child.start_kill();
            }
            Some(_) => {}
        }
    }

    async fn on_status(&mut self, status: StatusInfo) {
        let now = Instant::now();
        self.status_at = Some(SystemTime::now());

        // Identity.
        if self.identity == Identity::Pending
            && let Some(reported) = status.sp_address.clone()
        {
            let expected = self.expected_address().unwrap_or_default();
            if reported == expected {
                tracing::info!("wallet identity verified: friglet scans the configured wallet");
                self.identity = Identity::Verified;
                self.shared.electrum.open();
                let _ = wallet::mark_verified(self.dd());
                if let Some(t) = &mut self.transaction
                    && t.step == TxStep::Written
                {
                    t.step = TxStep::Verified;
                    let _ = wallet::mark_step(self.dd(), TxStep::Verified);
                }
            } else {
                self.identity = Identity::Mismatch;
                self.reported_address = Some(reported);
                self.shared.electrum.close();
                self.halt(
                    Failure::IdentityMismatch,
                    "friglet reports a different Silent Payments address than the configured wallet; friglet stopped".to_string(),
                );
                self.stop_friglet(STOP_GRACE).await;
                self.process = Process::Halted;
                self.status = Some(status);
                return;
            }
        }

        // Heights.
        if status.oracle_connected {
            self.oracle_tip = status.tip_height;
        }
        let scanned = status.scanned_height;
        if self.identity == Identity::Verified {
            self.samples.push_back((now, scanned));
            while self
                .samples
                .front()
                .is_some_and(|(t, _)| now.duration_since(*t) > Duration::from_secs(600))
            {
                self.samples.pop_front();
            }
            if let Some(addr) = self.expected_address() {
                let max = match &self.ui.max_scanned {
                    Some((a, m)) if *a == addr => (*m).max(scanned),
                    _ => scanned,
                };
                if self.ui.max_scanned.as_ref().map(|(_, m)| *m) != Some(max)
                    || self.ui.max_scanned.as_ref().map(|(a, _)| a) != Some(&addr)
                {
                    self.ui.max_scanned = Some((addr, max));
                    // Persist at most once a minute (SD cards); it only feeds the rescan
                    // banner across friglet-ui restarts.
                    if self
                        .ui_saved_at
                        .is_none_or(|t| t.elapsed() > Duration::from_secs(60))
                    {
                        self.ui_saved_at = Some(now);
                        self.save_ui_state();
                    }
                }
            }
            if let Some((_, _, at_height)) = &self.condition
                && scanned > *at_height
            {
                self.condition = None;
            }
        }

        // Scan task: pause, and restart after it stopped.
        if self.identity == Identity::Verified {
            let paused = self.ui.desired == Desired::Paused;
            if paused && status.scanning {
                let _ = crate::ipc::request(
                    &self.dd().friglet_socket(),
                    &Request::Stop,
                    Duration::from_secs(30),
                )
                .await;
            } else if !paused && !status.scanning {
                let due = self.next_scan_start.is_none_or(|t| t <= now);
                if due {
                    self.scan_attempt += 1;
                    let delay = backoff(self.scan_attempt);
                    tracing::warn!(
                        last_error = status
                            .last_error
                            .as_deref()
                            .map(redact::redact)
                            .unwrap_or_default(),
                        retry_in_s = delay.as_secs(),
                        "friglet's scan task is not running; sending Start"
                    );
                    self.next_scan_start = Some(now + delay);
                    let _ = crate::ipc::request(
                        &self.dd().friglet_socket(),
                        &Request::Start,
                        Duration::from_secs(30),
                    )
                    .await;
                }
            } else if status.scanning && self.next_scan_start.is_some() {
                // Running again; keep the backoff until progress shows.
                if self.samples.len() >= 2
                    && self.samples.back().map(|s| s.1) > self.samples.front().map(|s| s.1)
                {
                    self.scan_attempt = 0;
                    self.next_scan_start = None;
                }
            }
        }

        // Persisted height, transaction commit, prev/ expiry.
        // Every 30 s; every poll while a wallet change waits for its first saved scan.
        let every = if self.transaction.is_some() {
            POLL_EVERY
        } else {
            STATE_FILE_CHECK
        };
        if self.state_checked.is_none_or(|t| t.elapsed() >= every) {
            self.state_checked = Some(now);
            self.check_state_file();
        }
        self.status = Some(status);
    }

    fn check_state_file(&mut self) {
        let Some((keys, meta)) = self.wallet.clone() else {
            return;
        };
        let check = match wallet::check_state_file(self.dd(), &keys) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(error = %e, "cannot read friglet's state file");
                return;
            }
        };
        let Some(check) = check else { return };
        self.persisted = check.persisted_height;
        let sound = check.matches_wallet
            && check
                .persisted_height
                .is_some_and(|h| h >= meta.birthday_height);
        if self.identity != Identity::Verified || !sound {
            return;
        }
        if self
            .transaction
            .as_ref()
            .is_some_and(|t| t.step >= TxStep::Written)
        {
            match wallet::commit(self.dd()) {
                Ok(()) => {
                    tracing::info!(
                        "wallet change complete: identity verified and first scan saved"
                    );
                    self.transaction = None;
                    if self.dd().prev().exists() {
                        self.ui.prev_expires_at = Some(rfc3339(SystemTime::now() + PREV_KEEP));
                    }
                    self.save_ui_state();
                }
                Err(e) => tracing::error!(error = %e, "cannot commit the wallet change"),
            }
        }
        if self.transaction.is_none() && self.dd().prev().exists() {
            match self.ui.prev_expires_at.as_deref().and_then(parse_rfc3339) {
                None => {
                    self.ui.prev_expires_at = Some(rfc3339(SystemTime::now() + PREV_KEEP));
                    self.save_ui_state();
                }
                Some(t) if t <= SystemTime::now() => {
                    match wallet::delete_prev(self.dd()) {
                        Ok(()) => tracing::info!(
                            "previous wallet snapshot deleted (24 h after verified progress)"
                        ),
                        Err(e) => tracing::warn!(error = %e, "cannot delete prev/"),
                    }
                    self.ui.prev_expires_at = None;
                    self.save_ui_state();
                }
                Some(_) => {}
            }
        }
    }

    fn on_log(&mut self, generation: u64, cond: LogCondition) {
        if generation != self.generation {
            return;
        }
        let scanned = self.status.as_ref().map(|s| s.scanned_height).unwrap_or(0);
        match cond {
            LogCondition::Progress => self.condition = None,
            other => self.condition = Some((other, Instant::now(), scanned)),
        }
    }

    fn set_peer(&mut self, peer: PeerInfo) {
        *self.shared.last_peer.write().unwrap() = Some(peer.clone());
        self.peer = Some(peer);
        self.peer_checked = Some(Instant::now());
    }

    fn maybe_refresh_node(&mut self) {
        if self.node_check_running || self.shared.cfg.unsupported_network.is_some() {
            return;
        }
        let stale = self.peer_checked.is_none_or(|t| t.elapsed() > NODE_REFRESH);
        // When the node looks behind the oracle, confirm with a fresh handshake.
        let lagging = matches!((self.peer.as_ref().and_then(PeerInfo::tip), self.oracle_tip),
            (Some(n), Some(o)) if n + states::NODE_LAG_BLOCKS < o)
            && self
                .peer_checked
                .is_some_and(|t| t.elapsed() > Duration::from_secs(60));
        // Before setup the tip bounds the birthday; with a wallet the pre-start handshake
        // covers the time friglet is not running.
        if !(stale || lagging) || (self.wallet.is_some() && self.child.is_none()) {
            return;
        }
        self.node_check_running = true;
        let tx = self.node_tx.clone();
        let node = self.shared.cfg.node.clone();
        let network = self.shared.cfg.network;
        tokio::spawn(async move {
            let _ = tx.send(p2p::check_node(&node, network).await).await;
        });
    }

    fn on_node_check(&mut self, result: Result<PeerInfo, p2p::P2pError>) {
        self.node_check_running = false;
        match result {
            Ok(peer) => self.set_peer(peer),
            Err(e) => {
                tracing::warn!(error = %e, "node tip refresh failed");
                self.peer_checked = Some(Instant::now());
            }
        }
    }

    // ------------------------------------------------------------------------------------
    // Commands
    // ------------------------------------------------------------------------------------

    /// Handle a command; the new state is published before the reply goes out, so a 202
    /// body can carry it.
    async fn handle(&mut self, cmd: Command) {
        match cmd {
            Command::Setup {
                keys,
                birthday,
                replace,
                reply,
            } => {
                let outcome = self.cmd_setup(*keys, birthday, replace).await;
                self.publish();
                let _ = reply.send(outcome);
            }
            Command::Forget { reply } => {
                let r = self.cmd_forget().await;
                self.publish();
                let _ = reply.send(r);
            }
            Command::RestorePrevious { reply } => {
                let r = self.cmd_restore().await;
                self.publish();
                let _ = reply.send(r);
            }
            Command::DiscardPrevious { reply } => {
                let r = if self.transaction.is_some() {
                    Err(
                        "a wallet change is in progress; the previous wallet is its safety net"
                            .to_string(),
                    )
                } else if !self.dd().prev().exists() {
                    Ok(false)
                } else {
                    match wallet::delete_prev(self.dd()) {
                        Ok(()) => {
                            tracing::info!("previous wallet snapshot deleted by the user");
                            self.ui.prev_expires_at = None;
                            self.save_ui_state();
                            Ok(true)
                        }
                        Err(e) => Err(format!("cannot delete the previous wallet: {e}")),
                    }
                };
                self.publish();
                let _ = reply.send(r);
            }
            Command::Pause { reply } => {
                self.ui.desired = Desired::Paused;
                self.save_ui_state();
                if self.socket_up {
                    let _ = crate::ipc::request(
                        &self.dd().friglet_socket(),
                        &Request::Stop,
                        Duration::from_secs(30),
                    )
                    .await;
                }
                self.publish();
                let _ = reply.send(());
            }
            Command::Resume { reply } => {
                self.ui.desired = Desired::Run;
                self.save_ui_state();
                self.scan_attempt = 0;
                self.next_scan_start = None;
                match self.process {
                    Process::Halted if self.identity != Identity::Mismatch => {
                        self.process = Process::Idle;
                        self.failure = None;
                        self.attempt = 0;
                        self.next_start = Some(Instant::now());
                    }
                    Process::BackingOff | Process::Idle => self.next_start = Some(Instant::now()),
                    _ => {}
                }
                if self.socket_up {
                    let _ = crate::ipc::request(
                        &self.dd().friglet_socket(),
                        &Request::Start,
                        Duration::from_secs(30),
                    )
                    .await;
                }
                self.publish();
                let _ = reply.send(());
            }
        }
    }

    fn reset_for_new_wallet(&mut self) {
        self.failure = None;
        self.attempt = 0;
        self.exits.clear();
        self.identity = Identity::Unknown;
        self.reported_address = None;
        self.status = None;
        self.condition = None;
        self.oracle_tip = None;
        self.persisted = None;
        self.samples.clear();
        self.scan_attempt = 0;
        self.next_scan_start = None;
        self.process = Process::Idle;
        self.state_checked = None;
        self.ui.desired = Desired::Run;
        self.ui.max_scanned = None;
        self.ui.last_failure = None;
        self.save_ui_state();
    }

    async fn cmd_setup(&mut self, keys: WalletKeys, birthday: u64, replace: bool) -> Outcome {
        if let Some(msg) = &self.shared.cfg.unsupported_network {
            return Outcome::err(
                setup::EXIT_NETWORK,
                "unsupported-network",
                None,
                msg.clone(),
            );
        }
        let network = self.shared.cfg.network;
        if let Err(e) = keys.check_network(network) {
            return setup::descriptor_outcome(&e);
        }
        if let Err(m) = wallet::validate_birthday(
            network,
            birthday,
            self.peer.as_ref().and_then(PeerInfo::tip),
        ) {
            return Outcome::err(
                setup::EXIT_BIRTHDAY,
                "birthday-invalid",
                Some("birthday_height"),
                m,
            );
        }
        let identity = keys.identity(network);
        let plan = match wallet::plan_setup(self.dd(), &identity, birthday) {
            Ok(p) => p,
            Err(e) => {
                return Outcome::err(
                    setup::EXIT_IO,
                    "io",
                    None,
                    format!("cannot read the wallet files: {e}"),
                );
            }
        };
        let kind = match plan {
            // Re-entering the same wallet after a mismatch starts it fresh.
            SetupPlan::NoOp
                if self.transaction.is_none() && self.identity != Identity::Mismatch =>
            {
                return Outcome::ok(
                    Some(self.current_state().as_str()),
                    Some(&identity),
                    "this wallet is already set up; nothing changed",
                );
            }
            SetupPlan::NoOp => TxKind::Replace,
            SetupPlan::Replace { current } if !replace => {
                return Outcome::err(
                    setup::EXIT_EXISTS,
                    "wallet-exists",
                    None,
                    setup::exists_message(&current),
                );
            }
            SetupPlan::Replace { .. } => TxKind::Replace,
            SetupPlan::Fresh => TxKind::Setup,
        };
        tracing::info!(?kind, "wallet change requested: stopping friglet");
        self.stop_friglet(STOP_GRACE).await;
        match wallet::stage(self.dd(), &keys, network, birthday, kind, false) {
            Ok(intent) => self.transaction = Some(intent),
            Err(e) => {
                let _ = wallet::recover(self.dd());
                self.reload_wallet();
                self.next_start = Some(Instant::now());
                return Outcome::err(
                    setup::EXIT_IO,
                    "io",
                    None,
                    format!("cannot write the wallet files: {e}"),
                );
            }
        }
        self.reset_for_new_wallet();
        self.reload_wallet();
        self.next_start = Some(Instant::now());
        Outcome::ok(
            Some(StateId::Starting.as_str()),
            Some(&identity),
            "wallet saved; friglet starts and the identity is checked",
        )
    }

    async fn cmd_forget(&mut self) -> Result<(), String> {
        tracing::info!("forget requested: stopping friglet and deleting the wallet files");
        self.stop_friglet(STOP_GRACE).await;
        wallet::forget(self.dd()).map_err(|e| format!("cannot delete the wallet files: {e}"))?;
        self.transaction = None;
        self.wallet = None;
        self.reset_for_new_wallet();
        self.ui.prev_expires_at = None;
        self.save_ui_state();
        self.next_start = None;
        Ok(())
    }

    async fn cmd_restore(&mut self) -> Result<bool, String> {
        if !self.dd().prev().join(files::KEY_FILE).is_file() {
            return Ok(false);
        }
        self.stop_friglet(STOP_GRACE).await;
        match wallet::stage_restore(self.dd()) {
            Ok(Some(intent)) => {
                self.transaction = Some(intent);
                self.reset_for_new_wallet();
                self.reload_wallet();
                self.ui.prev_expires_at = None;
                self.save_ui_state();
                self.next_start = Some(Instant::now());
                Ok(true)
            }
            Ok(None) => {
                self.next_start = Some(Instant::now());
                Ok(false)
            }
            Err(e) => {
                let _ = wallet::recover(self.dd());
                self.reload_wallet();
                self.next_start = Some(Instant::now());
                Err(format!("cannot restore the previous wallet: {e}"))
            }
        }
    }

    // ------------------------------------------------------------------------------------
    // Presentation
    // ------------------------------------------------------------------------------------

    fn scan_error(&self) -> Option<Failure> {
        let status = self.status.as_ref()?;
        if !status.scanning && self.ui.desired != Desired::Paused {
            return Some(
                status
                    .last_error
                    .as_deref()
                    .map(classify::classify_last_error)
                    .unwrap_or(Failure::ScanStopped),
            );
        }
        let (cond, at, _) = self.condition.as_ref()?;
        if at.elapsed() > CONDITION_TTL {
            return None;
        }
        let pruned = self.peer.as_ref().is_some_and(PeerInfo::is_pruned);
        let node_tip = self.peer.as_ref().and_then(PeerInfo::tip);
        match cond {
            LogCondition::OracleRetrying => Some(Failure::OracleDropped),
            LogCondition::P2p { kind, height } => {
                Some(classify::p2p_failure(*kind, *height, node_tip, pruned))
            }
            LogCondition::Progress => None,
        }
    }

    fn inputs(&self) -> Inputs {
        let meta = self.wallet.as_ref().map(|(_, m)| m);
        let scanned = self.status.as_ref().map(|s| s.scanned_height);
        let max_scanned = match (&self.ui.max_scanned, self.expected_address()) {
            (Some((a, m)), Some(addr)) if *a == addr => Some(m.saturating_sub(RESCAN_SLACK)),
            _ => None,
        };
        Inputs {
            unsupported_network: self.shared.cfg.unsupported_network.is_some(),
            configured: self.wallet.is_some(),
            transaction: self.transaction.is_some(),
            desired: self.ui.desired,
            process: self.process,
            failure: self.failure.as_ref().map(|(f, _)| *f),
            crash_loop: self.exits.len() >= CRASH_LOOP_EXITS,
            identity: self.identity,
            scan: self.status.as_ref().map(|s| Scan {
                scanning: s.scanning,
                error: self.scan_error(),
            }),
            heights: ModelHeights {
                birthday: meta.map(|m| m.birthday_height),
                scanned,
                max_scanned,
                node_tip: self.peer.as_ref().and_then(PeerInfo::tip),
                oracle_tip: self.oracle_tip,
            },
        }
    }

    fn current_state(&self) -> StateId {
        states::derive(&self.inputs())
    }

    fn publish(&mut self) {
        let inputs = self.inputs();
        let id = states::derive(&inputs);
        if self.last_state != Some(id) {
            tracing::info!(state = id.as_str(), "state");
            self.last_state = Some(id);
        }
        let def = self.shared.catalog.get(id).clone();
        let mut detail = def.detail.clone();
        match id {
            StateId::UnsupportedNetwork => {
                if let Some(m) = &self.shared.cfg.unsupported_network {
                    detail = format!("{detail} ({m})");
                }
            }
            StateId::IdentityMismatch => {
                let expected = self.expected_address().unwrap_or_default();
                match &self.reported_address {
                    Some(r) => {
                        detail = format!("{detail} Expected {expected}; friglet reports {r}.")
                    }
                    None => {
                        if let Some((_, d)) = &self.failure {
                            detail = format!("{detail} {d}");
                        }
                    }
                }
            }
            StateId::VerifyingIdentity if self.identity_since.elapsed() > IDENTITY_NOTE_AFTER => {
                detail = format!(
                    "{detail} friglet has not reported an address for {} minutes; open Diagnostics.",
                    self.identity_since.elapsed().as_secs() / 60
                );
            }
            StateId::NeedsSetup | StateId::Scanning | StateId::UpToDate | StateId::Paused => {}
            _ => {
                if let Some((_, d)) = &self.failure
                    && matches!(self.process, Process::BackingOff | Process::Halted)
                {
                    detail = format!("{detail} Last error: {}", redact::redact(d));
                }
            }
        }
        let next_retry_at = match self.process {
            Process::BackingOff => self.next_start_wall.map(rfc3339),
            _ => None,
        };

        let meta = self.wallet.as_ref().map(|(_, m)| m.clone());
        let heights = HeightsView {
            birthday: meta.as_ref().map(|m| m.birthday_height),
            birthday_time: None,
            scanned: inputs.heights.scanned,
            persisted: self.persisted,
            node_tip: inputs.heights.node_tip,
            oracle_tip: self.oracle_tip,
            updated_at: self.status_at.map(rfc3339),
        };
        let progress = self.progress(&inputs, id);
        let cfg = &self.shared.cfg;
        let reported = self
            .reported_address
            .clone()
            .or_else(|| self.status.as_ref().and_then(|s| s.sp_address.clone()));
        let failed = matches!(self.process, Process::Halted) || self.identity == Identity::Mismatch;
        let transaction = self.transaction.as_ref().map(|t| TransactionView {
            kind: t.kind.api_kind().to_string(),
            step: match t.step {
                TxStep::Begin => "intent",
                TxStep::Snapshotted => "writing",
                TxStep::Written if self.socket_up => "verifying",
                TxStep::Written => "starting",
                TxStep::Verified => "persisting",
            }
            .to_string(),
            failed,
            error: if failed {
                self.failure.as_ref().map(|(_, d)| redact::redact(d))
            } else {
                None
            },
            started_at: t.started_at.clone(),
        });
        let prev_exists = self.dd().prev().join(files::KEY_FILE).is_file();
        let view = StateResponse {
            configured: self.wallet.is_some(),
            network: cfg.network_label(),
            state: StateView {
                id,
                severity: def.severity.clone(),
                headline: def.headline.clone(),
                detail,
                action: def.action.clone(),
                next_retry_at,
            },
            wallet: meta.map(|m| WalletView {
                sp_address: m.sp_address,
                key_fingerprint: m.key_fingerprint,
                network: m.network.as_str().to_string(),
                birthday_height: m.birthday_height,
                identity_verified: self.identity == Identity::Verified,
                reported_sp_address: reported,
            }),
            heights,
            progress,
            payments_found: self.status.as_ref().map(|s| s.tx_count),
            node: NodeView {
                peer: self.peer.as_ref().map(|p| p.addr.to_string()),
                configured: cfg.node.clone(),
                user_agent: self.peer.as_ref().map(|p| p.user_agent.clone()),
            },
            oracle: OracleView {
                url: cfg.oracle_url.clone(),
                third_party: oracle_is_third_party(&cfg.oracle_url),
            },
            versions: VersionsView {
                friglet: crate::friglet_short_rev().to_string(),
                friglet_ui: crate::VERSION.to_string(),
                dev_build: crate::DEV_BUILD,
            },
            restarts: self.ui.restarts,
            paused: self.ui.desired == Desired::Paused,
            transaction,
            previous: Some(PreviousView {
                exists: prev_exists,
                expires_at: if prev_exists {
                    self.ui.prev_expires_at.clone()
                } else {
                    None
                },
            }),
        };
        *self.shared.view.write().unwrap() = view;
    }

    fn progress(&self, inputs: &Inputs, id: StateId) -> ProgressView {
        let h = &inputs.heights;
        let (Some(birthday), Some(scanned), Some(target)) = (h.birthday, h.scanned, h.target())
        else {
            return ProgressView::default();
        };
        let fraction = if target <= birthday {
            if scanned >= target { 1.0 } else { 0.0 }
        } else {
            (scanned.saturating_sub(birthday.saturating_sub(1)) as f64
                / (target - birthday + 1) as f64)
                .clamp(0.0, 1.0)
        };
        let mut eta_seconds = None;
        if matches!(id, StateId::Scanning | StateId::RescanningAfterReset)
            && let (Some((t0, h0)), Some((t1, h1))) = (self.samples.front(), self.samples.back())
        {
            let span = t1.duration_since(*t0).as_secs_f64();
            // Stalled over the last minute: hide the ETA.
            let recent_progress = self
                .samples
                .iter()
                .rev()
                .find(|(t, _)| t1.duration_since(*t) >= Duration::from_secs(60))
                .is_none_or(|(_, h)| h1 > h);
            if span >= 60.0 && h1 > h0 && recent_progress && scanned < target {
                let rate = (h1 - h0) as f64 / span;
                eta_seconds = Some(((target - scanned) as f64 / rate).ceil() as u64);
            }
        }
        ProgressView {
            fraction: Some(fraction),
            eta_seconds,
        }
    }
}

fn oracle_is_third_party(url: &str) -> bool {
    let host = url
        .split("://")
        .nth(1)
        .unwrap_or(url)
        .split(['/', ':'])
        .next()
        .unwrap_or_default();
    !(host == "localhost"
        || host.starts_with("127.")
        || host.starts_with("10.")
        || host.starts_with("192.168.")
        || host.ends_with(".local")
        || !host.contains('.'))
}

async fn wait_child(child: &mut Option<ChildHandle>) -> io::Result<ExitStatus> {
    match child {
        Some(h) => h.child.wait().await,
        None => std::future::pending().await,
    }
}

fn spawn_reader<R>(
    stream: R,
    generation: u64,
    ring: Arc<Mutex<VecDeque<String>>>,
    tx: mpsc::Sender<(u64, LogCondition)>,
) -> JoinHandle<()>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(stream).lines();
        while let Ok(Some(raw)) = lines.next_line().await {
            let line = redact::strip_ansi(&raw);
            if let Some(cond) = classify::classify_line(&line) {
                let _ = tx.try_send((generation, cond));
            }
            tracing::info!(target: "friglet", "{}", redact::redact(&line));
            let mut ring = ring.lock().unwrap();
            if ring.len() >= RING_LINES {
                ring.pop_front();
            }
            ring.push_back(line);
        }
    })
}

/// Initial (empty) view before the supervisor publishes.
pub fn initial_view(cfg: &UiConfig, catalog: &Catalog) -> StateResponse {
    let def = catalog.get(StateId::Starting);
    StateResponse {
        configured: false,
        network: cfg.network_label(),
        state: StateView {
            id: StateId::Starting,
            severity: def.severity.clone(),
            headline: def.headline.clone(),
            detail: def.detail.clone(),
            action: def.action.clone(),
            next_retry_at: None,
        },
        wallet: None,
        heights: HeightsView::default(),
        progress: ProgressView::default(),
        payments_found: None,
        node: NodeView {
            peer: None,
            configured: cfg.node.clone(),
            user_agent: None,
        },
        oracle: OracleView {
            url: cfg.oracle_url.clone(),
            third_party: oracle_is_third_party(&cfg.oracle_url),
        },
        versions: VersionsView {
            friglet: crate::friglet_short_rev().to_string(),
            friglet_ui: crate::VERSION.to_string(),
            dev_build: crate::DEV_BUILD,
        },
        restarts: 0,
        paused: false,
        transaction: None,
        previous: None,
    }
}

/// Socket path for friglet-ui's own control socket (for documentation in errors).
pub fn ui_socket_path(dd: &DataDir) -> PathBuf {
    dd.ui_socket()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_is_capped_and_grows() {
        assert!(backoff(1) <= Duration::from_secs(2));
        assert!(backoff(5) >= Duration::from_secs(12));
        for a in 9..40 {
            assert!(backoff(a) <= BACKOFF_CAP);
        }
        assert!(backoff(12) >= Duration::from_secs(240));
    }

    #[test]
    fn third_party_oracle() {
        assert!(oracle_is_third_party("https://oracle.setor.dev"));
        assert!(oracle_is_third_party(
            "https://signet.oracle.setor.dev:443/x"
        ));
        assert!(!oracle_is_third_party("http://127.0.0.1:7001"));
        assert!(!oracle_is_third_party("http://oracle:7001"));
        assert!(!oracle_is_third_party("http://192.168.1.4:7001"));
    }

    #[test]
    fn exit_descriptions() {
        assert!(describe_exit(ExitKind::Signal(6)).contains("panic"));
        assert!(describe_exit(ExitKind::Code(1)).contains("code 1"));
        assert!(describe_exit(ExitKind::StartupDeadline).contains("oracle"));
    }
}
