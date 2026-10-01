//! friglet-ui configuration: `FRIGLET_UI_*` environment variables or the matching flags
//! (docs/contract.md), and the refusal of every other `FRIGLET_*` variable.

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;

use clap::Args;
use serde::Serialize;

use crate::network::Network;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Umbrel,
    Startos,
    Raspiblitz,
    Mynode,
    Compose,
}

impl Platform {
    pub fn as_str(self) -> &'static str {
        match self {
            Platform::Umbrel => "umbrel",
            Platform::Startos => "startos",
            Platform::Raspiblitz => "raspiblitz",
            Platform::Mynode => "mynode",
            Platform::Compose => "compose",
        }
    }

    /// Native (non-container) installs default to loopback.
    pub fn is_native(self) -> bool {
        matches!(self, Platform::Raspiblitz)
    }
}

impl FromStr for Platform {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "umbrel" => Ok(Platform::Umbrel),
            "startos" => Ok(Platform::Startos),
            "raspiblitz" => Ok(Platform::Raspiblitz),
            "mynode" => Ok(Platform::Mynode),
            "compose" => Ok(Platform::Compose),
            other => Err(format!(
                "unknown platform `{other}`: use umbrel, startos, raspiblitz, mynode or compose"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthMode {
    Password,
    Gateway,
    None,
}

impl AuthMode {
    pub fn as_str(self) -> &'static str {
        match self {
            AuthMode::Password => "password",
            AuthMode::Gateway => "gateway",
            AuthMode::None => "none",
        }
    }
}

impl FromStr for AuthMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "password" => Ok(AuthMode::Password),
            "gateway" => Ok(AuthMode::Gateway),
            "none" => Ok(AuthMode::None),
            other => Err(format!(
                "unknown auth mode `{other}`: use password, gateway or none"
            )),
        }
    }
}

/// Flags for the daemon; each falls back to its `FRIGLET_UI_*` variable.
#[derive(Debug, Clone, Default, Args)]
pub struct RunArgs {
    #[arg(long, env = "FRIGLET_UI_PLATFORM")]
    pub platform: Option<String>,
    #[arg(long, env = "FRIGLET_UI_LISTEN")]
    pub listen: Option<String>,
    #[arg(long, env = "FRIGLET_UI_NETWORK")]
    pub network: Option<String>,
    #[arg(long, env = "FRIGLET_UI_NODE")]
    pub node: Option<String>,
    #[arg(long, env = "FRIGLET_UI_ORACLE_URL")]
    pub oracle_url: Option<String>,
    #[arg(long, env = "FRIGLET_UI_AUTH")]
    pub auth: Option<String>,
    #[arg(long, env = "FRIGLET_UI_PASSWORD_FILE")]
    pub password_file: Option<PathBuf>,
    #[arg(long, env = "FRIGLET_UI_ELECTRUM_TCP")]
    pub electrum_tcp: Option<String>,
    #[arg(long, env = "FRIGLET_UI_ELECTRUM_TLS")]
    pub electrum_tls: Option<String>,
    #[arg(long, env = "FRIGLET_UI_PUBLIC_HOST")]
    pub public_host: Option<String>,
    #[arg(long, env = "FRIGLET_UI_ONION_HOST")]
    pub onion_host: Option<String>,
    #[arg(long, env = "FRIGLET_UI_ALLOWED_HOSTS", value_delimiter = ',')]
    pub allowed_hosts: Vec<String>,
    #[arg(long, env = "FRIGLET_UI_TRUSTED_PROXIES", value_delimiter = ',')]
    pub trusted_proxies: Vec<String>,
    #[arg(long, env = "FRIGLET_UI_FRIGLET_BIN")]
    pub friglet_bin: Option<PathBuf>,
}

/// An IP network (`10.0.0.0/8`, `::1`, `172.17.0.1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IpCidr {
    pub addr: IpAddr,
    pub prefix: u8,
}

impl FromStr for IpCidr {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        let s = s.trim();
        let (ip, prefix) = match s.split_once('/') {
            Some((ip, p)) => (ip, Some(p)),
            None => (s, None),
        };
        let addr: IpAddr = ip
            .parse()
            .map_err(|_| format!("invalid IP `{ip}` in FRIGLET_UI_TRUSTED_PROXIES"))?;
        let max = if addr.is_ipv4() { 32 } else { 128 };
        let prefix =
            match prefix {
                Some(p) => p.parse::<u8>().ok().filter(|p| *p <= max).ok_or_else(|| {
                    format!("invalid prefix `/{p}` in FRIGLET_UI_TRUSTED_PROXIES")
                })?,
                None => max,
            };
        Ok(IpCidr { addr, prefix })
    }
}

impl IpCidr {
    pub fn contains(&self, ip: IpAddr) -> bool {
        match (self.addr, ip.to_canonical()) {
            (IpAddr::V4(net), IpAddr::V4(ip)) => {
                let mask = if self.prefix == 0 {
                    0
                } else {
                    u32::MAX << (32 - u32::from(self.prefix))
                };
                u32::from(net) & mask == u32::from(ip) & mask
            }
            (IpAddr::V6(net), IpAddr::V6(ip)) => {
                let mask = if self.prefix == 0 {
                    0
                } else {
                    u128::MAX << (128 - u32::from(self.prefix))
                };
                u128::from(net) & mask == u128::from(ip) & mask
            }
            _ => false,
        }
    }
}

/// Validated daemon configuration.
#[derive(Debug, Clone)]
pub struct UiConfig {
    pub platform: Platform,
    pub data_dir: PathBuf,
    pub listen: SocketAddr,
    /// The configured network. When FRIGLET_UI_NETWORK is missing or unsupported this is a
    /// placeholder and `unsupported_network` says why: friglet-ui then serves the UI in the
    /// `unsupported-network` state and never starts friglet (no restart loop).
    pub network: Network,
    pub unsupported_network: Option<String>,
    /// FRIGLET_UI_NETWORK as given.
    pub network_raw: Option<String>,
    pub node: String,
    pub oracle_url: String,
    pub auth: AuthMode,
    /// Plaintext initial password (FRIGLET_UI_PASSWORD, env only: never a flag).
    pub password: Option<String>,
    pub password_file: Option<PathBuf>,
    pub electrum_tcp: Option<SocketAddr>,
    pub electrum_tls: Option<SocketAddr>,
    pub public_host: Option<String>,
    pub onion_host: Option<String>,
    pub allowed_hosts: Vec<String>,
    /// Peers whose `X-Forwarded-For` is trusted for the login rate limit.
    pub trusted_proxies: Vec<IpCidr>,
    pub friglet_bin: PathBuf,
}

pub const PASSWORD_ENV: &str = "FRIGLET_UI_PASSWORD";

/// Exact refusal text for a stray `FRIGLET_*` variable.
pub fn stray_variable_message(name: &str) -> String {
    format!(
        "refusing to start: environment variable {name} is set. friglet-ui configures friglet \
         itself and starts it with a cleared environment, so FRIGLET_* variables other than \
         FRIGLET_UI_* are never passed on and would silently do nothing. Fix: remove {name} \
         (use the matching FRIGLET_UI_* setting instead). See docs/troubleshooting.md#stray-friglet-variable"
    )
}

/// Refuse any `FRIGLET_*` variable that is not `FRIGLET_UI_*`, naming the first one found
/// (sorted, so the message is stable).
pub fn check_environment<I, K>(vars: I) -> Result<(), String>
where
    I: IntoIterator<Item = K>,
    K: AsRef<std::ffi::OsStr>,
{
    let mut stray: Vec<String> = vars
        .into_iter()
        .filter_map(|k| k.as_ref().to_str().map(str::to_string))
        .filter(|k| k.starts_with("FRIGLET_") && !k.starts_with("FRIGLET_UI_"))
        .collect();
    stray.sort();
    match stray.first() {
        Some(name) => Err(stray_variable_message(name)),
        None => Ok(()),
    }
}

/// `host:port`, or an `off` keyword.
pub fn parse_listener(value: Option<&str>, default: &str) -> Result<Option<SocketAddr>, String> {
    let v = value.unwrap_or(default).trim();
    if v.eq_ignore_ascii_case("off") || v.is_empty() {
        return Ok(None);
    }
    SocketAddr::from_str(v)
        .map(Some)
        .map_err(|e| format!("invalid listen address `{v}`: {e} (use ip:port or off)"))
}

impl UiConfig {
    pub fn resolve(
        args: &RunArgs,
        data_dir: PathBuf,
        password_env: Option<String>,
    ) -> Result<Self, String> {
        let platform = match &args.platform {
            Some(p) => p.parse()?,
            None => Platform::Compose,
        };
        let (network, unsupported_network) = match args.network.as_deref().map(str::trim) {
            None | Some("") => (
                Network::Bitcoin,
                Some(
                    "FRIGLET_UI_NETWORK is not set (use bitcoin, signet, testnet4 or regtest)"
                        .to_string(),
                ),
            ),
            Some(n) => match n.parse::<Network>() {
                Ok(net) => (net, None),
                Err(_) => (
                    Network::Bitcoin,
                    Some(format!(
                        "this node runs `{n}`, which friglet does not support (bitcoin, signet, testnet4 or regtest)"
                    )),
                ),
            },
        };
        let node = args
            .node
            .clone()
            .filter(|n| !n.trim().is_empty())
            .ok_or("FRIGLET_UI_NODE is required: the Bitcoin P2P peer as host:port")?;
        validate_node(&node)?;
        let oracle_url = match &args.oracle_url {
            Some(u) if !u.trim().is_empty() => u.trim().to_string(),
            _ if unsupported_network.is_some() => String::new(),
            _ => network
                .default_oracle_url()
                .ok_or_else(|| {
                    format!(
                        "FRIGLET_UI_ORACLE_URL is required on {network} (no public default oracle)"
                    )
                })?
                .to_string(),
        };
        if unsupported_network.is_none()
            && !(oracle_url.starts_with("https://") || oracle_url.starts_with("http://"))
        {
            return Err(format!(
                "FRIGLET_UI_ORACLE_URL `{oracle_url}` must start with https:// (or http:// for a local oracle)"
            ));
        }
        let auth = match &args.auth {
            Some(a) => a.parse()?,
            None if platform == Platform::Umbrel => AuthMode::Gateway,
            None => AuthMode::Password,
        };
        if auth == AuthMode::None && platform != Platform::Compose {
            return Err(format!(
                "FRIGLET_UI_AUTH=none is only allowed with FRIGLET_UI_PLATFORM=compose (platform is {})",
                platform.as_str()
            ));
        }
        let default_listen = if platform.is_native() {
            "127.0.0.1:8097"
        } else {
            "0.0.0.0:8097"
        };
        let listen = args.listen.as_deref().unwrap_or(default_listen);
        let listen = SocketAddr::from_str(listen)
            .map_err(|e| format!("invalid FRIGLET_UI_LISTEN `{listen}`: {e}"))?;
        let electrum_tcp = parse_listener(args.electrum_tcp.as_deref(), "0.0.0.0:50031")?;
        let electrum_tls = parse_listener(args.electrum_tls.as_deref(), "0.0.0.0:50032")?;
        let friglet_bin = args.friglet_bin.clone().unwrap_or_else(default_friglet_bin);
        let proxies: Vec<&str> = args
            .trusted_proxies
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        let trusted_proxies = if proxies.is_empty() {
            vec!["127.0.0.1".parse()?, "::1".parse()?]
        } else {
            proxies
                .into_iter()
                .map(str::parse)
                .collect::<Result<_, _>>()?
        };
        Ok(UiConfig {
            platform,
            data_dir,
            listen,
            network,
            unsupported_network,
            network_raw: args
                .network
                .as_deref()
                .map(|n| n.trim().to_string())
                .filter(|n| !n.is_empty()),
            node,
            oracle_url,
            auth,
            password: password_env.filter(|p| !p.is_empty()),
            password_file: args.password_file.clone(),
            electrum_tcp,
            electrum_tls,
            public_host: args.public_host.clone().filter(|h| !h.is_empty()),
            onion_host: args.onion_host.clone().filter(|h| !h.is_empty()),
            allowed_hosts: args
                .allowed_hosts
                .iter()
                .map(|h| h.trim().to_ascii_lowercase())
                .filter(|h| !h.is_empty())
                .collect(),
            trusted_proxies,
            friglet_bin,
        })
    }

    /// Network name for the API: the configured value, also when unsupported.
    pub fn network_label(&self) -> String {
        match &self.network_raw {
            Some(raw) if self.unsupported_network.is_some() => raw.clone(),
            _ => self.network.as_str().to_string(),
        }
    }

    pub fn oracle_is_default(&self) -> bool {
        self.network.default_oracle_url() == Some(self.oracle_url.as_str())
    }
}

/// `host:port` with a non-empty host and a numeric port.
pub fn validate_node(node: &str) -> Result<(), String> {
    let Some((host, port)) = node.rsplit_once(':') else {
        return Err(format!("FRIGLET_UI_NODE `{node}` must be host:port"));
    };
    if host.trim_matches(['[', ']']).is_empty() {
        return Err(format!("FRIGLET_UI_NODE `{node}` has no host"));
    }
    port.parse::<u16>()
        .map_err(|_| format!("FRIGLET_UI_NODE `{node}` has an invalid port"))?;
    Ok(())
}

/// `/usr/local/bin/friglet` in the image; next to `friglet-ui` on native installs.
pub fn default_friglet_bin() -> PathBuf {
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let sibling = dir.join("friglet");
        if sibling.is_file() {
            return sibling;
        }
    }
    PathBuf::from("/usr/local/bin/friglet")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args() -> RunArgs {
        RunArgs {
            network: Some("signet".into()),
            node: Some("bitcoind:38333".into()),
            ..RunArgs::default()
        }
    }

    #[test]
    fn stray_friglet_variables_are_refused_by_name() {
        assert!(check_environment(["PATH", "FRIGLET_UI_NETWORK", "HOME"]).is_ok());
        let err = check_environment([
            "FRIGLET_UI_NODE",
            "FRIGLET_SCAN_SECRET",
            "FRIGLET_ORACLE_URL",
        ])
        .unwrap_err();
        assert!(err.contains("FRIGLET_ORACLE_URL"), "{err}");
        assert!(err.contains("docs/troubleshooting.md#stray-friglet-variable"));
        let err = check_environment(["FRIGLET_CONTROL_SOCKET"]).unwrap_err();
        assert!(err.contains("FRIGLET_CONTROL_SOCKET"));
        // A prefix lookalike is not ours to judge.
        assert!(check_environment(["FRIGLETX", "MY_FRIGLET_Y"]).is_ok());
    }

    #[test]
    fn defaults_follow_the_contract() {
        let c = UiConfig::resolve(&args(), "/data".into(), None).unwrap();
        assert_eq!(c.platform, Platform::Compose);
        assert_eq!(c.listen.to_string(), "0.0.0.0:8097");
        assert_eq!(c.oracle_url, "https://signet.oracle.setor.dev");
        assert_eq!(c.auth, AuthMode::Password);
        assert_eq!(c.electrum_tcp.unwrap().to_string(), "0.0.0.0:50031");
        assert_eq!(c.electrum_tls.unwrap().to_string(), "0.0.0.0:50032");

        let mut a = args();
        a.platform = Some("umbrel".into());
        assert_eq!(
            UiConfig::resolve(&a, "/data".into(), None).unwrap().auth,
            AuthMode::Gateway
        );
        a.platform = Some("raspiblitz".into());
        assert_eq!(
            UiConfig::resolve(&a, "/data".into(), None)
                .unwrap()
                .listen
                .to_string(),
            "127.0.0.1:8097"
        );
    }

    #[test]
    fn off_disables_electrum_listeners_and_none_is_compose_only() {
        let mut a = args();
        a.electrum_tls = Some("off".into());
        a.electrum_tcp = Some("OFF".into());
        let c = UiConfig::resolve(&a, "/data".into(), None).unwrap();
        assert!(c.electrum_tls.is_none() && c.electrum_tcp.is_none());

        a.auth = Some("none".into());
        assert!(UiConfig::resolve(&a, "/data".into(), None).is_ok());
        a.platform = Some("startos".into());
        assert!(UiConfig::resolve(&a, "/data".into(), None).is_err());
    }

    #[test]
    fn trusted_proxies_default_and_cidrs() {
        let c = UiConfig::resolve(&args(), "/data".into(), None).unwrap();
        let ip = |s: &str| s.parse::<IpAddr>().unwrap();
        assert!(
            c.trusted_proxies
                .iter()
                .any(|n| n.contains(ip("127.0.0.1")))
        );
        assert!(c.trusted_proxies.iter().any(|n| n.contains(ip("::1"))));
        assert!(
            c.trusted_proxies
                .iter()
                .any(|n| n.contains(ip("::ffff:127.0.0.1")))
        );
        assert!(
            !c.trusted_proxies
                .iter()
                .any(|n| n.contains(ip("172.17.0.1")))
        );
        let mut a = args();
        a.trusted_proxies = vec!["172.17.0.1".into(), " 10.0.0.0/8".into(), "fd00::/8".into()];
        let c = UiConfig::resolve(&a, "/data".into(), None).unwrap();
        assert!(
            c.trusted_proxies
                .iter()
                .any(|n| n.contains(ip("172.17.0.1")))
        );
        assert!(c.trusted_proxies.iter().any(|n| n.contains(ip("10.2.3.4"))));
        assert!(c.trusted_proxies.iter().any(|n| n.contains(ip("fd12::1"))));
        assert!(
            !c.trusted_proxies
                .iter()
                .any(|n| n.contains(ip("127.0.0.1")))
        );
        a.trusted_proxies = vec!["10.0.0.0/40".into()];
        assert!(UiConfig::resolve(&a, "/data".into(), None).is_err());
    }

    #[test]
    fn required_and_invalid_values() {
        let mut a = args();
        a.network = None;
        let c = UiConfig::resolve(&a, "/data".into(), None).unwrap();
        assert!(
            c.unsupported_network
                .unwrap()
                .contains("FRIGLET_UI_NETWORK")
        );
        let mut a = args();
        a.network = Some("testnet".into());
        let c = UiConfig::resolve(&a, "/data".into(), None).unwrap();
        assert!(
            c.unsupported_network.unwrap().contains("`testnet`"),
            "Umbrel's testnet3 is a state, not an error"
        );
        let mut a = args();
        a.node = Some("bitcoind".into());
        assert!(UiConfig::resolve(&a, "/data".into(), None).is_err());
        let mut a = args();
        a.network = Some("regtest".into());
        assert!(
            UiConfig::resolve(&a, "/data".into(), None)
                .unwrap_err()
                .contains("ORACLE_URL")
        );
        a.oracle_url = Some("ftp://x".into());
        assert!(UiConfig::resolve(&a, "/data".into(), None).is_err());
    }
}
