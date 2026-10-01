//! Setup requests: one validation path shared by the web form, the online CLI handover and
//! the offline (staged) CLI, with the contract's exit codes.

use serde::{Deserialize, Serialize};

use crate::descriptor::{self, DescriptorError, WalletIdentity, WalletKeys};
use crate::network::Network;
use crate::wallet;

pub const EXIT_OK: i32 = 0;
pub const EXIT_USAGE: i32 = 2;
pub const EXIT_DESCRIPTOR: i32 = 3;
pub const EXIT_BIRTHDAY: i32 = 4;
pub const EXIT_NETWORK: i32 = 5;
pub const EXIT_NODE: i32 = 6;
pub const EXIT_EXISTS: i32 = 7;
pub const EXIT_IO: i32 = 8;

/// `--json` result: `{"ok","code","state","sp_address","message"}` plus the field the error
/// belongs to (for the web form).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Outcome {
    pub ok: bool,
    pub code: i32,
    pub state: Option<String>,
    pub sp_address: Option<String>,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

impl Outcome {
    pub fn ok(
        state: Option<&str>,
        identity: Option<&WalletIdentity>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            ok: true,
            code: EXIT_OK,
            state: state.map(str::to_string),
            sp_address: identity.map(|i| i.sp_address.clone()),
            message: message.into(),
            key_fingerprint: identity.map(|i| i.key_fingerprint.clone()),
            network: identity.map(|i| i.network.as_str().to_string()),
            field: None,
            error_code: None,
        }
    }

    pub fn err(
        code: i32,
        error_code: &str,
        field: Option<&str>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            ok: false,
            code,
            state: None,
            sp_address: None,
            message: message.into(),
            key_fingerprint: None,
            network: None,
            field: field.map(str::to_string),
            error_code: Some(error_code.to_string()),
        }
    }

    pub fn with_state(mut self, state: &str) -> Self {
        self.state = Some(state.to_string());
        self
    }
}

/// Parse and check the descriptor and birthday for `network`. `node_tip` bounds the birthday
/// when known.
pub fn validate(
    descriptor_text: &str,
    birthday: u64,
    network: Network,
    node_tip: Option<u64>,
) -> Result<(WalletKeys, WalletIdentity), Box<Outcome>> {
    let keys = descriptor::parse(descriptor_text).map_err(|e| Box::new(descriptor_outcome(&e)))?;
    keys.check_network(network)
        .map_err(|e| Box::new(descriptor_outcome(&e)))?;
    wallet::validate_birthday(network, birthday, node_tip).map_err(|m| {
        Box::new(Outcome::err(
            EXIT_BIRTHDAY,
            "birthday-invalid",
            Some("birthday_height"),
            m,
        ))
    })?;
    let identity = keys.identity(network);
    Ok((keys, identity))
}

pub fn descriptor_outcome(e: &DescriptorError) -> Outcome {
    let code = match e {
        DescriptorError::NetworkMismatch { .. } => EXIT_NETWORK,
        _ => EXIT_DESCRIPTOR,
    };
    Outcome::err(code, e.code(), Some("descriptor"), e.to_string())
}

/// Message for a wallet that exists without `replace`.
pub fn exists_message(current: &wallet::WalletMeta) -> String {
    format!(
        "a different wallet is already set up ({}, birthday {}). Replacing it rescans from the new birthday; \
         confirm the change (CLI: --replace). See docs/troubleshooting.md#wallet-exists",
        current.sp_address, current.birthday_height
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptor::tests::random_keys;
    use crate::network::KeyFamily;

    #[test]
    fn exit_codes_for_each_failure() {
        let (s, p) = random_keys(1);
        let test_desc = WalletKeys::new(s, p, None).to_descriptor(KeyFamily::Test);
        let main_desc = WalletKeys::new(s, p, None).to_descriptor(KeyFamily::Mainnet);
        assert!(validate(&test_desc, 100, Network::Signet, Some(200)).is_ok());
        assert_eq!(
            validate("sp(nonsense)", 100, Network::Signet, None)
                .unwrap_err()
                .code,
            EXIT_DESCRIPTOR
        );
        let e = validate(&main_desc, 100, Network::Signet, None).unwrap_err();
        assert_eq!(e.code, EXIT_NETWORK);
        assert_eq!(e.field.as_deref(), Some("descriptor"));
        let e = validate(&test_desc, 300, Network::Signet, Some(200)).unwrap_err();
        assert_eq!(e.code, EXIT_BIRTHDAY);
        assert_eq!(e.field.as_deref(), Some("birthday_height"));
        assert_eq!(
            validate(&main_desc, 1, Network::Bitcoin, None)
                .unwrap_err()
                .code,
            EXIT_BIRTHDAY
        );
    }

    #[test]
    fn outcome_json_shape() {
        let o = Outcome::err(EXIT_EXISTS, "wallet-exists", None, "x");
        let v = serde_json::to_value(&o).unwrap();
        for k in ["ok", "code", "state", "sp_address", "message"] {
            assert!(v.get(k).is_some(), "{k} missing");
        }
        assert_eq!(v["code"], 7);
        assert_eq!(v["ok"], false);
    }
}
