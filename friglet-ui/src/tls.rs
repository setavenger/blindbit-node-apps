//! Self-signed certificate for the Electrum TLS listener: generated once with rcgen, stored
//! in the data dir (key 0600), kept stable across restarts (wallets pin it on first use).
//! The SHA-256 fingerprint of the DER certificate is shown on the Connect page.

use std::fs;
use std::io;
use std::sync::Arc;

use bitcoin::hashes::{Hash, sha256};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};

use crate::files::{self, DataDir, TLS_CERT_FILE, TLS_KEY_FILE};

pub struct TlsMaterial {
    pub config: Arc<rustls::ServerConfig>,
    pub fingerprint_sha256: String,
}

/// Load the certificate, generating it first when missing.
pub fn ensure(dd: &DataDir, hosts: &[String]) -> io::Result<TlsMaterial> {
    let cert_path = dd.path(TLS_CERT_FILE);
    let key_path = dd.path(TLS_KEY_FILE);
    if !cert_path.is_file() || !key_path.is_file() {
        let (cert_pem, key_pem) = generate(hosts)?;
        files::write_atomic(&key_path, key_pem.as_bytes())?;
        files::write_atomic(&cert_path, cert_pem.as_bytes())?;
    }
    load(&fs::read(&cert_path)?, &fs::read(&key_path)?)
}

/// New self-signed certificate for `hosts` plus localhost names; returns (cert, key) PEM.
pub fn generate(hosts: &[String]) -> io::Result<(String, String)> {
    let mut names: Vec<String> = vec!["friglet".into(), "localhost".into(), "127.0.0.1".into()];
    for h in hosts {
        if !h.is_empty() && !names.contains(h) {
            names.push(h.clone());
        }
    }
    let certified = rcgen::generate_simple_self_signed(names).map_err(io::Error::other)?;
    Ok((certified.cert.pem(), certified.key_pair.serialize_pem()))
}

pub fn load(cert_pem: &[u8], key_pem: &[u8]) -> io::Result<TlsMaterial> {
    let certs: Vec<CertificateDer<'static>> = CertificateDer::pem_slice_iter(cert_pem)
        .collect::<Result<_, _>>()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("certificate: {e:?}")))?;
    let first = certs
        .first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "no certificate in PEM"))?;
    let fingerprint_sha256 = fingerprint(first.as_ref());
    let key = PrivateKeyDer::from_pem_slice(key_pem)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("private key: {e:?}")))?;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(io::Error::other)?
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(io::Error::other)?;
    Ok(TlsMaterial {
        config: Arc::new(config),
        fingerprint_sha256,
    })
}

/// `AB:CD:…` SHA-256 of a DER certificate.
pub fn fingerprint(der: &[u8]) -> String {
    sha256::Hash::hash(der)
        .as_byte_array()
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn generated_once_and_stable() {
        let t = tempfile::tempdir().unwrap();
        let dd = DataDir::create(t.path().join("d")).unwrap();
        let a = ensure(&dd, &["umbrel.local".into()]).unwrap();
        let b = ensure(&dd, &["other.local".into()]).unwrap();
        assert_eq!(
            a.fingerprint_sha256, b.fingerprint_sha256,
            "certificate is not regenerated"
        );
        assert_eq!(a.fingerprint_sha256.len(), 32 * 3 - 1);
        let mode = fs::metadata(dd.path(TLS_KEY_FILE))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
