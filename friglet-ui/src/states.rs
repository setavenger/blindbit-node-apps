//! State catalog (`states.toml`) and the state model: four inputs (desired operation,
//! process lifecycle, scan condition, wallet transaction) plus node and identity checks,
//! combined with an explicit precedence into one presentation state id.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const CATALOG_TOML: &str = include_str!("../states.toml");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StateId {
    NeedsSetup,
    Starting,
    VerifyingIdentity,
    IdentityMismatch,
    Scanning,
    UpToDate,
    ScanStopped,
    Paused,
    OracleUnreachable,
    OracleDroppedResuming,
    PeerUnreachable,
    NodePruned,
    NodeSyncing,
    PortInUse,
    RescanningAfterReset,
    CrashLoop,
    UnknownExit,
    UnsupportedNetwork,
}

impl StateId {
    pub const ALL: [StateId; 18] = [
        StateId::NeedsSetup,
        StateId::Starting,
        StateId::VerifyingIdentity,
        StateId::IdentityMismatch,
        StateId::Scanning,
        StateId::UpToDate,
        StateId::ScanStopped,
        StateId::Paused,
        StateId::OracleUnreachable,
        StateId::OracleDroppedResuming,
        StateId::PeerUnreachable,
        StateId::NodePruned,
        StateId::NodeSyncing,
        StateId::PortInUse,
        StateId::RescanningAfterReset,
        StateId::CrashLoop,
        StateId::UnknownExit,
        StateId::UnsupportedNetwork,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            StateId::NeedsSetup => "needs-setup",
            StateId::Starting => "starting",
            StateId::VerifyingIdentity => "verifying-identity",
            StateId::IdentityMismatch => "identity-mismatch",
            StateId::Scanning => "scanning",
            StateId::UpToDate => "up-to-date",
            StateId::ScanStopped => "scan-stopped",
            StateId::Paused => "paused",
            StateId::OracleUnreachable => "oracle-unreachable",
            StateId::OracleDroppedResuming => "oracle-dropped-resuming",
            StateId::PeerUnreachable => "peer-unreachable",
            StateId::NodePruned => "node-pruned",
            StateId::NodeSyncing => "node-syncing",
            StateId::PortInUse => "port-in-use",
            StateId::RescanningAfterReset => "rescanning-after-reset",
            StateId::CrashLoop => "crash-loop",
            StateId::UnknownExit => "unknown-exit",
            StateId::UnsupportedNetwork => "unsupported-network",
        }
    }

    /// `friglet-ui health` passes only here: scanning (including a rescan after a reset)
    /// and up to date.
    pub fn is_healthy(self) -> bool {
        matches!(
            self,
            StateId::Scanning | StateId::UpToDate | StateId::RescanningAfterReset
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionDef {
    pub label: String,
    pub kind: String,
    pub target: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateDef {
    pub severity: String,
    pub class: String,
    pub headline: String,
    pub detail: String,
    pub action: ActionDef,
    pub widget: String,
    pub health: String,
    pub cause: String,
    pub fix: String,
    pub anchor: String,
    #[serde(default)]
    pub browser: bool,
}

#[derive(Debug, Clone)]
pub struct Catalog {
    entries: BTreeMap<String, StateDef>,
}

impl Catalog {
    pub fn load() -> Self {
        Self::parse(CATALOG_TOML).expect("states.toml is valid (checked by tests)")
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let entries: BTreeMap<String, StateDef> =
            toml::from_str(text).map_err(|e| format!("states.toml: {e}"))?;
        for id in StateId::ALL {
            if !entries.contains_key(id.as_str()) {
                return Err(format!("states.toml: missing state `{}`", id.as_str()));
            }
        }
        for (id, def) in &entries {
            if !matches!(def.severity.as_str(), "ok" | "info" | "warn" | "error") {
                return Err(format!(
                    "states.toml: `{id}` has severity `{}`",
                    def.severity
                ));
            }
            if !matches!(
                def.class.as_str(),
                "progress" | "setup" | "transient" | "permanent"
            ) {
                return Err(format!("states.toml: `{id}` has class `{}`", def.class));
            }
            if !matches!(def.action.kind.as_str(), "link" | "post" | "none") {
                return Err(format!(
                    "states.toml: `{id}` has action kind `{}`",
                    def.action.kind
                ));
            }
            if !matches!(def.health.as_str(), "ok" | "starting" | "attention") {
                return Err(format!("states.toml: `{id}` has health `{}`", def.health));
            }
            if def.headline.is_empty() || def.detail.is_empty() || def.widget.is_empty() {
                return Err(format!(
                    "states.toml: `{id}` lacks headline, detail or widget text"
                ));
            }
        }
        Ok(Self { entries })
    }

    pub fn get(&self, id: StateId) -> &StateDef {
        &self.entries[id.as_str()]
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.entries.keys().map(String::as_str)
    }
}

// ---------------------------------------------------------------------------------------------
// docs/troubleshooting.md
// ---------------------------------------------------------------------------------------------

/// Refusals outside the state catalog whose texts cite a troubleshooting anchor:
/// (anchor, title, cause, fix).
pub const REFUSALS: [(&str, &str, &str, &str); 3] = [
    (
        "stray-friglet-variable",
        "friglet-ui refuses to start: a FRIGLET_* variable is set",
        "An environment variable starting with FRIGLET_ but not FRIGLET_UI_ is set. friglet-ui starts friglet with a cleared environment, so such a variable would silently do nothing (or, set by hand on friglet, override the managed configuration).",
        "Remove the variable from the container or service environment; use the matching FRIGLET_UI_* setting (docs/contract.md).",
    ),
    (
        "missing-ui-password",
        "friglet-ui refuses to start: no UI password",
        "FRIGLET_UI_AUTH=password (the default outside Umbrel) and no password hash is stored in the data directory yet.",
        "Compose: set FRIGLET_UI_PASSWORD. Native installs and StartOS: write the password to the file named by FRIGLET_UI_PASSWORD_FILE (read on every start). Or run `friglet-ui password set` with the password on stdin.",
    ),
    (
        "wallet-exists",
        "Setup refused: a different wallet is already set up",
        "`friglet-ui setup` (exit code 7) or POST /api/setup (409 wallet-exists) was given another descriptor or birthday than the configured wallet.",
        "Confirm the replacement: `friglet-ui setup --replace`, or confirm the change in the web UI. The old wallet is kept in prev/ for 24 hours and can be restored.",
    ),
];

/// Render docs/troubleshooting.md from the catalog (one `###` heading, hence one anchor,
/// per state id and refusal).
pub fn troubleshooting_markdown(c: &Catalog) -> String {
    let mut out = String::new();
    out.push_str("# Troubleshooting\n\n");
    out.push_str(
        "<!-- Generated from friglet-ui/states.toml by friglet-ui's tests. Do not edit by hand.\n     \
         Regenerate: FRIGLET_UI_REGEN_DOCS=1 cargo test -p friglet-ui troubleshooting -->\n\n",
    );
    out.push_str(
        "Every state the friglet status page, the Umbrel widget and the StartOS health check can show, \
         with what friglet-ui saw and what to do. Anchors are the state ids (`#node-pruned`).\n\n",
    );
    out.push_str("| State | Severity | Retry | Widget |\n|---|---|---|---|\n");
    for (id, d) in &c.entries {
        let retry = match d.class.as_str() {
            "transient" => "automatic",
            "permanent" => "after you act",
            _ => "-",
        };
        out.push_str(&format!(
            "| [`{id}`](#{id}) | {} | {retry} | {} |\n",
            d.severity, d.widget
        ));
    }
    out.push_str("\n## States\n");
    for (id, d) in &c.entries {
        out.push_str(&format!("\n### {id}\n\n**{}**", d.headline));
        if d.browser {
            out.push_str(" (shown by the web page itself)");
        }
        out.push_str(&format!(
            "\n\n{}\n\n- Severity: {} ({}).\n- How friglet-ui detects it: {}\n- Fix: {}\n",
            d.detail, d.severity, d.class, d.cause, d.fix
        ));
        if d.action.kind != "none" && !d.action.label.is_empty() {
            out.push_str(&format!("- Offered action: {}.\n", d.action.label));
        }
    }
    out.push_str("\n## Refusals\n");
    for (anchor, title, cause, fix) in REFUSALS {
        out.push_str(&format!(
            "\n### {anchor}\n\n**{title}**\n\n- Cause: {cause}\n- Fix: {fix}\n"
        ));
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------------------------

/// What the user wants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Desired {
    #[default]
    Run,
    Paused,
}

/// friglet process lifecycle as the supervisor sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Process {
    /// Not started (unconfigured, or before the first start).
    #[default]
    Idle,
    /// Pre-start checks (resolve, handshake) or spawned and the control socket does not
    /// answer yet.
    Starting,
    /// Running and answering `GetStatus`.
    Running,
    /// Exited; waiting for the next automatic start.
    BackingOff,
    /// Exited or not started because of a permanent failure; waits for the user.
    Halted,
}

/// Classified failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Failure {
    OracleUnreachable,
    OracleDropped,
    PeerUnreachable,
    NodePruned,
    NodeSyncing,
    PortInUse,
    IdentityMismatch,
    ScanStopped,
    Unknown,
}

impl Failure {
    pub fn is_permanent(self) -> bool {
        matches!(
            self,
            Failure::NodePruned | Failure::PortInUse | Failure::IdentityMismatch
        )
    }

    pub fn state(self) -> StateId {
        match self {
            Failure::OracleUnreachable => StateId::OracleUnreachable,
            Failure::OracleDropped => StateId::OracleDroppedResuming,
            Failure::PeerUnreachable => StateId::PeerUnreachable,
            Failure::NodePruned => StateId::NodePruned,
            Failure::NodeSyncing => StateId::NodeSyncing,
            Failure::PortInUse => StateId::PortInUse,
            Failure::IdentityMismatch => StateId::IdentityMismatch,
            Failure::ScanStopped => StateId::ScanStopped,
            Failure::Unknown => StateId::UnknownExit,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Identity {
    #[default]
    Unknown,
    Pending,
    Verified,
    Mismatch,
}

/// The scan condition from the last `GetStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Scan {
    pub scanning: bool,
    /// Classified `last_error`, when the scan task ended with one.
    pub error: Option<Failure>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModelHeights {
    pub birthday: Option<u64>,
    pub scanned: Option<u64>,
    /// Highest scanned height seen for this wallet (falls only on a wallet change).
    pub max_scanned: Option<u64>,
    pub node_tip: Option<u64>,
    pub oracle_tip: Option<u64>,
}

/// How far the node may lag the oracle before `node-syncing` shows.
pub const NODE_LAG_BLOCKS: u64 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Inputs {
    /// FRIGLET_UI_NETWORK is missing or not supported: nothing else matters.
    pub unsupported_network: bool,
    pub configured: bool,
    /// A wallet change (setup, replace, restore) is in progress and not yet verified.
    pub transaction: bool,
    pub desired: Desired,
    pub process: Process,
    /// Last process-level failure (exit or failed pre-start check), cleared on a healthy run.
    pub failure: Option<Failure>,
    pub crash_loop: bool,
    pub identity: Identity,
    pub scan: Option<Scan>,
    pub heights: ModelHeights,
}

impl ModelHeights {
    /// min(node tip, oracle tip), whichever is known.
    pub fn target(&self) -> Option<u64> {
        match (self.node_tip, self.oracle_tip) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
}

/// Precedence, highest first:
/// 0. FRIGLET_UI_NETWORK missing or unsupported → `unsupported-network`
/// 1. no wallet → `needs-setup`
/// 2. identity mismatch → `identity-mismatch`
/// 3. permanent failure while halted → its state (`port-in-use`, `node-pruned`)
/// 4. process not running: crash loop → `crash-loop`; else the last failure's state, or
///    `starting` (also during a wallet change)
/// 5. running, identity not verified → `verifying-identity`
/// 6. user paused → `paused`
/// 7. scan trouble (`last_error`, or friglet's retry log lines while it keeps running) →
///    its classified state; scan task ended without one → `scan-stopped`
/// 8. node behind the oracle → `node-syncing`
/// 9. scanned height went backwards → `rescanning-after-reset`
/// 10. scanned ≥ target and ≥ birthday → `up-to-date`
/// 11. otherwise → `scanning`
pub fn derive(i: &Inputs) -> StateId {
    if i.unsupported_network {
        return StateId::UnsupportedNetwork;
    }
    if !i.configured && !i.transaction {
        return StateId::NeedsSetup;
    }
    if i.identity == Identity::Mismatch {
        return StateId::IdentityMismatch;
    }
    match i.process {
        Process::Halted => {
            return i
                .failure
                .map(Failure::state)
                .unwrap_or(StateId::UnknownExit);
        }
        Process::Idle | Process::BackingOff | Process::Starting => {
            if i.crash_loop && i.process == Process::BackingOff {
                return StateId::CrashLoop;
            }
            if i.process == Process::BackingOff
                && let Some(f) = i.failure
            {
                return f.state();
            }
            return StateId::Starting;
        }
        Process::Running => {}
    }
    if i.identity != Identity::Verified {
        return StateId::VerifyingIdentity;
    }
    if i.desired == Desired::Paused {
        return StateId::Paused;
    }
    let Some(scan) = i.scan else {
        return StateId::VerifyingIdentity;
    };
    if let Some(e) = scan.error {
        return e.state();
    }
    if !scan.scanning {
        return StateId::ScanStopped;
    }
    let h = &i.heights;
    if let (Some(node), Some(oracle)) = (h.node_tip, h.oracle_tip)
        && node + NODE_LAG_BLOCKS < oracle
    {
        return StateId::NodeSyncing;
    }
    if let (Some(scanned), Some(max)) = (h.scanned, h.max_scanned)
        && scanned < max
    {
        return StateId::RescanningAfterReset;
    }
    if let (Some(scanned), Some(target)) = (h.scanned, h.target())
        && scanned >= target
        && h.birthday.is_none_or(|b| scanned >= b)
    {
        return StateId::UpToDate;
    }
    StateId::Scanning
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_every_api_state_and_valid_fields() {
        let c = Catalog::load();
        // The ids docs/api.md lists, plus the browser-only banner.
        let api_ids = [
            "needs-setup",
            "starting",
            "verifying-identity",
            "identity-mismatch",
            "scanning",
            "up-to-date",
            "scan-stopped",
            "paused",
            "oracle-unreachable",
            "oracle-dropped-resuming",
            "peer-unreachable",
            "node-pruned",
            "node-syncing",
            "port-in-use",
            "rescanning-after-reset",
            "crash-loop",
            "unknown-exit",
            "unsupported-network",
            "ui-disconnected",
        ];
        let ids: Vec<&str> = c.ids().collect();
        for id in api_ids {
            assert!(ids.contains(&id), "catalog lacks {id}");
        }
        assert_eq!(
            ids.len(),
            api_ids.len(),
            "catalog has ids the API does not list"
        );
        for id in StateId::ALL {
            let def = c.get(id);
            assert_eq!(serde_json::to_value(id).unwrap(), id.as_str());
            if def.severity == "error" || def.severity == "warn" {
                assert_eq!(def.widget, "Needs attention", "{id:?}");
            }
            assert_eq!(def.health == "ok", id.is_healthy(), "{id:?}");
        }
        assert!(c.get(StateId::NodePruned).class == "permanent");
        assert!(c.get(StateId::PortInUse).class == "permanent");
        assert!(c.get(StateId::IdentityMismatch).class == "permanent");
        for id in [
            StateId::PortInUse,
            StateId::NodePruned,
            StateId::IdentityMismatch,
        ] {
            let f = match id {
                StateId::PortInUse => Failure::PortInUse,
                StateId::NodePruned => Failure::NodePruned,
                _ => Failure::IdentityMismatch,
            };
            assert!(f.is_permanent());
            assert_eq!(f.state(), id);
        }
    }

    /// docs/troubleshooting.md is generated from states.toml; this fails on drift.
    /// Regenerate with `FRIGLET_UI_REGEN_DOCS=1 cargo test -p friglet-ui troubleshooting`.
    #[test]
    fn troubleshooting_doc_is_current() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../docs/troubleshooting.md");
        let generated = troubleshooting_markdown(&Catalog::load());
        if std::env::var_os("FRIGLET_UI_REGEN_DOCS").is_some() {
            std::fs::write(&path, &generated).unwrap();
        }
        let on_disk = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            on_disk == generated,
            "docs/troubleshooting.md is out of date; run FRIGLET_UI_REGEN_DOCS=1 cargo test -p friglet-ui troubleshooting"
        );
    }

    /// Every `docs/troubleshooting.md#anchor` cited in friglet-ui's texts exists.
    #[test]
    fn troubleshooting_anchors_cited_in_code_exist() {
        let doc = troubleshooting_markdown(&Catalog::load());
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut cited = 0;
        for entry in std::fs::read_dir(src).unwrap() {
            let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
            for part in text.split("troubleshooting.md#").skip(1) {
                let anchor: String = part
                    .chars()
                    .take_while(|c| c.is_ascii_lowercase() || *c == '-')
                    .collect();
                if anchor.is_empty() || anchor == "anchor" {
                    continue;
                }
                cited += 1;
                assert!(
                    doc.contains(&format!("\n### {anchor}\n")),
                    "missing anchor #{anchor}"
                );
            }
        }
        assert!(cited >= 4);
    }

    #[test]
    fn catalog_rejects_missing_or_bad_entries() {
        assert!(Catalog::parse("").is_err());
        let bad = CATALOG_TOML.replace("severity = \"ok\"", "severity = \"green\"");
        assert!(Catalog::parse(&bad).is_err());
    }

    fn running_verified() -> Inputs {
        Inputs {
            configured: true,
            process: Process::Running,
            identity: Identity::Verified,
            scan: Some(Scan {
                scanning: true,
                error: None,
            }),
            heights: ModelHeights {
                birthday: Some(100),
                scanned: Some(150),
                max_scanned: Some(150),
                node_tip: Some(200),
                oracle_tip: Some(200),
            },
            ..Inputs::default()
        }
    }

    #[test]
    fn unconfigured_is_needs_setup_unless_a_transaction_runs() {
        assert_eq!(derive(&Inputs::default()), StateId::NeedsSetup);
        let i = Inputs {
            transaction: true,
            process: Process::Starting,
            ..Inputs::default()
        };
        assert_eq!(derive(&i), StateId::Starting);
    }

    #[test]
    fn happy_path_walks_starting_verifying_scanning_up_to_date() {
        let mut i = running_verified();
        i.process = Process::Starting;
        i.identity = Identity::Pending;
        assert_eq!(derive(&i), StateId::Starting);
        i.process = Process::Running;
        assert_eq!(derive(&i), StateId::VerifyingIdentity);
        i.identity = Identity::Verified;
        assert_eq!(derive(&i), StateId::Scanning);
        i.heights.scanned = Some(200);
        i.heights.max_scanned = Some(200);
        assert_eq!(derive(&i), StateId::UpToDate);
    }

    #[test]
    fn up_to_date_needs_birthday_coverage_and_uses_min_tip() {
        let mut i = running_verified();
        i.heights.node_tip = Some(199);
        i.heights.oracle_tip = Some(200);
        i.heights.scanned = Some(199);
        i.heights.max_scanned = Some(199);
        assert_eq!(
            derive(&i),
            StateId::UpToDate,
            "min(node, oracle) is the target"
        );
        // Birthday at the tip but the scan has not covered it yet.
        i.heights.birthday = Some(200);
        i.heights.scanned = Some(199);
        i.heights.node_tip = Some(200);
        assert_eq!(derive(&i), StateId::Scanning);
    }

    #[test]
    fn identity_mismatch_beats_everything_but_setup() {
        let mut i = running_verified();
        i.identity = Identity::Mismatch;
        i.process = Process::Halted;
        i.failure = Some(Failure::PortInUse);
        i.crash_loop = true;
        assert_eq!(derive(&i), StateId::IdentityMismatch);
        i.configured = false;
        assert_eq!(derive(&i), StateId::NeedsSetup);
    }

    #[test]
    fn permanent_failures_show_while_halted() {
        let mut i = running_verified();
        i.process = Process::Halted;
        i.failure = Some(Failure::NodePruned);
        assert_eq!(derive(&i), StateId::NodePruned);
        i.failure = Some(Failure::PortInUse);
        assert_eq!(derive(&i), StateId::PortInUse);
    }

    #[test]
    fn backoff_shows_the_failure_and_crash_loop_wins() {
        let mut i = running_verified();
        i.process = Process::BackingOff;
        i.failure = Some(Failure::OracleUnreachable);
        assert_eq!(derive(&i), StateId::OracleUnreachable);
        i.failure = Some(Failure::OracleDropped);
        assert_eq!(derive(&i), StateId::OracleDroppedResuming);
        i.failure = Some(Failure::Unknown);
        assert_eq!(derive(&i), StateId::UnknownExit);
        i.crash_loop = true;
        assert_eq!(derive(&i), StateId::CrashLoop);
        // Crash loop is not shown while friglet runs again.
        i.process = Process::Running;
        assert_eq!(derive(&i), StateId::Scanning);
    }

    #[test]
    fn paused_beats_scan_conditions_but_not_identity() {
        let mut i = running_verified();
        i.desired = Desired::Paused;
        i.scan = Some(Scan {
            scanning: false,
            error: None,
        });
        assert_eq!(derive(&i), StateId::Paused);
        i.identity = Identity::Pending;
        assert_eq!(derive(&i), StateId::VerifyingIdentity);
    }

    #[test]
    fn scan_stopped_and_classified_scan_errors() {
        let mut i = running_verified();
        i.scan = Some(Scan {
            scanning: false,
            error: None,
        });
        assert_eq!(derive(&i), StateId::ScanStopped);
        i.scan = Some(Scan {
            scanning: false,
            error: Some(Failure::PeerUnreachable),
        });
        assert_eq!(derive(&i), StateId::PeerUnreachable);
        i.scan = Some(Scan {
            scanning: false,
            error: Some(Failure::NodeSyncing),
        });
        assert_eq!(derive(&i), StateId::NodeSyncing);
    }

    #[test]
    fn node_syncing_and_rescanning() {
        let mut i = running_verified();
        i.heights.node_tip = Some(150);
        i.heights.oracle_tip = Some(200);
        assert_eq!(derive(&i), StateId::NodeSyncing);
        i.heights.node_tip = Some(199);
        i.heights.scanned = Some(120);
        i.heights.max_scanned = Some(180);
        assert_eq!(derive(&i), StateId::RescanningAfterReset);
        i.heights.scanned = Some(180);
        assert_eq!(derive(&i), StateId::Scanning);
    }

    #[test]
    fn unsupported_network_beats_everything() {
        let mut i = running_verified();
        i.unsupported_network = true;
        assert_eq!(derive(&i), StateId::UnsupportedNetwork);
        i.configured = false;
        assert_eq!(derive(&i), StateId::UnsupportedNetwork);
    }

    #[test]
    fn scan_trouble_while_scanning_shows() {
        let mut i = running_verified();
        i.scan = Some(Scan {
            scanning: true,
            error: Some(Failure::OracleDropped),
        });
        assert_eq!(derive(&i), StateId::OracleDroppedResuming);
    }

    #[test]
    fn starting_during_a_wallet_change() {
        let i = Inputs {
            configured: true,
            transaction: true,
            process: Process::Idle,
            ..Inputs::default()
        };
        assert_eq!(derive(&i), StateId::Starting);
    }
}
