//! JSON shapes of `GET /api/state` and `GET /api/widget` (docs/api.md, docs/contract.md).

use serde::{Deserialize, Serialize};

use crate::states::{ActionDef, StateId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateResponse {
    pub configured: bool,
    /// The box's configured network (FRIGLET_UI_NETWORK), present before setup.
    pub network: String,
    pub state: StateView,
    pub wallet: Option<WalletView>,
    pub heights: HeightsView,
    pub progress: ProgressView,
    pub payments_found: Option<u64>,
    pub node: NodeView,
    pub oracle: OracleView,
    pub versions: VersionsView,
    pub restarts: u64,
    pub paused: bool,
    pub transaction: Option<TransactionView>,
    pub previous: Option<PreviousView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviousView {
    pub exists: bool,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateView {
    pub id: StateId,
    pub severity: String,
    pub headline: String,
    pub detail: String,
    pub action: ActionDef,
    pub next_retry_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletView {
    pub sp_address: String,
    pub key_fingerprint: String,
    pub network: String,
    pub birthday_height: u64,
    pub identity_verified: bool,
    /// What friglet reports; differs from `sp_address` only in identity-mismatch.
    pub reported_sp_address: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HeightsView {
    pub birthday: Option<u64>,
    pub birthday_time: Option<String>,
    pub scanned: Option<u64>,
    pub persisted: Option<u64>,
    pub node_tip: Option<u64>,
    pub oracle_tip: Option<u64>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProgressView {
    pub fraction: Option<f64>,
    pub eta_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeView {
    pub peer: Option<String>,
    pub configured: String,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OracleView {
    pub url: String,
    pub third_party: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionsView {
    pub friglet: String,
    pub friglet_ui: String,
    pub dev_build: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionView {
    /// replace | forget | restore
    pub kind: String,
    /// intent | stopping | snapshot | writing | starting | verifying | persisting | done
    pub step: String,
    pub failed: bool,
    pub error: Option<String>,
    pub started_at: String,
}

/// Umbrel `text-with-progress` widget plus our three fields. No wallet data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Widget {
    #[serde(rename = "type")]
    pub kind: String,
    pub refresh: String,
    pub link: String,
    pub title: String,
    pub text: String,
    pub subtext: String,
    #[serde(rename = "progressLabel")]
    pub progress_label: String,
    pub progress: f64,
    pub state: StateId,
    pub scanned_height: Option<u64>,
    pub tip_height: Option<u64>,
}

/// `1234567` → `1,234,567`.
pub fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

impl Widget {
    pub fn from_state(s: &StateResponse, widget_text: &str) -> Self {
        let id = s.state.id;
        let tip = match (s.heights.node_tip, s.heights.oracle_tip) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        };
        let attention = matches!(s.state.severity.as_str(), "warn" | "error");
        let scanning = matches!(id, StateId::Scanning | StateId::RescanningAfterReset);
        let (text, progress) = if attention {
            ("Needs attention".to_string(), 0.0)
        } else if id == StateId::UpToDate {
            ("100%".to_string(), 1.0)
        } else if scanning {
            let f = s.progress.fraction.unwrap_or(0.0).clamp(0.0, 1.0);
            (format!("{}%", (f * 100.0).floor() as u64), f)
        } else {
            (widget_text.to_string(), 0.0)
        };
        let subtext = match (s.heights.scanned, tip) {
            (Some(a), Some(b)) if s.configured && !attention => {
                format!("Block {} of {}", thousands(a.min(b)), thousands(b))
            }
            _ => String::new(),
        };
        Widget {
            kind: "text-with-progress".into(),
            refresh: "30s".into(),
            link: String::new(),
            title: "Silent Payments scan".into(),
            text,
            subtext,
            progress_label: widget_text.to_string(),
            progress,
            state: id,
            scanned_height: s.heights.scanned,
            tip_height: tip,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_separator() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(850_000), "850,000");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }
}
