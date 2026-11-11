//! An in-process hub for tests (feature `test-hub`): a real WebSocket server that follows
//! the hub protocol's rules. It stores envelopes by tag, answers a fetch with everything under the
//! prefixes and then pushes new matches live, never deletes on fetch, and deletes only on a purge
//! token whose commitment matches.
//!
//! Partner operators are simulated: a bundle uploaded as "may cross" is delivered
//! marked as if it had come from another operator's hub.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, PoisonError};

use futures_util::{SinkExt, StreamExt};
use mem_crypto::PurgeToken;
use mem_crypto::purge::commit;
use mem_proto::Bundle;
use mem_proto::hub::{HUB_VERSION, HubLimits, HubMsg, PutError};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;

/// (tag, commitment) -> (bundle, uploaded as "may cross").
type Store = BTreeMap<([u8; 16], [u8; 16]), (Vec<u8>, bool)>;

/// A running test hub.
#[derive(Clone, Debug)]
pub struct TestHub {
    /// Where it listens.
    pub addr: SocketAddr,
    store: Arc<Mutex<Store>>,
}

impl TestHub {
    /// Starts listening on a free loopback port in the current Tokio runtime.
    ///
    /// # Panics
    ///
    /// If no port can be bound.
    #[allow(clippy::unwrap_used, reason = "test helper")]
    pub async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let store: Arc<Mutex<Store>> = Arc::default();
        let (new, _) = broadcast::channel::<(Vec<u8>, bool)>(256);
        let s = Arc::clone(&store);
        tokio::spawn(async move {
            while let Ok((tcp, _)) = listener.accept().await {
                let (s, new) = (Arc::clone(&s), new.clone());
                tokio::spawn(async move {
                    if let Ok(ws) = tokio_tungstenite::accept_async(tcp).await {
                        client(ws, s, new).await;
                    }
                });
            }
        });
        Self { addr, store }
    }

    /// `ws://` address of the hub.
    #[must_use]
    pub fn url(&self) -> String {
        format!("ws://{}/v1", self.addr)
    }

    /// Bundles held now.
    #[must_use]
    pub fn len(&self) -> usize {
        self.store
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// Whether it holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

fn matches(prefixes: &[[u8; 2]], bundle: &[u8]) -> bool {
    Bundle::decode(bundle).is_ok_and(|b| {
        let t = b.dst_tag();
        prefixes.iter().any(|p| t.as_bytes()[..2] == p[..])
    })
}

async fn client<S>(mut ws: S, store: Arc<Mutex<Store>>, new: broadcast::Sender<(Vec<u8>, bool)>)
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
        + SinkExt<Message>
        + Unpin,
{
    let send = |m: &HubMsg| Message::Binary(m.encode().unwrap_or_default().into());
    let mut live = new.subscribe();
    let mut prefixes: Vec<[u8; 2]> = Vec::new();
    loop {
        tokio::select! {
            frame = ws.next() => {
                let Some(Ok(Message::Binary(b))) = frame else { return };
                let Ok(msg) = HubMsg::decode(&b) else { return };
                let reply: Vec<HubMsg> = match msg {
                    HubMsg::Hello { .. } => vec![HubMsg::Welcome {
                        version: HUB_VERSION,
                        limits: HubLimits { max_bundle: 65_536, per_tag: 524_288, puts_per_minute: 600 },
                    }],
                    HubMsg::Put { id, bundle, may_cross } => match Bundle::decode(&bundle) {
                        Ok(b) => {
                            store
                                .lock()
                                .unwrap_or_else(PoisonError::into_inner)
                                .insert(
                                    (*b.dst_tag().as_bytes(), *b.purge_commitment()),
                                    (bundle.clone(), may_cross),
                                );
                            let _ = new.send((bundle, may_cross));
                            vec![HubMsg::PutOk { id }]
                        }
                        Err(_) => vec![HubMsg::PutErr { id, reason: PutError::Malformed }],
                    },
                    HubMsg::Fetch(p) => {
                        prefixes = p;
                        let found: Vec<(Vec<u8>, bool)> = store
                            .lock()
                            .unwrap_or_else(PoisonError::into_inner)
                            .values()
                            .filter(|(b, _)| matches(&prefixes, b))
                            .cloned()
                            .collect();
                        let mut out: Vec<HubMsg> = found.chunks(16).map(batch).collect();
                        out.push(HubMsg::FetchDone);
                        out
                    }
                    HubMsg::Purge(tokens) => {
                        let mut s = store.lock().unwrap_or_else(PoisonError::into_inner);
                        for t in tokens {
                            let c = *commit(&PurgeToken::from_bytes(t)).as_bytes();
                            s.retain(|(_, commitment), _| *commitment != c);
                        }
                        Vec::new()
                    }
                    HubMsg::Bye => return,
                    _ => Vec::new(),
                };
                for r in &reply {
                    if ws.send(send(r)).await.is_err() {
                        return;
                    }
                }
            }
            b = live.recv() => {
                if let Ok(b) = b
                    && matches(&prefixes, &b.0)
                    && ws.send(send(&batch(&[b]))).await.is_err()
                {
                    return;
                }
            }
        }
    }
}

/// One `bundles` message; "may cross" uploads come back marked foreign (a simulated partner).
fn batch(items: &[(Vec<u8>, bool)]) -> HubMsg {
    HubMsg::Bundles {
        bundles: items.iter().map(|(b, _)| b.clone()).collect(),
        foreign: items
            .iter()
            .enumerate()
            .filter(|(_, (_, cross))| *cross)
            .filter_map(|(i, _)| u8::try_from(i).ok())
            .collect(),
    }
}
