//! Iroh channel, `cla-iroh`. Grade P1.
//!
//! Direct QUIC connections between MEM nodes on the same local network:
//!
//! - **Transport key:** a fresh random Iroh key per [`IrohCla::start`] (each app start). It is
//!   never derived from, or linked to, the MEM identity.
//! - **Discovery:** mDNS under the service name [`SERVICE_NAME`]. No n0 DNS/pkarr publishing,
//!   no relays, no port mapping: nothing leaves the local network.
//! - **Connections:** of two nodes, the one with the smaller endpoint id dials; the other
//!   accepts. One bidirectional stream per connection carries length-prefixed encounter
//!   messages (`u32` big-endian length, then the CBOR message, at most
//!   [`MAX_MSG_LEN`] bytes).
//!
//! The agent never sees endpoint ids or addresses, only a local [`PeerHandle`] per connection.
//! Everything a peer sends is hostile: frames are size-checked before allocation and
//! decoded by the bounded `mem-proto` decoder.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use iroh::endpoint::{
    Connection, IdleTimeout, QuicTransportConfig, RecvStream, SendStream, VarInt, presets,
};
use iroh::{Endpoint, EndpointAddr, EndpointId};
use iroh_mdns_address_lookup::{DiscoveryEvent, MdnsAddressLookup};
use mem_proto::encounter::{EncounterMsg, MAX_MSG_LEN};
use n0_future::StreamExt;
use tokio::sync::{mpsc, oneshot};
use tracing::debug;

/// QUIC application protocol for MEM encounters.
pub const ALPN: &[u8] = b"mem/encounter/1";
/// mDNS service name: advertised as `_mem1._udp.local`.
pub const SERVICE_NAME: &str = "mem1";
/// QUIC keep-alive interval.
pub const KEEP_ALIVE: Duration = Duration::from_secs(3);
/// A connection with no traffic for this long is closed.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(10);
/// Messages queued per peer before sends fail fast.
const SEND_QUEUE: usize = 64;
/// How often discovered but unconnected nodes are dialled again.
const REDIAL_EVERY: Duration = Duration::from_secs(10);
/// Give up on a dial after this long.
const DIAL_TIMEOUT: Duration = Duration::from_secs(10);

/// Local handle for one connection. The agent's `PeerHandle` wraps the same number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PeerHandle(pub u64);

/// Errors from this channel.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum IrohError {
    /// The endpoint could not be bound.
    #[error("could not start the network endpoint")]
    Bind,
    /// mDNS could not be started.
    #[error("could not start local discovery")]
    Discovery,
    /// The peer is gone or its queue is full.
    #[error("peer unavailable")]
    PeerUnavailable,
    /// The message could not be encoded.
    #[error("message not encodable")]
    Encode,
}

/// What happens on the network, for the runtime.
#[derive(Debug)]
pub enum IrohEvent {
    /// A connection is up; start the encounter.
    Connected(PeerHandle),
    /// A peer sent a well-formed encounter message.
    Message(PeerHandle, EncounterMsg),
    /// The connection ended (closed, failed, or sent malformed data).
    Disconnected(PeerHandle),
}

/// Configuration.
#[derive(Debug, Clone)]
pub struct IrohConfig {
    /// Discover peers with mDNS (off in tests that wire addresses directly).
    pub mdns: bool,
    /// mDNS service name; tests use a unique one so parallel runs do not meet.
    pub service_name: String,
    /// Peer handles start above this, so two instances (local network and Wi-Fi Direct)
    /// never hand out the same handle.
    pub handle_base: u64,
}

impl Default for IrohConfig {
    fn default() -> Self {
        Self {
            mdns: true,
            service_name: SERVICE_NAME.into(),
            handle_base: 0,
        }
    }
}

type Outgoing = (Vec<u8>, Option<oneshot::Sender<bool>>);

struct PeerConn {
    remote: EndpointId,
    conn: Connection,
    tx: mpsc::Sender<Outgoing>,
}

struct Inner {
    endpoint: Endpoint,
    peers: Mutex<HashMap<PeerHandle, PeerConn>>,
    discovered: Mutex<HashSet<EndpointId>>,
    dialing: Mutex<HashSet<EndpointId>>,
    next: AtomicU64,
    events: mpsc::UnboundedSender<IrohEvent>,
}

/// A running Iroh channel. Cheap to clone.
#[derive(Clone)]
pub struct IrohCla {
    inner: Arc<Inner>,
    _mdns: Option<MdnsAddressLookup>,
}

impl std::fmt::Debug for IrohCla {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IrohCla").finish_non_exhaustive()
    }
}

impl IrohCla {
    /// Binds a fresh endpoint and starts discovery, accepting and dialling. Must run inside
    /// a Tokio runtime.
    ///
    /// # Errors
    ///
    /// [`IrohError::Bind`] or [`IrohError::Discovery`].
    pub async fn start(
        config: IrohConfig,
    ) -> Result<(Self, mpsc::UnboundedReceiver<IrohEvent>), IrohError> {
        // `Minimal`: crypto provider only. No relay transport, no DNS/pkarr publishing.
        let endpoint = Endpoint::builder(presets::Minimal)
            .alpns(vec![ALPN.to_vec()])
            .transport_config(transport_config())
            .bind()
            .await
            .map_err(|_| IrohError::Bind)?;
        let (events, rx) = mpsc::unbounded_channel();
        let inner = Arc::new(Inner {
            endpoint,
            peers: Mutex::new(HashMap::new()),
            discovered: Mutex::new(HashSet::new()),
            dialing: Mutex::new(HashSet::new()),
            next: AtomicU64::new(config.handle_base + 1),
            events,
        });
        let mdns = if config.mdns {
            let mdns = MdnsAddressLookup::builder()
                .service_name(config.service_name)
                .build(inner.endpoint.id())
                .map_err(|_| IrohError::Discovery)?;
            inner
                .endpoint
                .address_lookup()
                .map_err(|_| IrohError::Discovery)?
                .add(mdns.clone());
            tokio::spawn(discover(Arc::clone(&inner), mdns.subscribe().await));
            tokio::spawn(redial(Arc::clone(&inner)));
            Some(mdns)
        } else {
            None
        };
        tokio::spawn(accept(Arc::clone(&inner)));
        Ok((Self { inner, _mdns: mdns }, rx))
    }

    /// This node's address (tests use it to connect without mDNS).
    #[must_use]
    pub fn addr(&self) -> EndpointAddr {
        self.inner.endpoint.addr()
    }

    /// Dials a node at a known address (tests; mDNS does this automatically).
    pub fn connect(&self, addr: EndpointAddr) {
        tokio::spawn(dial(Arc::clone(&self.inner), addr));
    }

    /// This endpoint's id and UDP port, for an advertisement outside mDNS (Wi-Fi Direct).
    /// The id is the per-start transport key, never the MEM identity.
    #[must_use]
    pub fn local(&self) -> ([u8; 32], Option<u16>) {
        let port = self
            .inner
            .endpoint
            .bound_sockets()
            .iter()
            .find(|a| a.is_ipv4())
            .map(std::net::SocketAddr::port);
        (*self.inner.endpoint.id().as_bytes(), port)
    }

    /// This endpoint's IPv4 addresses on the local networks, for a connect hint: no
    /// loopback, unspecified or link-local ones; private (LAN) addresses first; at most `max`.
    #[must_use]
    pub fn lan_ipv4(&self, max: usize) -> Vec<std::net::SocketAddrV4> {
        let mut out: Vec<std::net::SocketAddrV4> = self
            .addr()
            .ip_addrs()
            .filter_map(|a| match a {
                std::net::SocketAddr::V4(v4) => Some(*v4),
                std::net::SocketAddr::V6(_) => None,
            })
            .filter(|a| {
                let ip = a.ip();
                !ip.is_loopback() && !ip.is_unspecified() && !ip.is_link_local()
            })
            .collect();
        out.sort_by_key(|a| (!a.ip().is_private(), *a));
        out.dedup();
        out.truncate(max);
        out
    }

    /// Dials the endpoint `id` at any of `addrs` in one attempt (a connect hint).
    /// Ignored for an invalid id or our own.
    pub fn connect_at(&self, id: [u8; 32], addrs: &[std::net::SocketAddr]) {
        let Ok(id) = EndpointId::from_bytes(&id) else {
            return;
        };
        if id == self.inner.endpoint.id() || addrs.is_empty() {
            return;
        }
        let addr = addrs
            .iter()
            .fold(EndpointAddr::new(id), |a, s| a.with_ip_addr(*s));
        self.connect(addr);
    }

    /// Dials the endpoint `id` at `addr` (Wi-Fi Direct: the group owner's address and the port
    /// from its advertisement). Ignored for an invalid id.
    pub fn connect_direct(&self, id: [u8; 32], addr: std::net::SocketAddr) {
        if let Ok(id) = EndpointId::from_bytes(&id) {
            self.connect(EndpointAddr::new(id).with_ip_addr(addr));
        }
    }

    /// Queues `msg` for `peer`. The returned receiver yields `true` once the bytes were written
    /// to the stream, `false` if the connection failed first.
    ///
    /// # Errors
    ///
    /// [`IrohError::PeerUnavailable`] if the peer is gone or backed up;
    /// [`IrohError::Encode`] if the message violates its limits.
    pub fn send(
        &self,
        peer: PeerHandle,
        msg: &EncounterMsg,
    ) -> Result<oneshot::Receiver<bool>, IrohError> {
        let bytes = msg.encode().map_err(|_| IrohError::Encode)?;
        let (done, rx) = oneshot::channel();
        let peers = self
            .inner
            .peers
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let p = peers.get(&peer).ok_or(IrohError::PeerUnavailable)?;
        p.tx.try_send((bytes, Some(done)))
            .map_err(|_| IrohError::PeerUnavailable)?;
        Ok(rx)
    }

    /// Closes the connection to `peer` (after a `bye` or a protocol violation).
    pub fn close(&self, peer: PeerHandle) {
        let removed = self
            .inner
            .peers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&peer);
        if let Some(p) = removed {
            p.conn.close(VarInt::from_u32(0), b"bye");
        }
    }

    /// Number of open connections.
    #[must_use]
    pub fn connections(&self) -> usize {
        self.inner
            .peers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// Closes every connection and the endpoint.
    pub async fn shutdown(&self) {
        let all: Vec<PeerConn> = self
            .inner
            .peers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .drain()
            .map(|(_, p)| p)
            .collect();
        for p in all {
            p.conn.close(VarInt::from_u32(0), b"bye");
        }
        self.inner.endpoint.close().await;
    }
}

/// Tracks mDNS results; dials newly discovered nodes we are responsible for dialling.
async fn discover(
    inner: Arc<Inner>,
    mut events: impl n0_future::Stream<Item = DiscoveryEvent> + Unpin,
) {
    while let Some(event) = events.next().await {
        match event {
            DiscoveryEvent::Discovered { endpoint_info, .. } => {
                let id = endpoint_info.endpoint_id;
                inner
                    .discovered
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .insert(id);
                maybe_dial(&inner, id);
            }
            DiscoveryEvent::Expired { endpoint_id } => {
                inner
                    .discovered
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .remove(&endpoint_id);
            }
            _ => {}
        }
        if inner.endpoint.is_closed() {
            break;
        }
    }
}

/// Periodically retries discovered nodes that have no connection (after a drop or failure).
async fn redial(inner: Arc<Inner>) {
    let mut tick = tokio::time::interval(REDIAL_EVERY);
    loop {
        tick.tick().await;
        if inner.endpoint.is_closed() {
            break;
        }
        let ids: Vec<EndpointId> = inner
            .discovered
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .copied()
            .collect();
        for id in ids {
            maybe_dial(&inner, id);
        }
    }
}

/// Dials `id` if we are the side that dials (smaller id), are not connected and not already
/// dialling. Prevents duplicate connections when both sides discover each other.
fn maybe_dial(inner: &Arc<Inner>, id: EndpointId) {
    if inner.endpoint.id() >= id {
        return;
    }
    let connected = inner
        .peers
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .values()
        .any(|p| p.remote == id);
    if connected
        || !inner
            .dialing
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(id)
    {
        return;
    }
    tokio::spawn(dial(Arc::clone(inner), EndpointAddr::from(id)));
}

async fn dial(inner: Arc<Inner>, addr: EndpointAddr) {
    let id = addr.id;
    let attempt = async {
        let conn = inner.endpoint.connect(addr, ALPN).await.ok()?;
        let (send, recv) = conn.open_bi().await.ok()?;
        Some((conn, send, recv))
    };
    let result = tokio::time::timeout(DIAL_TIMEOUT, attempt).await;
    inner
        .dialing
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&id);
    if let Ok(Some((conn, send, recv))) = result {
        register(&inner, conn, send, recv);
    } else {
        debug!("dial failed");
    }
}

async fn accept(inner: Arc<Inner>) {
    while let Some(incoming) = inner.endpoint.accept().await {
        let inner = Arc::clone(&inner);
        tokio::spawn(async move {
            let Ok(conn) = incoming.await else {
                return;
            };
            if conn.alpn() != ALPN {
                conn.close(VarInt::from_u32(1), b"alpn");
                return;
            }
            // The dialler opens the stream and writes `hello` first.
            if let Ok(Ok((send, recv))) = tokio::time::timeout(DIAL_TIMEOUT, conn.accept_bi()).await
            {
                register(&inner, conn, send, recv);
            }
        });
    }
}

/// QUIC keep-alive and idle timeout for failure detection: a vanished device is
/// noticed within about [`IDLE_TIMEOUT`], so its unconfirmed messages wait for the next encounter
/// instead of being written into a dead connection for half a minute.
fn transport_config() -> QuicTransportConfig {
    let builder = QuicTransportConfig::builder()
        .keep_alive_interval(KEEP_ALIVE)
        .default_path_keep_alive_interval(KEEP_ALIVE)
        .default_path_max_idle_timeout(IDLE_TIMEOUT);
    match IdleTimeout::try_from(IDLE_TIMEOUT) {
        Ok(t) => builder.max_idle_timeout(Some(t)),
        Err(_) => builder,
    }
    .build()
}

/// Registers a connection, starts its reader and writer, and announces it.
fn register(inner: &Arc<Inner>, conn: Connection, send: SendStream, recv: RecvStream) {
    let peer = PeerHandle(inner.next.fetch_add(1, Ordering::Relaxed));
    let (tx, rx) = mpsc::channel(SEND_QUEUE);
    inner
        .peers
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(
            peer,
            PeerConn {
                remote: conn.remote_id(),
                conn: conn.clone(),
                tx,
            },
        );
    let _ = inner.events.send(IrohEvent::Connected(peer));
    tokio::spawn(writer(send, rx));
    tokio::spawn(reader(Arc::clone(inner), peer, conn, recv));
}

async fn writer(mut send: SendStream, mut rx: mpsc::Receiver<Outgoing>) {
    while let Some((bytes, done)) = rx.recv().await {
        let ok = write_frame(&mut send, &bytes).await;
        if let Some(done) = done {
            let _ = done.send(ok);
        }
        if !ok {
            break;
        }
    }
    let _ = send.finish();
}

async fn write_frame(send: &mut SendStream, bytes: &[u8]) -> bool {
    let Ok(len) = u32::try_from(bytes.len()) else {
        return false;
    };
    send.write_all(&len.to_be_bytes()).await.is_ok() && send.write_all(bytes).await.is_ok()
}

async fn reader(inner: Arc<Inner>, peer: PeerHandle, conn: Connection, mut recv: RecvStream) {
    while let Some(msg) = read_frame(&mut recv).await {
        if inner.events.send(IrohEvent::Message(peer, msg)).is_err() {
            break;
        }
    }
    // EOF, error or a malformed frame: the session is over. Also reported after a local
    // `close`, so the agent always forgets the peer.
    inner
        .peers
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .remove(&peer);
    conn.close(VarInt::from_u32(0), b"done");
    let _ = inner.events.send(IrohEvent::Disconnected(peer));
}

/// Reads one length-prefixed message. `None` on EOF, I/O error, oversize or malformed input.
async fn read_frame(recv: &mut RecvStream) -> Option<EncounterMsg> {
    let mut len = [0u8; 4];
    recv.read_exact(&mut len).await.ok()?;
    let len = usize::try_from(u32::from_be_bytes(len)).ok()?;
    // Checked before allocating.
    if len == 0 || len > MAX_MSG_LEN {
        return None;
    }
    let mut buf = vec![0u8; len];
    recv.read_exact(&mut buf).await.ok()?;
    EncounterMsg::decode(&buf).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn next_event(rx: &mut mpsc::UnboundedReceiver<IrohEvent>) -> IrohEvent {
        tokio::time::timeout(Duration::from_secs(20), rx.recv())
            .await
            .unwrap()
            .unwrap()
    }

    fn no_mdns() -> IrohConfig {
        IrohConfig {
            mdns: false,
            ..IrohConfig::default()
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn direct_connection_exchanges_messages_both_ways() {
        let (a, mut ea) = IrohCla::start(no_mdns()).await.unwrap();
        let (b, mut eb) = IrohCla::start(no_mdns()).await.unwrap();
        a.connect(b.addr());
        let IrohEvent::Connected(pa) = next_event(&mut ea).await else {
            panic!()
        };
        // The dialler writes first; the acceptor sees the connection with that message.
        let done = a.send(pa, &EncounterMsg::Tags(vec![[1; 16]])).unwrap();
        assert!(done.await.unwrap());
        let IrohEvent::Connected(pb) = next_event(&mut eb).await else {
            panic!()
        };
        let IrohEvent::Message(from, msg) = next_event(&mut eb).await else {
            panic!()
        };
        assert_eq!((from, msg), (pb, EncounterMsg::Tags(vec![[1; 16]])));

        let big = EncounterMsg::Bundles(vec![
            mem_proto::encounter::BundleEntry::bare(vec![
                7u8;
                60_000
            ]);
            2
        ]);
        assert!(b.send(pb, &big).unwrap().await.unwrap());
        let IrohEvent::Message(_, got) = next_event(&mut ea).await else {
            panic!()
        };
        assert_eq!(got, big);

        a.close(pa);
        assert!(matches!(
            next_event(&mut eb).await,
            IrohEvent::Disconnected(p) if p == pb
        ));
        a.shutdown().await;
        b.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn oversized_frame_ends_the_session() {
        let (a, mut ea) = IrohCla::start(no_mdns()).await.unwrap();
        let (b, mut eb) = IrohCla::start(no_mdns()).await.unwrap();
        // Speak raw QUIC to `b` with the MEM ALPN and lie about the length.
        let conn = a.inner.endpoint.connect(b.addr(), ALPN).await.unwrap();
        let (mut send, _recv) = conn.open_bi().await.unwrap();
        let too_big = u32::try_from(MAX_MSG_LEN + 1).unwrap();
        send.write_all(&too_big.to_be_bytes()).await.unwrap();
        assert!(matches!(next_event(&mut eb).await, IrohEvent::Connected(_)));
        assert!(matches!(
            next_event(&mut eb).await,
            IrohEvent::Disconnected(_)
        ));
        drop(ea.try_recv());
        a.shutdown().await;
        b.shutdown().await;
    }

    /// Real mDNS on this machine's interfaces. Ignored by default: CI containers often have
    /// no multicast. Run with `cargo test -p cla-iroh -- --ignored`.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "needs multicast on the host network"]
    async fn mdns_discovery_connects_exactly_once() {
        let service = format!("memtest{}", std::process::id());
        let cfg = IrohConfig {
            mdns: true,
            service_name: service,
            handle_base: 0,
        };
        let (a, mut ea) = IrohCla::start(cfg.clone()).await.unwrap();
        let (b, mut eb) = IrohCla::start(cfg).await.unwrap();
        // Only the smaller id dials; it sees the connection first.
        let (dialer, events) = if a.inner.endpoint.id() < b.inner.endpoint.id() {
            (&a, &mut ea)
        } else {
            (&b, &mut eb)
        };
        let IrohEvent::Connected(peer) = next_event(events).await else {
            panic!()
        };
        assert!(
            dialer
                .send(peer, &EncounterMsg::Bye)
                .unwrap()
                .await
                .unwrap()
        );
        // Let discovery repeat: still exactly one connection on each side.
        tokio::time::sleep(Duration::from_secs(3)).await;
        assert_eq!(a.connections(), 1);
        assert_eq!(b.connections(), 1);
        a.shutdown().await;
        b.shutdown().await;
    }
}
