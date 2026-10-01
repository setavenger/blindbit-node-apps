//! Electrum front: the external TCP and TLS listeners friglet-ui owns, proxying to friglet's
//! loopback Electrum server (127.0.0.1:50001).
//!
//! Listeners are bound only while the gate is open (identity verified) and are dropped, with
//! every client connection, when it closes (wallet change, friglet restart, mismatch).

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tokio_rustls::TlsAcceptor;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Gate {
    pub open: bool,
    /// Bumped to drop every client connection.
    pub generation: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListenerStatus {
    pub addr: Option<SocketAddr>,
    pub bound: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct FrontStatus {
    pub tcp: ListenerStatus,
    pub tls: ListenerStatus,
}

#[derive(Clone)]
pub struct ElectrumFront {
    gate: watch::Sender<Gate>,
    status: Arc<Mutex<FrontStatus>>,
}

#[derive(Clone, Copy)]
enum Which {
    Tcp,
    Tls,
}

impl ElectrumFront {
    /// Start the listener tasks (closed until [`ElectrumFront::open`]).
    pub fn start(
        tcp: Option<SocketAddr>,
        tls: Option<(SocketAddr, Arc<rustls::ServerConfig>)>,
        upstream: SocketAddr,
    ) -> Self {
        let (gate, _) = watch::channel(Gate::default());
        let status = Arc::new(Mutex::new(FrontStatus::default()));
        let front = Self { gate, status };
        if let Some(addr) = tcp {
            front.status.lock().unwrap().tcp.addr = Some(addr);
            tokio::spawn(listener_task(
                Which::Tcp,
                addr,
                None,
                upstream,
                front.gate.subscribe(),
                front.status.clone(),
            ));
        }
        if let Some((addr, cfg)) = tls {
            front.status.lock().unwrap().tls.addr = Some(addr);
            let acceptor = TlsAcceptor::from(cfg);
            tokio::spawn(listener_task(
                Which::Tls,
                addr,
                Some(acceptor),
                upstream,
                front.gate.subscribe(),
                front.status.clone(),
            ));
        }
        front
    }

    pub fn open(&self) {
        self.gate.send_if_modified(|g| {
            let changed = !g.open;
            g.open = true;
            changed
        });
    }

    /// Close the listeners and drop every client.
    pub fn close(&self) {
        self.gate.send_modify(|g| {
            g.open = false;
            g.generation += 1;
        });
    }

    pub fn is_open(&self) -> bool {
        self.gate.borrow().open
    }

    pub fn status(&self) -> FrontStatus {
        self.status.lock().unwrap().clone()
    }

    /// Open and at least one listener bound.
    pub fn listeners_open(&self) -> bool {
        let s = self.status();
        self.is_open() && (s.tcp.bound || s.tls.bound)
    }
}

fn set_status(status: &Mutex<FrontStatus>, which: Which, f: impl FnOnce(&mut ListenerStatus)) {
    let mut s = status.lock().unwrap();
    match which {
        Which::Tcp => f(&mut s.tcp),
        Which::Tls => f(&mut s.tls),
    }
}

async fn listener_task(
    which: Which,
    addr: SocketAddr,
    tls: Option<TlsAcceptor>,
    upstream: SocketAddr,
    mut gate: watch::Receiver<Gate>,
    status: Arc<Mutex<FrontStatus>>,
) {
    loop {
        // Wait for the gate to open.
        while !gate.borrow_and_update().open {
            if gate.changed().await.is_err() {
                return;
            }
        }
        let listener = match TcpListener::bind(addr).await {
            Ok(l) => l,
            Err(e) => {
                tracing::error!(%addr, error = %e, "cannot bind Electrum listener; retrying in 10 s");
                set_status(&status, which, |s| {
                    s.bound = false;
                    s.error = Some(format!("cannot listen on {addr}: {e}"));
                });
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_secs(10)) => {}
                    r = gate.changed() => if r.is_err() { return; },
                }
                continue;
            }
        };
        tracing::info!(%addr, tls = tls.is_some(), "Electrum listener open");
        set_status(&status, which, |s| {
            s.bound = true;
            s.error = None;
        });
        loop {
            tokio::select! {
                accepted = listener.accept() => {
                    let Ok((client, _peer)) = accepted else { continue };
                    let generation = gate.borrow().generation;
                    let conn_gate = gate.clone();
                    let tls = tls.clone();
                    tokio::spawn(async move {
                        let _ = client.set_nodelay(true);
                        match tls {
                            Some(acceptor) => {
                                let Ok(Ok(stream)) = tokio::time::timeout(Duration::from_secs(15), acceptor.accept(client)).await else {
                                    return;
                                };
                                proxy(stream, upstream, conn_gate, generation).await;
                            }
                            None => proxy(client, upstream, conn_gate, generation).await,
                        }
                    });
                }
                r = gate.changed() => {
                    if r.is_err() || !gate.borrow().open {
                        break;
                    }
                }
            }
        }
        drop(listener);
        tracing::info!(%addr, "Electrum listener closed");
        set_status(&status, which, |s| s.bound = false);
    }
}

async fn proxy<S>(
    mut client: S,
    upstream: SocketAddr,
    mut gate: watch::Receiver<Gate>,
    generation: u64,
) where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let Ok(Ok(mut server)) =
        tokio::time::timeout(Duration::from_secs(10), TcpStream::connect(upstream)).await
    else {
        return;
    };
    let _ = server.set_nodelay(true);
    let dropped = async {
        loop {
            {
                let g = gate.borrow_and_update();
                if !g.open || g.generation != generation {
                    return;
                }
            }
            if gate.changed().await.is_err() {
                return;
            }
        }
    };
    tokio::select! {
        _ = tokio::io::copy_bidirectional(&mut client, &mut server) => {}
        _ = dropped => {}
    }
}

/// Send `server.version` to `addr` (optionally over TLS without verifying the certificate)
/// and return the server's answer.
pub async fn server_version(
    addr: SocketAddr,
    tls: bool,
    timeout: Duration,
) -> Result<String, String> {
    let fut = async {
        let tcp = TcpStream::connect(addr).await.map_err(|e| e.to_string())?;
        if tls {
            let cfg = insecure_client_config();
            let connector = tokio_rustls::TlsConnector::from(cfg);
            let name =
                rustls::pki_types::ServerName::try_from("friglet").map_err(|e| e.to_string())?;
            let stream = connector
                .connect(name.to_owned(), tcp)
                .await
                .map_err(|e| e.to_string())?;
            exchange_version(stream).await
        } else {
            exchange_version(tcp).await
        }
    };
    tokio::time::timeout(timeout, fut)
        .await
        .map_err(|_| "no answer in time".to_string())?
}

async fn exchange_version<S: AsyncRead + AsyncWrite + Unpin>(mut s: S) -> Result<String, String> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let req = b"{\"jsonrpc\":\"2.0\",\"id\":0,\"method\":\"server.version\",\"params\":[\"friglet-ui\",\"1.4\"]}\n";
    s.write_all(req).await.map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(s);
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .await
        .map_err(|e| e.to_string())?;
    if line.is_empty() {
        return Err("connection closed without an answer".into());
    }
    let v: serde_json::Value = serde_json::from_str(line.trim()).map_err(|e| e.to_string())?;
    match v.get("result") {
        Some(r) => Ok(r.to_string()),
        None => Err(format!("unexpected answer: {}", v)),
    }
}

/// TLS client that accepts any certificate: only used to test our own self-signed listener.
fn insecure_client_config() -> Arc<rustls::ClientConfig> {
    #[derive(Debug)]
    struct AcceptAny(Arc<rustls::crypto::CryptoProvider>);
    impl rustls::client::danger::ServerCertVerifier for AcceptAny {
        fn verify_server_cert(
            &self,
            _: &rustls::pki_types::CertificateDer<'_>,
            _: &[rustls::pki_types::CertificateDer<'_>],
            _: &rustls::pki_types::ServerName<'_>,
            _: &[u8],
            _: rustls::pki_types::UnixTime,
        ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        }
        fn verify_tls12_signature(
            &self,
            message: &[u8],
            cert: &rustls::pki_types::CertificateDer<'_>,
            dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            rustls::crypto::verify_tls12_signature(
                message,
                cert,
                dss,
                &self.0.signature_verification_algorithms,
            )
        }
        fn verify_tls13_signature(
            &self,
            message: &[u8],
            cert: &rustls::pki_types::CertificateDer<'_>,
            dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            rustls::crypto::verify_tls13_signature(
                message,
                cert,
                dss,
                &self.0.signature_verification_algorithms,
            )
        }
        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            self.0.signature_verification_algorithms.supported_schemes()
        }
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let cfg = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .expect("ring supports the default versions")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAny(provider)))
        .with_no_client_auth();
    Arc::new(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    /// A fake friglet Electrum server answering server.version.
    async fn fake_upstream() -> SocketAddr {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((s, _)) = l.accept().await {
                tokio::spawn(async move {
                    let (r, mut w) = s.into_split();
                    let mut lines = BufReader::new(r).lines();
                    while let Ok(Some(_)) = lines.next_line().await {
                        let _ = w
                            .write_all(b"{\"jsonrpc\":\"2.0\",\"id\":0,\"result\":[\"friglet 0.1.0\",\"1.4\"]}\n")
                            .await;
                    }
                });
            }
        });
        addr
    }

    fn free_port() -> SocketAddr {
        std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
    }

    async fn wait_bound(front: &ElectrumFront, bound: bool) {
        for _ in 0..100 {
            let s = front.status();
            if s.tcp.bound == bound && s.tls.bound == bound {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!(
            "listeners did not reach bound={bound}: {:?}",
            front.status()
        );
    }

    #[tokio::test]
    async fn closed_until_opened_then_proxies_tcp_and_tls() {
        let upstream = fake_upstream().await;
        let (cert, key) = crate::tls::generate(&[]).unwrap();
        let tls = crate::tls::load(cert.as_bytes(), key.as_bytes()).unwrap();
        let tcp_addr = free_port();
        let tls_addr = free_port();
        let front = ElectrumFront::start(Some(tcp_addr), Some((tls_addr, tls.config)), upstream);
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            TcpStream::connect(tcp_addr).await.is_err(),
            "closed before identity is verified"
        );
        front.open();
        wait_bound(&front, true).await;
        assert!(front.listeners_open());
        let v = server_version(tcp_addr, false, Duration::from_secs(5))
            .await
            .unwrap();
        assert!(v.contains("friglet"), "{v}");
        let v = server_version(tls_addr, true, Duration::from_secs(5))
            .await
            .unwrap();
        assert!(v.contains("friglet"), "{v}");
        front.close();
        wait_bound(&front, false).await;
        assert!(TcpStream::connect(tcp_addr).await.is_err());
    }

    #[tokio::test]
    async fn close_drops_connected_clients() {
        let upstream = fake_upstream().await;
        let tcp_addr = free_port();
        let front = ElectrumFront::start(Some(tcp_addr), None, upstream);
        front.open();
        for _ in 0..100 {
            if front.status().tcp.bound {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let mut s = TcpStream::connect(tcp_addr).await.unwrap();
        s.write_all(b"{\"id\":1,\"method\":\"server.version\",\"params\":[]}\n")
            .await
            .unwrap();
        let mut r = BufReader::new(&mut s);
        let mut line = String::new();
        r.read_line(&mut line).await.unwrap();
        assert!(line.contains("result"));
        front.close();
        let mut rest = String::new();
        let n = tokio::time::timeout(Duration::from_secs(5), r.read_line(&mut rest))
            .await
            .unwrap()
            .unwrap_or(0);
        assert_eq!(n, 0, "client connection is closed on wallet change");
    }
}
