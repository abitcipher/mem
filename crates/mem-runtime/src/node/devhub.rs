//! A device acting as a hub for its owner's contacts (serving), and our sessions with contacts'
//! device hubs (using). Both speak `spec/hub.cddl` over Iroh with ALPN `mem/hub/1`, after a
//! login in which each side proves knowledge of the pair's contact secret, bound to the hub's
//! transport key and both nonces. The core decides who may log in and what to upload; this
//! module moves messages and keeps the hub's bundles in the courier database.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError, Weak};
use std::time::Duration;

use cla_iroh::{HubReader, HubStream, HubWriter, IrohCla};
use mem_core::model::{ContactId, OpId, PeerHandle};
use mem_core::{ClaError, ClaId, Input, Output};
use mem_proto::encounter::{MAX_BUNDLES_PER_MSG, MAX_PURGE_TOKENS};
use mem_proto::hub::{HUB_VERSION, HubLimits, HubMsg, PutError};
use tokio::sync::{broadcast, mpsc};

use super::{Node, Shared, is_wifi};

/// Device hub connections get handles from here: above local-network handles, below Wi-Fi
/// Direct's.
pub(super) const DEVHUB_HANDLE_BASE: u64 = 1 << 61;
/// Each login step must arrive within this long.
const LOGIN_TIMEOUT: Duration = Duration::from_secs(10);
/// Bytes of bundles per `bundles` message, below the frame limit.
const BATCH_BYTES: usize = 900 * 1024;

/// Device hub state shared by the node's tasks.
pub(super) struct DevHub {
    /// Our logged-in sessions with contacts' hubs: what to send on each.
    links: Mutex<HashMap<u64, mpsc::UnboundedSender<HubMsg>>>,
    next: AtomicU64,
    /// Serving: something new was stored (sessions look for matches).
    stored: broadcast::Sender<()>,
    /// Serving: contacts logged in now, by session.
    clients: Mutex<HashMap<u64, ContactId>>,
}

impl Default for DevHub {
    fn default() -> Self {
        Self {
            links: Mutex::new(HashMap::new()),
            next: AtomicU64::new(DEVHUB_HANDLE_BASE),
            stored: broadcast::channel(16).0,
            clients: Mutex::new(HashMap::new()),
        }
    }
}

fn nonce() -> Option<[u8; 16]> {
    let mut n = [0u8; 16];
    getrandom::fill(&mut n).ok()?;
    Some(n)
}

/// Constant-time equality of two proofs.
fn same(a: &[u8; 32], b: &[u8; 32]) -> bool {
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

async fn send(w: &mut HubWriter, msg: &HubMsg) -> bool {
    match msg.encode() {
        Ok(bytes) => w.send(&bytes).await,
        Err(_) => false,
    }
}

/// The next message within the login timeout; `None` on timeout, end or garbage.
async fn next(r: &mut HubReader) -> Option<HubMsg> {
    let bytes = tokio::time::timeout(LOGIN_TIMEOUT, r.recv()).await.ok()??;
    HubMsg::decode(&bytes).ok()
}

/// Runs `f` on the node in the blocking pool; `None` if the node is gone.
async fn on_node<T: Send + 'static>(
    weak: &Weak<Shared>,
    f: impl FnOnce(&Node) -> T + Send + 'static,
) -> Option<T> {
    let weak = weak.clone();
    tokio::task::spawn_blocking(move || weak.upgrade().map(|shared| f(&Node { shared })))
        .await
        .ok()
        .flatten()
}

impl Node {
    fn iroh_for(&self, peer: PeerHandle) -> Option<IrohCla> {
        if is_wifi(peer.0) {
            self.wifi_handle()
        } else {
            self.lan_handle()
        }
    }

    // ------------------------------------------------------------------ serving

    /// A contact opened a hub session on our `cla` endpoint.
    pub(super) async fn serve_hub(weak: Weak<Shared>, stream: HubStream, cla: ClaId) {
        let (mut w, mut r) = stream.split();
        if let Some(contact) = Self::hub_login(&weak, &mut w, &mut r, cla).await {
            let session = Self::client_joined(&weak, contact, None);
            Self::hub_session(&weak, &mut w, &mut r, contact).await;
            Self::client_joined(&weak, contact, Some(session));
        }
        w.close();
    }

    /// Records a contact logged in to our hub (`ended` `None`; returns the session number) or
    /// the end of that session.
    fn client_joined(weak: &Weak<Shared>, contact: ContactId, ended: Option<u64>) -> u64 {
        let Some(shared) = weak.upgrade() else {
            return 0;
        };
        let mut clients = shared
            .devhub
            .clients
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let session = match ended {
            None => {
                let n = shared.devhub.next.fetch_add(1, Ordering::Relaxed);
                clients.insert(n, contact);
                n
            }
            Some(n) => {
                clients.remove(&n);
                n
            }
        };
        drop(clients);
        let _ = shared.events.send(super::NodeEvent::Reachability);
        session
    }

    /// Contacts logged in to this device's hub now.
    #[must_use]
    pub fn device_hub_clients(&self) -> Vec<ContactId> {
        let mut v: Vec<ContactId> = self
            .shared
            .devhub
            .clients
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .copied()
            .collect();
        v.sort();
        v.dedup();
        v
    }

    /// Hub side of the login; the contact on success.
    async fn hub_login(
        weak: &Weak<Shared>,
        w: &mut HubWriter,
        r: &mut HubReader,
        cla: ClaId,
    ) -> Option<ContactId> {
        let Some(HubMsg::Hello { version }) = next(r).await else {
            return None;
        };
        let (settings, local) = on_node(weak, move |n| {
            let iroh = if cla == ClaId::WifiDirect {
                n.wifi_handle()
            } else {
                n.lan_handle()
            };
            (
                n.read(|rt| rt.agent().device_hub_settings()),
                iroh.map(|i| i.id()),
            )
        })
        .await?;
        let local = local?;
        if version != HUB_VERSION || !settings.serve {
            return None;
        }
        let server_nonce = nonce()?;
        let limits = HubLimits {
            max_bundle: 65_536,
            per_tag: u32::try_from(settings.per_contact_bytes).unwrap_or(u32::MAX),
            puts_per_minute: 600,
        };
        if !send(w, &HubMsg::Welcome { version, limits }).await
            || !send(w, &HubMsg::HubChallenge { server_nonce }).await
        {
            return None;
        }
        let Some(HubMsg::HubAuth { client_nonce, mac }) = next(r).await else {
            return None;
        };
        let (contact, answer) = on_node(weak, move |n| {
            n.read(|rt| {
                rt.agent()
                    .device_hub_login(&local, &server_nonce, &client_nonce, &mac)
            })
        })
        .await??;
        send(w, &HubMsg::HubAuthOk { mac: answer })
            .await
            .then_some(contact)
    }

    /// A logged-in contact's session: uploads, fetches with live delivery, purges.
    async fn hub_session(
        weak: &Weak<Shared>,
        w: &mut HubWriter,
        r: &mut HubReader,
        contact: ContactId,
    ) {
        let Some(mut stored) = weak.upgrade().map(|s| s.devhub.stored.subscribe()) else {
            return;
        };
        let mut prefixes: Vec<[u8; 2]> = Vec::new();
        let mut last = 0i64;
        loop {
            tokio::select! {
                bytes = r.recv() => {
                    let Some(Ok(msg)) = bytes.map(|b| HubMsg::decode(&b)) else {
                        return;
                    };
                    match msg {
                        HubMsg::Put { id, bundle, .. } => {
                            let reply = Self::hub_put(weak, contact, id, bundle).await;
                            if !send(w, &reply).await {
                                return;
                            }
                        }
                        HubMsg::Fetch(p) => {
                            prefixes = p;
                            last = 0;
                            if !Self::hub_deliver(weak, w, &prefixes, &mut last).await
                                || !send(w, &HubMsg::FetchDone).await
                            {
                                return;
                            }
                        }
                        HubMsg::Purge(tokens) => {
                            let _ = on_node(weak, move |n| n.lock().hub_purge(&tokens)).await;
                        }
                        _ => return,
                    }
                }
                news = stored.recv() => {
                    if matches!(news, Err(broadcast::error::RecvError::Closed)) {
                        return;
                    }
                    if !prefixes.is_empty()
                        && !Self::hub_deliver(weak, w, &prefixes, &mut last).await
                    {
                        return;
                    }
                }
            }
        }
    }

    async fn hub_put(weak: &Weak<Shared>, contact: ContactId, id: u64, bundle: Vec<u8>) -> HubMsg {
        let took = on_node(weak, move |n| {
            let took = n.lock().hub_take(contact, &bundle);
            if matches!(took, Ok(Ok(Some(_)))) {
                let _ = n.shared.devhub.stored.send(());
            }
            took
        })
        .await;
        match took {
            Some(Ok(Ok(_))) => HubMsg::PutOk { id },
            Some(Ok(Err(reason))) => HubMsg::PutErr { id, reason },
            Some(Err(_)) | None => HubMsg::PutErr {
                id,
                reason: PutError::Quota,
            },
        }
    }

    /// Sends what is held for `prefixes` after `last`, in bounded batches; advances `last`.
    async fn hub_deliver(
        weak: &Weak<Shared>,
        w: &mut HubWriter,
        prefixes: &[[u8; 2]],
        last: &mut i64,
    ) -> bool {
        let (p, after) = (prefixes.to_vec(), *last);
        let Some(Ok(found)) = on_node(weak, move |n| n.read(|rt| rt.hub_fetch(&p, after))).await
        else {
            return true;
        };
        let mut batch: Vec<Vec<u8>> = Vec::new();
        let mut size = 0;
        for (seq, bytes) in found {
            if !batch.is_empty()
                && (batch.len() == MAX_BUNDLES_PER_MSG || size + bytes.len() > BATCH_BYTES)
            {
                let bundles = std::mem::take(&mut batch);
                size = 0;
                if !send(
                    w,
                    &HubMsg::Bundles {
                        bundles,
                        foreign: Vec::new(),
                    },
                )
                .await
                {
                    return false;
                }
            }
            size += bytes.len();
            batch.push(bytes);
            *last = seq;
        }
        batch.is_empty()
            || send(
                w,
                &HubMsg::Bundles {
                    bundles: batch,
                    foreign: Vec::new(),
                },
            )
            .await
    }

    // ------------------------------------------------------------------- using

    /// Dials the hub of `contact`'s device behind encounter `peer`.
    pub(super) fn want_device_hub(&self, peer: PeerHandle, contact: ContactId) {
        let Some(iroh) = self.iroh_for(peer) else {
            return;
        };
        let Some(hub_id) = iroh.remote_id(cla_iroh::PeerHandle(peer.0)) else {
            return;
        };
        let weak = std::sync::Arc::downgrade(&self.shared);
        self.spawn(async move {
            let Some(stream) = iroh.dial_hub(cla_iroh::PeerHandle(peer.0)).await else {
                return;
            };
            Self::use_hub(weak, stream, hub_id, contact).await;
        });
    }

    async fn use_hub(weak: Weak<Shared>, stream: HubStream, hub_id: [u8; 32], contact: ContactId) {
        let (mut w, mut r) = stream.split();
        if !Self::client_login(&weak, &mut w, &mut r, hub_id, contact).await {
            w.close();
            return;
        }
        let Some(shared) = weak.upgrade() else {
            return;
        };
        let link = shared.devhub.next.fetch_add(1, Ordering::Relaxed);
        let (tx, mut rx) = mpsc::unbounded_channel::<HubMsg>();
        shared
            .devhub
            .links
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(link, tx);
        drop(shared);
        let peer = PeerHandle(link);
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                let bye = msg == HubMsg::Bye;
                if !send(&mut w, &msg).await || bye {
                    break;
                }
            }
            w.close();
        });
        let _ = on_node(&weak, move |n| {
            n.handle(Input::DeviceHubUp { peer, contact })
        })
        .await;
        while let Some(bytes) = r.recv().await {
            let input = match HubMsg::decode(&bytes) {
                Ok(HubMsg::Bundles { bundles, .. }) => Input::DeviceHubBundles { peer, bundles },
                Ok(HubMsg::FetchDone) => Input::DeviceHubFetchDone { peer },
                Ok(HubMsg::PutOk { id }) => Input::TransmitResult {
                    op: OpId(id),
                    result: Ok(()),
                },
                Ok(HubMsg::PutErr { id, .. }) => Input::TransmitResult {
                    op: OpId(id),
                    result: Err(ClaError::Failed),
                },
                Ok(_) => continue,
                Err(_) => break,
            };
            let _ = on_node(&weak, move |n| n.handle(input)).await;
        }
        let _ = on_node(&weak, move |n| {
            n.shared
                .devhub
                .links
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(&link);
            n.handle(Input::DeviceHubDown { peer })
        })
        .await;
    }

    /// Client side of the login: nothing is uploaded or fetched before the hub's proof checks.
    async fn client_login(
        weak: &Weak<Shared>,
        w: &mut HubWriter,
        r: &mut HubReader,
        hub_id: [u8; 32],
        contact: ContactId,
    ) -> bool {
        if !send(
            w,
            &HubMsg::Hello {
                version: HUB_VERSION,
            },
        )
        .await
        {
            return false;
        }
        let Some(HubMsg::Welcome { version, .. }) = next(r).await else {
            return false;
        };
        let Some(HubMsg::HubChallenge { server_nonce }) = next(r).await else {
            return false;
        };
        let Some(client_nonce) = nonce() else {
            return false;
        };
        let proofs = on_node(weak, move |n| {
            n.read(|rt| {
                rt.agent()
                    .device_hub_proofs(contact, &hub_id, &server_nonce, &client_nonce)
            })
        })
        .await
        .flatten();
        let Some(proofs) = proofs else {
            return false;
        };
        if version != HUB_VERSION
            || !send(
                w,
                &HubMsg::HubAuth {
                    client_nonce,
                    mac: proofs.client,
                },
            )
            .await
        {
            return false;
        }
        matches!(next(r).await, Some(HubMsg::HubAuthOk { mac }) if same(&mac, &proofs.hub))
    }

    fn link_send(&self, peer: PeerHandle, msg: HubMsg) -> bool {
        self.shared
            .devhub
            .links
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(&peer.0)
            .is_some_and(|tx| tx.send(msg).is_ok())
    }

    /// Carries out the core's device hub outputs.
    pub(super) fn dispatch_devhub(&self, o: &Output) {
        match o {
            Output::Transmit {
                op,
                peer: Some(peer),
                bytes,
                ..
            } => {
                let put = HubMsg::Put {
                    id: op.0,
                    bundle: bytes.clone(),
                    may_cross: false,
                };
                // Reported done on the hub's `put_ok`.
                if !self.link_send(*peer, put) {
                    self.fail_later(*op);
                }
            }
            Output::DeviceHubFetch { peer, prefixes } => {
                self.link_send(*peer, HubMsg::Fetch(prefixes.clone()));
            }
            Output::DeviceHubPurge { peer, tokens } => {
                for chunk in tokens.chunks(MAX_PURGE_TOKENS) {
                    self.link_send(*peer, HubMsg::Purge(chunk.to_vec()));
                }
            }
            Output::CloseDeviceHub { peer } => {
                self.link_send(*peer, HubMsg::Bye);
            }
            Output::WantDeviceHub { peer, contact } => self.want_device_hub(*peer, *contact),
            _ => {}
        }
    }
}
