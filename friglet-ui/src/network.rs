//! Bitcoin networks friglet-ui supports, with the per-network constants the rest of the
//! crate needs (address prefixes, taproot activation, oracle defaults, P2P magic and port).

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    Bitcoin,
    Signet,
    Testnet4,
    Regtest,
}

/// Which descriptor key prefix a network accepts (BIP-392: `spscan` for mainnet, `tspscan`
/// for every test network).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyFamily {
    Mainnet,
    Test,
}

impl KeyFamily {
    pub fn describe(self) -> &'static str {
        match self {
            KeyFamily::Mainnet => "mainnet",
            KeyFamily::Test => "a test network (signet, testnet4 or regtest)",
        }
    }
}

impl Network {
    pub const ALL: [Network; 4] = [
        Network::Bitcoin,
        Network::Signet,
        Network::Testnet4,
        Network::Regtest,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Network::Bitcoin => "bitcoin",
            Network::Signet => "signet",
            Network::Testnet4 => "testnet4",
            Network::Regtest => "regtest",
        }
    }

    pub fn family(self) -> KeyFamily {
        match self {
            Network::Bitcoin => KeyFamily::Mainnet,
            _ => KeyFamily::Test,
        }
    }

    /// Human-readable part friglet (bdk_sp) uses for the wallet's `sp_address`.
    ///
    /// BIP-352 names `sp` for mainnet and `tsp` for testnets; bdk_sp at the friglet pin uses
    /// `sprt` for regtest. friglet-ui follows friglet so the identity check compares like
    /// with like.
    pub fn sp_hrp(self) -> &'static str {
        match self {
            Network::Bitcoin => "sp",
            Network::Signet | Network::Testnet4 => "tsp",
            Network::Regtest => "sprt",
        }
    }

    /// Lowest birthday friglet-ui accepts: taproot activation (Silent Payments outputs are
    /// taproot outputs). Signet and testnet4 have taproot from genesis; regtest allows 0.
    pub fn min_birthday(self) -> u64 {
        match self {
            Network::Bitcoin => 709_632,
            Network::Signet | Network::Testnet4 => 1,
            Network::Regtest => 0,
        }
    }

    /// Default BlindBit oracle per network (contract: FRIGLET_UI_ORACLE_URL).
    pub fn default_oracle_url(self) -> Option<&'static str> {
        match self {
            Network::Bitcoin => Some("https://oracle.setor.dev"),
            Network::Signet => Some("https://signet.oracle.setor.dev"),
            Network::Testnet4 | Network::Regtest => None,
        }
    }

    pub fn default_p2p_port(self) -> u16 {
        match self {
            Network::Bitcoin => 8333,
            Network::Signet => 38333,
            Network::Testnet4 => 48333,
            Network::Regtest => 18444,
        }
    }

    pub fn bitcoin(self) -> bitcoin::Network {
        match self {
            Network::Bitcoin => bitcoin::Network::Bitcoin,
            Network::Signet => bitcoin::Network::Signet,
            Network::Testnet4 => bitcoin::Network::Testnet4,
            Network::Regtest => bitcoin::Network::Regtest,
        }
    }

    /// P2P message start bytes.
    pub fn magic(self) -> bitcoin::p2p::Magic {
        self.bitcoin().magic()
    }
}

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Network {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "bitcoin" | "mainnet" => Ok(Network::Bitcoin),
            "signet" => Ok(Network::Signet),
            "testnet4" => Ok(Network::Testnet4),
            "regtest" => Ok(Network::Regtest),
            other => Err(format!(
                "unknown network `{other}`: use bitcoin, signet, testnet4 or regtest"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_contract_names() {
        for n in Network::ALL {
            assert_eq!(n.as_str().parse::<Network>().unwrap(), n);
        }
        assert!("testnet".parse::<Network>().is_err());
    }

    #[test]
    fn taproot_activation_table() {
        assert_eq!(Network::Bitcoin.min_birthday(), 709_632);
        assert_eq!(Network::Signet.min_birthday(), 1);
        assert_eq!(Network::Regtest.min_birthday(), 0);
    }

    #[test]
    fn magic_matches_known_values() {
        assert_eq!(
            Network::Bitcoin.magic().to_bytes(),
            [0xf9, 0xbe, 0xb4, 0xd9]
        );
        assert_eq!(Network::Signet.magic().to_bytes(), [0x0a, 0x03, 0xcf, 0x40]);
        assert_eq!(
            Network::Regtest.magic().to_bytes(),
            [0xfa, 0xbf, 0xb5, 0xda]
        );
    }

    #[test]
    fn oracle_defaults() {
        assert_eq!(
            Network::Signet.default_oracle_url(),
            Some("https://signet.oracle.setor.dev")
        );
        assert_eq!(
            Network::Bitcoin.default_oracle_url(),
            Some("https://oracle.setor.dev")
        );
        assert_eq!(Network::Regtest.default_oracle_url(), None);
    }
}
