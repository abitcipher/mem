//! Hub channel client.
//!
//! One WebSocket to the hub at a time, binary frames only, each frame one [`HubMsg`] in
//! deterministic CBOR. The client reconnects with exponential backoff (1 s to 60 s) and reports
//! what happens as [`HubEvent`]s; the sans-IO agent decides what to upload, fetch and purge.
//!
//! - TLS through rustls with the Mozilla root store (`wss://`); plain `ws://` only reaches this
//!   machine, which the agent enforces when the address is set.
//! - Frames are bounded before they are buffered: at most one encounter-sized message.
//! - An upload is reported done only on the hub's `put_ok`, never on the write: "sent" means the
//!   hub holds it.
//! - Purge tokens given while disconnected wait (bounded) and go out after reconnecting.

use std::collections::{BTreeSet, VecDeque};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use mem_proto::encounter::MAX_MSG_LEN;
use mem_proto::hub::{Crossing, HUB_VERSION, HubLimits, HubMsg, HubOrigin};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::protocol::WebSocketConfig;

/// First retry after a lost connection; doubles up to [`MAX_BACKOFF`].
const MIN_BACKOFF: Duration = Duration::from_secs(1);
/// Longest wait between connection attempts.
const MAX_BACKOFF: Duration = Duration::from_secs(60);
/// The hub must answer `hello` within this long.
const WELCOME_TIMEOUT: Duration = Duration::from_secs(10);
/// A ping this often keeps NATs and proxies from dropping an idle socket.
const PING_EVERY: Duration = Duration::from_secs(25);
/// Purge tokens kept across a reconnect.
const MAX_HELD_TOKENS: usize = 4096;

/// What the hub connection reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HubEvent {
    /// Connected and greeted; the hub's limits.
    Connected(HubLimits),
    /// Bundles matching the fetch (untrusted), each with where the hub says it came from.
    Bundles(Vec<(Vec<u8>, HubOrigin)>),
    /// The backlog for the last fetch is complete.
    FetchDone,
    /// The hub answered an upload: `ok` if it holds the bundle.
    PutResult {
        /// The id given to [`HubClient::put`].
        id: u64,
        /// Stored (or already held).
        ok: bool,
    },
    /// The connection ended; the client keeps retrying until [`HubClient::shutdown`].
    Disconnected,
    /// The hub sent its signed hub list (untrusted until the core checks it).
    HubList(Vec<u8>),
}

enum Command {
    Put {
        id: u64,
        bundle: Vec<u8>,
        crossing: Crossing,
    },
    Fetch(Vec<[u8; 2]>),
    Purge(Vec<[u8; 16]>),
    WakeRegister(String, Vec<[u8; 2]>),
    WakeUnregister(String),
    GetHubs,
    Shutdown,
}

/// Handle to the hub connection task. Cheap to clone.
#[derive(Clone, Debug)]
pub struct HubClient {
    tx: mpsc::UnboundedSender<Command>,
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Command")
    }
}

impl HubClient {
    /// Starts connecting to `url` in the current Tokio runtime.
    ///
    /// # Panics
    ///
    /// Outside a Tokio runtime (as `tokio::spawn` does).
    #[must_use]
    pub fn start(url: String) -> (Self, mpsc::UnboundedReceiver<HubEvent>) {
        Self::start_many(alloc_urls(url))
    }

    /// Starts with several hubs: stays with the first that works and moves to the
    /// next after a failure.
    ///
    /// # Panics
    ///
    /// Outside a Tokio runtime, or with no URL.
    #[must_use]
    pub fn start_many(urls: Vec<String>) -> (Self, mpsc::UnboundedReceiver<HubEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let (events, events_rx) = mpsc::unbounded_channel();
        tokio::spawn(run(urls, rx, events));
        (Self { tx }, events_rx)
    }

    /// Asks the hub for its signed hub list ([`HubEvent::HubList`] follows).
    pub fn get_hubs(&self) {
        let _ = self.tx.send(Command::GetHubs);
    }

    /// Uploads a bundle; a [`HubEvent::PutResult`] with `id` follows (failed at once while
    /// disconnected). `crossing` says whether partner operators' hubs may carry it.
    pub fn put(&self, id: u64, bundle: Vec<u8>, crossing: Crossing) {
        let _ = self.tx.send(Command::Put {
            id,
            bundle,
            crossing,
        });
    }

    /// Replaces the fetch set.
    pub fn fetch(&self, prefixes: Vec<[u8; 2]>) {
        let _ = self.tx.send(Command::Fetch(prefixes));
    }

    /// Presents purge tokens (kept until connected).
    pub fn purge(&self, tokens: Vec<[u8; 16]>) {
        let _ = self.tx.send(Command::Purge(tokens));
    }

    /// Asks the hub to wake `endpoint` for `prefixes`; dropped while disconnected
    /// (the agent registers again on every connection).
    pub fn wake_register(&self, endpoint: String, prefixes: Vec<[u8; 2]>) {
        let _ = self.tx.send(Command::WakeRegister(endpoint, prefixes));
    }

    /// Withdraws a wake-up registration.
    pub fn wake_unregister(&self, endpoint: String) {
        let _ = self.tx.send(Command::WakeUnregister(endpoint));
    }

    /// Says goodbye and stops reconnecting.
    pub fn shutdown(&self) {
        let _ = self.tx.send(Command::Shutdown);
    }
}

enum Exit {
    Lost,
    Shutdown,
}

fn config() -> WebSocketConfig {
    let mut c = WebSocketConfig::default();
    // Checked before buffering: one hub message, a little framing slack.
    c.max_message_size = Some(MAX_MSG_LEN + 64);
    c.max_frame_size = Some(MAX_MSG_LEN + 64);
    c
}

fn alloc_urls(url: String) -> Vec<String> {
    vec![url]
}

async fn run(
    urls: Vec<String>,
    mut cmds: mpsc::UnboundedReceiver<Command>,
    events: mpsc::UnboundedSender<HubEvent>,
) {
    let mut backoff = MIN_BACKOFF;
    let mut held: VecDeque<[u8; 16]> = VecDeque::new();
    let mut at = 0usize;
    loop {
        let Some(url) = urls.get(at % urls.len().max(1)) else {
            return;
        };
        let connected =
            tokio_tungstenite::connect_async_with_config(url.as_str(), Some(config()), false).await;
        // The next attempt goes to the next hub, unless this one worked.
        at = at.wrapping_add(1);
        if let Ok((ws, _)) = connected {
            at = at.wrapping_sub(1);
            match session(ws, &mut cmds, &events, &mut held).await {
                Exit::Shutdown => return,
                Exit::Lost => {
                    backoff = MIN_BACKOFF;
                    // A hub that drops us is skipped next time: fail over.
                    at = at.wrapping_add(1);
                    if events.send(HubEvent::Disconnected).is_err() {
                        return;
                    }
                }
            }
        } else {
            tracing::debug!("hub unreachable");
        }
        // Wait before retrying; meanwhile uploads fail fast and tokens wait.
        let wait = tokio::time::sleep(backoff);
        tokio::pin!(wait);
        loop {
            tokio::select! {
                () = &mut wait => break,
                cmd = cmds.recv() => match cmd {
                    None | Some(Command::Shutdown) => return,
                    Some(Command::Put { id, .. }) => {
                        let _ = events.send(HubEvent::PutResult { id, ok: false });
                    }
                    Some(Command::Purge(t)) => hold(&mut held, t),
                    Some(
                        Command::Fetch(_)
                        | Command::WakeRegister(..)
                        | Command::WakeUnregister(_)
                        | Command::GetHubs,
                    ) => {}
                },
            }
        }
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
}

fn hold(held: &mut VecDeque<[u8; 16]>, tokens: Vec<[u8; 16]>) {
    for t in tokens {
        if held.len() >= MAX_HELD_TOKENS {
            held.pop_front();
        }
        held.push_back(t);
    }
}

async fn send<S>(ws: &mut S, msg: &HubMsg) -> bool
where
    S: SinkExt<Message> + Unpin,
{
    match msg.encode() {
        Ok(bytes) => ws.send(Message::Binary(bytes.into())).await.is_ok(),
        Err(_) => true,
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one connection's whole life: greeting, then one select loop"
)]
async fn session<S>(
    mut ws: S,
    cmds: &mut mpsc::UnboundedReceiver<Command>,
    events: &mpsc::UnboundedSender<HubEvent>,
    held: &mut VecDeque<[u8; 16]>,
) -> Exit
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
        + SinkExt<Message>
        + Unpin,
{
    if !send(
        &mut ws,
        &HubMsg::Hello {
            version: HUB_VERSION,
        },
    )
    .await
    {
        return Exit::Lost;
    }
    let welcome = tokio::time::timeout(WELCOME_TIMEOUT, async {
        while let Some(Ok(m)) = ws.next().await {
            if let Message::Binary(b) = m {
                return match HubMsg::decode(&b) {
                    Ok(HubMsg::Welcome { version, limits }) if version == HUB_VERSION => {
                        Some(limits)
                    }
                    _ => None,
                };
            }
        }
        None
    })
    .await;
    let Ok(Some(limits)) = welcome else {
        return Exit::Lost;
    };
    if events.send(HubEvent::Connected(limits)).is_err() {
        return Exit::Shutdown;
    }
    // Tokens held while disconnected go first.
    while !held.is_empty() {
        let n = held.len().min(mem_proto::encounter::MAX_PURGE_TOKENS);
        let batch: Vec<[u8; 16]> = held.drain(..n).collect();
        if !send(&mut ws, &HubMsg::Purge(batch)).await {
            return Exit::Lost;
        }
    }
    let mut in_flight: BTreeSet<u64> = BTreeSet::new();
    let mut ping = tokio::time::interval(PING_EVERY);
    ping.tick().await;
    let exit = loop {
        tokio::select! {
            frame = ws.next() => {
                let Some(Ok(frame)) = frame else { break Exit::Lost };
                match frame {
                    Message::Binary(b) => match HubMsg::decode(&b) {
                        Ok(HubMsg::Bundles { bundles, foreign }) => {
                            let marked = bundles
                                .into_iter()
                                .enumerate()
                                .map(|(i, b)| {
                                    let other = foreign.iter().any(|f| usize::from(*f) == i);
                                    let origin = if other {
                                        HubOrigin::OtherOperator
                                    } else {
                                        HubOrigin::OwnOperator
                                    };
                                    (b, origin)
                                })
                                .collect();
                            if events.send(HubEvent::Bundles(marked)).is_err() {
                                break Exit::Shutdown;
                            }
                        }
                        Ok(HubMsg::FetchDone) => {
                            if events.send(HubEvent::FetchDone).is_err() {
                                break Exit::Shutdown;
                            }
                        }
                        Ok(HubMsg::PutOk { id }) => {
                            in_flight.remove(&id);
                            let _ = events.send(HubEvent::PutResult { id, ok: true });
                        }
                        Ok(HubMsg::Hubs(list)) => {
                            let _ = events.send(HubEvent::HubList(list));
                        }
                        Ok(HubMsg::PutErr { id, reason }) => {
                            in_flight.remove(&id);
                            tracing::debug!(?reason, "hub refused an upload");
                            let _ = events.send(HubEvent::PutResult { id, ok: false });
                        }
                        // `bye`, or anything a hub must not send: the session is over.
                        _ => break Exit::Lost,
                    },
                    Message::Close(_) | Message::Text(_) => break Exit::Lost,
                    Message::Ping(_) | Message::Pong(_) | Message::Frame(_) => {}
                }
            }
            cmd = cmds.recv() => {
                let msg = match cmd {
                    None | Some(Command::Shutdown) => {
                        let _ = send(&mut ws, &HubMsg::Bye).await;
                        let _ = ws.close().await;
                        break Exit::Shutdown;
                    }
                    Some(Command::Put { id, bundle, crossing }) => {
                        in_flight.insert(id);
                        HubMsg::Put { id, bundle, may_cross: crossing == Crossing::Partners }
                    }
                    Some(Command::Fetch(p)) => HubMsg::Fetch(p),
                    Some(Command::Purge(t)) => HubMsg::Purge(t),
                    Some(Command::WakeRegister(endpoint, prefixes)) => {
                        HubMsg::WakeRegister { endpoint, prefixes }
                    }
                    Some(Command::WakeUnregister(endpoint)) => HubMsg::WakeUnregister { endpoint },
                    Some(Command::GetHubs) => HubMsg::GetHubs,
                };
                if !send(&mut ws, &msg).await {
                    if let HubMsg::Purge(t) = msg {
                        hold(held, t);
                    }
                    break Exit::Lost;
                }
            }
            _ = ping.tick() => {
                if ws.send(Message::Ping(Vec::new().into())).await.is_err() {
                    break Exit::Lost;
                }
            }
        }
    };
    // Unanswered uploads failed; the agent retries them on the next connection.
    for id in in_flight {
        let _ = events.send(HubEvent::PutResult { id, ok: false });
    }
    exit
}

#[cfg(feature = "test-hub")]
pub mod test_hub;
