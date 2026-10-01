//! Generate a throwaway test wallet: writes `sp(tspscan…)#checksum` (or `spscan` with
//! `--mainnet`) to the given file with mode 0600 and prints the expected address and the
//! scan secret's file path. Test use only; never for real funds.
//!
//! cargo run --example gen_test_descriptor -- <out-file> [signet|testnet4|regtest|bitcoin]

use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;

use bitcoin::secp256k1::{PublicKey, Secp256k1, SecretKey};
use friglet_ui::descriptor::WalletKeys;
use friglet_ui::network::Network;

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args
        .next()
        .expect("usage: gen_test_descriptor <out-file> [network]");
    let network: Network = args
        .next()
        .unwrap_or_else(|| "signet".into())
        .parse()
        .expect("network");
    let secp = Secp256k1::new();
    let scan = SecretKey::from_slice(&rand::random::<[u8; 32]>()).expect("random key");
    let spend = SecretKey::from_slice(&rand::random::<[u8; 32]>()).expect("random key");
    let keys = WalletKeys::new(scan, PublicKey::from_secret_key(&secp, &spend), None);
    let desc = keys.to_descriptor(network.family());
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&out)
        .expect("create out file");
    writeln!(f, "{desc}").expect("write");
    let id = keys.identity(network);
    println!("sp_address={}", id.sp_address);
    println!("key_fingerprint={}", id.key_fingerprint);
}
