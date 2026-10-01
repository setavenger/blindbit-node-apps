//! One Bitcoin P2P `version` handshake with the node peer: its height (`start_height`),
//! service bits and user agent. Used before every friglet start (re-resolving the peer to a
//! literal IP) and every 10 minutes for the node tip.

use std::net::SocketAddr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bitcoin::consensus::encode;
use bitcoin::p2p::ServiceFlags;
use bitcoin::p2p::address::Address;
use bitcoin::p2p::message::{NetworkMessage, RawNetworkMessage};
use bitcoin::p2p::message_network::VersionMessage;
use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::network::Network;

pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(15);
const PROTOCOL_VERSION: u32 = 70016;
const MAX_PAYLOAD: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PeerInfo {
    pub addr: SocketAddr,
    pub version: u32,
    pub services: u64,
    pub start_height: i64,
    pub user_agent: String,
}

impl PeerInfo {
    /// NODE_NETWORK_LIMITED without NODE_NETWORK: a pruned node that cannot serve old blocks.
    pub fn is_pruned(&self) -> bool {
        let s = ServiceFlags::from(self.services);
        s.has(ServiceFlags::NETWORK_LIMITED) && !s.has(ServiceFlags::NETWORK)
    }

    pub fn tip(&self) -> Option<u64> {
        u64::try_from(self.start_height).ok()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum P2pError {
    Resolve(String),
    Connect(String),
    Timeout,
    Protocol(String),
}

impl std::fmt::Display for P2pError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            P2pError::Resolve(m) => write!(f, "cannot resolve the node address: {m}"),
            P2pError::Connect(m) => write!(f, "cannot connect to the node: {m}"),
            P2pError::Timeout => f.write_str("the node did not answer the P2P handshake in time"),
            P2pError::Protocol(m) => write!(f, "unexpected P2P answer from the node: {m}"),
        }
    }
}

impl std::error::Error for P2pError {}

/// Resolve `host:port` to one literal socket address, preferring IPv4.
pub async fn resolve(node: &str) -> Result<SocketAddr, P2pError> {
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host(node)
        .await
        .map_err(|e| P2pError::Resolve(format!("{node}: {e}")))?
        .collect();
    addrs
        .iter()
        .find(|a| a.is_ipv4())
        .or_else(|| addrs.first())
        .copied()
        .ok_or_else(|| P2pError::Resolve(format!("{node}: no addresses")))
}

/// Resolve and handshake.
pub async fn check_node(node: &str, network: Network) -> Result<PeerInfo, P2pError> {
    let addr = resolve(node).await?;
    handshake(addr, network, HANDSHAKE_TIMEOUT).await
}

pub async fn handshake(
    addr: SocketAddr,
    network: Network,
    timeout: Duration,
) -> Result<PeerInfo, P2pError> {
    match tokio::time::timeout(timeout, handshake_inner(addr, network)).await {
        Ok(r) => r,
        Err(_) => Err(P2pError::Timeout),
    }
}

async fn handshake_inner(addr: SocketAddr, network: Network) -> Result<PeerInfo, P2pError> {
    let mut stream = TcpStream::connect(addr)
        .await
        .map_err(|e| P2pError::Connect(e.to_string()))?;
    let magic = network.magic();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let unspecified: SocketAddr = ([0, 0, 0, 0], 0).into();
    let mut version = VersionMessage::new(
        ServiceFlags::NONE,
        now,
        Address::new(&addr, ServiceFlags::NONE),
        Address::new(&unspecified, ServiceFlags::NONE),
        rand::random(),
        format!("/friglet-ui:{}/", env!("CARGO_PKG_VERSION")),
        0,
    );
    version.version = PROTOCOL_VERSION;
    version.relay = false;
    write_msg(&mut stream, magic, NetworkMessage::Version(version)).await?;

    loop {
        let msg = read_msg(&mut stream, magic).await?;
        if let NetworkMessage::Version(v) = msg {
            // Be polite; the answer does not matter.
            let _ = write_msg(&mut stream, magic, NetworkMessage::Verack).await;
            let _ = stream.shutdown().await;
            return Ok(PeerInfo {
                addr,
                version: v.version,
                services: v.services.to_u64(),
                start_height: i64::from(v.start_height),
                user_agent: v.user_agent,
            });
        }
    }
}

async fn write_msg(
    stream: &mut TcpStream,
    magic: bitcoin::p2p::Magic,
    msg: NetworkMessage,
) -> Result<(), P2pError> {
    let raw = RawNetworkMessage::new(magic, msg);
    stream
        .write_all(&encode::serialize(&raw))
        .await
        .map_err(|e| P2pError::Connect(e.to_string()))
}

pub(crate) async fn read_msg(
    stream: &mut TcpStream,
    magic: bitcoin::p2p::Magic,
) -> Result<NetworkMessage, P2pError> {
    let mut header = [0u8; 24];
    stream.read_exact(&mut header).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::UnexpectedEof {
            P2pError::Protocol("the node closed the connection".into())
        } else {
            P2pError::Connect(e.to_string())
        }
    })?;
    if header[..4] != magic.to_bytes() {
        return Err(P2pError::Protocol(
            "wrong network magic: the node runs a different network".into(),
        ));
    }
    let len = u32::from_le_bytes([header[16], header[17], header[18], header[19]]) as usize;
    if len > MAX_PAYLOAD {
        return Err(P2pError::Protocol(format!(
            "message too large ({len} bytes)"
        )));
    }
    let mut buf = Vec::with_capacity(24 + len);
    buf.extend_from_slice(&header);
    buf.resize(24 + len, 0);
    stream
        .read_exact(&mut buf[24..])
        .await
        .map_err(|e| P2pError::Connect(e.to_string()))?;
    let raw: RawNetworkMessage =
        encode::deserialize(&buf).map_err(|e| P2pError::Protocol(e.to_string()))?;
    Ok(raw.into_payload())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use tokio::net::TcpListener;

    /// A fake peer answering one handshake with the given services and height.
    pub(crate) async fn fake_peer(network: Network, services: u64, height: i32) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut s, peer)) = listener.accept().await {
                let magic = network.magic();
                let Ok(NetworkMessage::Version(_)) = read_msg(&mut s, magic).await else {
                    continue;
                };
                let mut v = VersionMessage::new(
                    ServiceFlags::from(services),
                    0,
                    Address::new(&peer, ServiceFlags::NONE),
                    Address::new(&addr, ServiceFlags::from(services)),
                    1,
                    "/Satoshi:31.1.0/".into(),
                    height,
                );
                v.version = 70016;
                let _ = write_msg(
                    &mut s,
                    magic,
                    NetworkMessage::Unknown {
                        command: bitcoin::p2p::message::CommandString::try_from_static(
                            "wtxidrelay",
                        )
                        .unwrap(),
                        payload: vec![],
                    },
                )
                .await;
                let _ = write_msg(&mut s, magic, NetworkMessage::Version(v)).await;
                let _ = write_msg(&mut s, magic, NetworkMessage::Verack).await;
            }
        });
        addr
    }

    #[tokio::test]
    async fn reads_height_services_and_user_agent() {
        let addr = fake_peer(Network::Signet, 1 | 8 | 1024, 271_000).await;
        let info = handshake(addr, Network::Signet, Duration::from_secs(5))
            .await
            .unwrap();
        assert_eq!(info.start_height, 271_000);
        assert_eq!(info.tip(), Some(271_000));
        assert_eq!(info.user_agent, "/Satoshi:31.1.0/");
        assert!(
            !info.is_pruned(),
            "NODE_NETWORK and NODE_NETWORK_LIMITED is a full node"
        );
    }

    #[tokio::test]
    async fn pruned_node_is_detected() {
        let addr = fake_peer(Network::Regtest, 8 | 1024, 500).await;
        let info = handshake(addr, Network::Regtest, Duration::from_secs(5))
            .await
            .unwrap();
        assert!(info.is_pruned());
    }

    #[tokio::test]
    async fn wrong_network_is_a_protocol_error() {
        let addr = fake_peer(Network::Bitcoin, 1, 900_000).await;
        // The fake peer ignores a version with a foreign magic, so this times out or errors.
        let r = handshake(addr, Network::Signet, Duration::from_millis(500)).await;
        assert!(r.is_err());
    }

    #[tokio::test]
    async fn silent_peer_times_out_and_closed_port_fails() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let _keep = tokio::spawn(async move {
            let _conn = listener.accept().await;
            tokio::time::sleep(Duration::from_secs(10)).await;
        });
        assert_eq!(
            handshake(addr, Network::Signet, Duration::from_millis(300)).await,
            Err(P2pError::Timeout)
        );
        let closed = TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap()
            .local_addr()
            .unwrap();
        assert!(matches!(
            handshake(closed, Network::Signet, Duration::from_secs(2)).await,
            Err(P2pError::Connect(_))
        ));
    }

    #[tokio::test]
    async fn resolve_prefers_literal_ipv4() {
        assert_eq!(
            resolve("127.0.0.1:8333").await.unwrap().to_string(),
            "127.0.0.1:8333"
        );
        assert_eq!(resolve("localhost:8333").await.unwrap().port(), 8333);
        assert!(matches!(
            resolve("does-not-exist.invalid:1").await,
            Err(P2pError::Resolve(_))
        ));
    }
}
