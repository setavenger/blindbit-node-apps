//! friglet-ui: setup, supervision, status and Electrum front for the friglet Silent Payments
//! daemon on node-in-a-box platforms. See docs/contract.md and docs/api.md.

pub mod auth;
pub mod classify;
pub mod cli;
pub mod config;
pub mod control;
pub mod descriptor;
pub mod electrum;
pub mod files;
pub mod ipc;
pub mod network;
pub mod p2p;
pub mod redact;
pub mod setup;
pub mod states;
pub mod supervisor;
pub mod tls;
pub mod util;
pub mod view;
pub mod wallet;
pub mod web;

/// friglet's HTTP API, loopback only (never published).
pub const FRIGLET_HTTP_ADDR: &str = "127.0.0.1:8095";
/// friglet's Electrum server, loopback only; friglet-ui's listeners proxy to it.
pub const FRIGLET_ELECTRUM_ADDR: &str = "127.0.0.1:50001";

/// friglet-ui version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
/// Full blindbit-rs commit friglet is pinned to (versions.env).
pub const FRIGLET_REV: &str = env!("FRIGLET_UI_FRIGLET_REV");
/// Built with BLINDBIT_RS_SRC (local or unmerged blindbit-rs).
pub const DEV_BUILD: bool = matches!(env!("FRIGLET_UI_DEV_BUILD").as_bytes(), b"1");

/// Short form of [`FRIGLET_REV`].
pub fn friglet_short_rev() -> &'static str {
    &FRIGLET_REV[..FRIGLET_REV.len().min(7)]
}

pub mod assets {
    include!(concat!(env!("OUT_DIR"), "/assets.rs"));

    pub fn get(path: &str) -> Option<&'static [u8]> {
        ASSETS.iter().find(|(p, _)| *p == path).map(|(_, b)| *b)
    }
}

#[cfg(test)]
mod tests {
    /// The friglet-ipc git `rev` in Cargo.toml must be the friglet pin in versions.env.
    #[test]
    fn friglet_ipc_rev_matches_versions_env() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let manifest = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
        let versions = std::fs::read_to_string(root.join("../versions.env")).unwrap();
        let pin = versions
            .lines()
            .find_map(|l| l.strip_prefix("BLINDBIT_RS_REV="))
            .expect("BLINDBIT_RS_REV in versions.env")
            .trim();
        let dep = manifest
            .lines()
            .find(|l| l.starts_with("friglet-ipc"))
            .expect("friglet-ipc dependency");
        assert!(
            dep.contains(&format!("rev = \"{pin}\"")),
            "friglet-ipc rev differs from versions.env ({pin}): {dep}"
        );
        assert_eq!(super::FRIGLET_REV, pin);
        let lock = std::fs::read_to_string(root.join("../Cargo.lock")).unwrap();
        assert!(lock.contains(&format!("blindbit-rs.git?rev={pin}#{pin}")));
    }

    #[test]
    fn assets_have_an_index() {
        assert!(super::assets::get("index.html").is_some_and(|b| !b.is_empty()));
    }
}
