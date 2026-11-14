//! The running node: agent + store + network channels + clock.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError, Weak};
use std::time::Duration;

use cla_iroh::{IrohCla, IrohConfig, IrohEvent};
use mem_core::model::{ContactId, MsgId, PeerHandle};
use mem_core::{ClaError, ClaId, EncounterMsg, Input, Output};
use tokio::sync::{broadcast, mpsc, oneshot};

use crate::{Incoming, KeyGuard, RestoreSummary, Runtime, RuntimeError, TextInput, text_input};
use mem_proto::encounter::BundleEntry;

mod devhub;

/// How often the clock ticks while a peer is connected: the ack delay and retransmission timers
/// need about one-second resolution.
const TICK_CONNECTED: Duration = Duration::from_secs(1);
/// How often it ticks otherwise (tag epochs roll over at midnight UTC; outbox expiry).
const TICK_IDLE_EVERY: u32 = 30;

/// Something the UI may want to refresh for. Carries ids only, never content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeEvent {
    /// A message from this contact arrived over a network channel.
    MessageReceived(ContactId),
    /// A message's status changed (e.g. handed to a nearby peer).
    MessageStatus(MsgId),
    /// Which contacts are reachable nearby changed.
    Reachability,
    /// The contact list changed.
    Contacts,
    /// A contact request arrived.
    Requests,
    /// A group changed or an invitation arrived.
    Groups,
    /// A group message arrived.
    GroupMessage(mem_core::model::GroupId),
    /// Our main device retired this device.
    DeviceRetired,
    /// Call setup from this contact is waiting: [`Node::take_call_signals`].
    Call(ContactId),
    /// Shares we offer, or shares offered to us, changed.
    Shares,
    /// A contact asked for a shared file: [`Node::take_share_requests`].
    ShareWanted,
    /// Something went to or came from this contact over this channel (a message, an ack, a
    /// file part). For a live view only; no content, no size.
    Traffic {
        /// The contact.
        contact: ContactId,
        /// The channel.
        cla: ClaId,
        /// Sent by us.
        outgoing: bool,
        /// A message or file shown in a chat (not setup traffic).
        visible: bool,
    },
}

/// A contact may download one of our shared files: the app reads it and serves it.
#[derive(Clone, PartialEq, Eq)]
pub struct ShareRequest {
    /// Who asked.
    pub contact: ContactId,
    /// The share.
    pub share: mem_core::model::ShareId,
    /// The item.
    pub item: mem_core::model::ShareItemId,
    /// Where the file is, as the app gave it.
    pub handle: String,
}

impl std::fmt::Debug for ShareRequest {
    /// Never prints where the file is.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShareRequest").finish_non_exhaustive()
    }
}

/// Share requests kept for the app; the rate limit in the core keeps this small.
const SHARE_INBOX: usize = 64;

/// A call setup message from a contact, as it arrived.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingCallSignal {
    /// Who sent it.
    pub contact: ContactId,
    /// The message.
    pub signal: mem_core::CallSignal,
    /// The channel it came over, and its grade.
    pub via: mem_core::model::ChannelUse,
}

/// Call setup messages kept for the app; older ones are dropped beyond this (a call needs only
/// the latest few).
const CALL_INBOX: usize = 64;

/// A command for the Bluetooth LE radio plugin. Link ids are the plugin's own.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BleCommand {
    /// Write these stream bytes (one or more whole frames) to the link, in order, then report
    /// with [`Node::ble_sent`] and the same `op`.
    Send {
        /// Plugin link id.
        link: u64,
        /// Write id for the result.
        op: u64,
        /// Stream bytes.
        bytes: Vec<u8>,
    },
    /// Close the link. The plugin still reports it down with [`Node::ble_link_down`].
    Close {
        /// Plugin link id.
        link: u64,
    },
}

type BleSink = Box<dyn FnMut(BleCommand) -> bool + Send>;

/// The Bluetooth side: open links and the way to the plugin.
#[derive(Default)]
struct Ble {
    links: cla_ble::Links,
    sink: Option<BleSink>,
    /// Writes not yet reported by the plugin: op → (link, result).
    pending: HashMap<u64, (u64, oneshot::Sender<bool>)>,
    next_op: u64,
}

impl Ble {
    /// Hands a command to the plugin; `false` if nobody listens.
    fn command(&mut self, c: BleCommand) -> bool {
        let ok = self.sink.as_mut().is_some_and(|f| f(c));
        if !ok {
            self.sink = None;
        }
        ok
    }

    /// Forgets a link: its unreported writes failed.
    fn close(&mut self, link: u64) {
        self.links.close(link);
        let ops: Vec<u64> = self
            .pending
            .iter()
            .filter(|(_, (l, _))| *l == link)
            .map(|(op, _)| *op)
            .collect();
        for op in ops {
            if let Some((_, done)) = self.pending.remove(&op) {
                let _ = done.send(false);
            }
        }
    }
}

/// Node options.
#[derive(Debug, Clone, Default)]
pub struct NodeConfig {
    /// The local-network channel.
    pub lan: IrohConfig,
    /// Operator keys that may sign hub lists; [`OPERATOR_KEYS`] in the app.
    pub operator_keys: Vec<[u8; 32]>,
    /// A signed hub list built into the app, used until a newer one arrives.
    pub bootstrap_hub_list: Option<Vec<u8>>,
    /// The platform key store that holds the state key; `None` on desktop.
    pub key_guard: Option<Arc<dyn KeyGuard>>,
}

/// Wi-Fi Direct peer handles start here: above every local-network handle, below
/// Bluetooth's top-bit handles.
const WIFI_HANDLE_BASE: u64 = 1 << 62;

const fn is_wifi(handle: u64) -> bool {
    handle >= WIFI_HANDLE_BASE && handle < (1 << 63)
}

/// The operator keys the app trusts for hub lists. Empty until the operator
/// generates the production key with `mem-hublist keygen`; until then users enter hub addresses.
pub const OPERATOR_KEYS: &[[u8; 32]] = &[];

struct Shared {
    rt: Mutex<Runtime>,
    /// `Some` until drop. Background tasks hold only `Weak<Shared>`, so dropping the last
    /// [`Node`] really stops everything.
    tokio: Option<tokio::runtime::Runtime>,
    lan: Mutex<Option<IrohCla>>,
    /// Iroh over Wi-Fi Direct links: its own endpoint, no mDNS.
    wifi: Mutex<Option<IrohCla>>,
    ble: Mutex<Ble>,
    hub: Mutex<Option<cla_hub::HubClient>>,
    /// The hubs the running client was started with.
    hub_urls: Mutex<Vec<String>>,
    email: Mutex<Option<cla_email::EmailClient>>,
    events: broadcast::Sender<NodeEvent>,
    /// Call setup waiting for the app (events carry ids only, never content).
    calls: Mutex<std::collections::VecDeque<IncomingCallSignal>>,
    /// Shared files asked for, waiting for the app to read them.
    share_wants: Mutex<std::collections::VecDeque<ShareRequest>>,
    /// Device hub sessions, both as a hub and with contacts' hubs.
    devhub: devhub::DevHub,
    config: NodeConfig,
}

/// A [`Runtime`] plus the asynchronous parts: the Iroh channel and a clock. Cheap to clone;
/// all methods are safe to call from any non-async thread.
#[derive(Clone)]
pub struct Node {
    shared: Arc<Shared>,
}

impl std::fmt::Debug for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Node").finish_non_exhaustive()
    }
}

impl Node {
    /// Opens the profile in `dir` and starts the clock and, unless the user turned it off,
    /// the local-network channel.
    ///
    /// # Errors
    ///
    /// [`RuntimeError`] if storage fails or the async runtime cannot start.
    pub fn open(dir: impl AsRef<Path>, config: NodeConfig) -> Result<Self, RuntimeError> {
        let rt = Runtime::open_guarded(dir, config.key_guard.as_deref())?;
        let tokio = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("mem-net")
            .enable_all()
            .build()
            .map_err(|_| RuntimeError::Async)?;
        let lan_enabled = rt.agent().channel_enabled(ClaId::Iroh);
        let node = Self {
            shared: Arc::new(Shared {
                rt: Mutex::new(rt),
                tokio: Some(tokio),
                lan: Mutex::new(None),
                wifi: Mutex::new(None),
                ble: Mutex::new(Ble::default()),
                hub: Mutex::new(None),
                hub_urls: Mutex::new(Vec::new()),
                email: Mutex::new(None),
                events: broadcast::channel(64).0,
                calls: Mutex::new(std::collections::VecDeque::new()),
                share_wants: Mutex::new(std::collections::VecDeque::new()),
                devhub: devhub::DevHub::default(),
                config,
            }),
        };
        node.spawn_clock();
        if lan_enabled {
            node.start_lan();
        }
        node.sync_wifi();
        let keys = node.shared.config.operator_keys.clone();
        let _ = node.handle(Input::SetOperatorKeys(keys));
        if let Some(bytes) = node.shared.config.bootstrap_hub_list.clone() {
            let _ = node.handle(Input::HubListReceived { bytes });
        }
        node.sync_hub();
        node.sync_email();
        Ok(node)
    }

    /// Read access to the runtime (queries only).
    pub fn read<T>(&self, f: impl FnOnce(&Runtime) -> T) -> T {
        f(&self.lock())
    }

    /// Handles `input`, persists, and carries out network outputs and events.
    ///
    /// # Errors
    ///
    /// See [`Runtime::handle`].
    pub fn handle(&self, input: Input) -> Result<Vec<Output>, RuntimeError> {
        let out = self.lock().handle(input)?;
        self.dispatch(&out);
        Ok(out)
    }

    /// Feeds text pasted or scanned by the user, received over `cla`.
    ///
    /// # Errors
    ///
    /// See [`Runtime::handle`].
    pub fn receive_text(&self, text: &str, cla: ClaId) -> Result<Incoming, RuntimeError> {
        match text_input(text, cla) {
            Some(TextInput::Bundle(input)) => Ok(Incoming::Bundle(self.handle(input)?)),
            Some(TextInput::Card(input)) => Ok(Incoming::Card(self.handle(input)?)),
            None => Ok(Incoming::NotMem),
        }
    }

    /// Turns the local-network channel on or off; the choice is persisted.
    ///
    /// # Errors
    ///
    /// See [`Runtime::handle`].
    pub fn set_lan_enabled(&self, enabled: bool) -> Result<(), RuntimeError> {
        self.handle(Input::SetChannelEnabled {
            cla: ClaId::Iroh,
            enabled,
        })?;
        if enabled {
            self.start_lan();
        } else {
            self.stop_lan();
        }
        Ok(())
    }

    /// Turns Wi-Fi Direct on or off; the choice is persisted. The radio itself is
    /// driven by the app's plugin; this runs the endpoint the links connect to.
    ///
    /// # Errors
    ///
    /// See [`Runtime::handle`].
    pub fn set_wifi_direct_enabled(&self, enabled: bool) -> Result<(), RuntimeError> {
        self.handle(Input::SetChannelEnabled {
            cla: ClaId::WifiDirect,
            enabled,
        })?;
        self.sync_wifi();
        Ok(())
    }

    /// This device's Wi-Fi Direct endpoint id and UDP port, for the plugin's advertisement, while
    /// the channel is on.
    #[must_use]
    pub fn wifi_local(&self) -> Option<([u8; 32], u16)> {
        let (id, port) = self.wifi_handle()?.local();
        Some((id, port?))
    }

    /// The plugin joined a Wi-Fi Direct group: dial the peer's endpoint `id` at `addr` (the
    /// group owner's address and its advertised port).
    pub fn wifi_peer(&self, id: [u8; 32], addr: std::net::SocketAddr) {
        if let (Some(w), Some(tokio)) = (self.wifi_handle(), &self.shared.tokio) {
            let _guard = tokio.enter();
            w.connect_direct(id, addr);
        }
    }

    /// Open Wi-Fi Direct encounters (the plugin ends an idle group).
    #[must_use]
    pub fn wifi_connections(&self) -> usize {
        self.wifi_handle().map_or(0, |w| w.connections())
    }

    fn wifi_handle(&self) -> Option<IrohCla> {
        self.shared
            .wifi
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn sync_wifi(&self) {
        let on = self.read(|rt| rt.agent().channel_enabled(ClaId::WifiDirect));
        let running = self.wifi_handle().is_some();
        if on && !running {
            let Some(tokio) = &self.shared.tokio else {
                return;
            };
            let config = IrohConfig {
                mdns: false,
                service_name: String::new(),
                handle_base: WIFI_HANDLE_BASE,
            };
            let Ok((wifi, events)) = tokio.block_on(IrohCla::start(config)) else {
                tracing::warn!("Wi-Fi Direct endpoint failed to start");
                return;
            };
            *self
                .shared
                .wifi
                .lock()
                .unwrap_or_else(PoisonError::into_inner) = Some(wifi);
            self.spawn(Self::lan_loop(
                Arc::downgrade(&self.shared),
                events,
                ClaId::WifiDirect,
            ));
        } else if !on && running {
            let wifi = self
                .shared
                .wifi
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take();
            if let Some(w) = wifi {
                self.spawn(async move { w.shutdown().await });
            }
        }
    }

    fn stop_lan(&self) {
        let lan = self
            .shared
            .lan
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(lan) = lan {
            self.spawn(async move { lan.shutdown().await });
        }
    }

    /// An encrypted backup of this profile. Slow: Argon2id.
    ///
    /// # Errors
    ///
    /// See [`Runtime::make_backup`].
    pub fn make_backup(&self, history: bool) -> Result<Vec<u8>, RuntimeError> {
        self.read(|rt| rt.make_backup(history))
    }

    /// Restores a backup into this profile (a move: same identity). The local
    /// network restarts so peers see the restored identity's tags.
    ///
    /// # Errors
    ///
    /// See [`Runtime::restore_backup`].
    pub fn restore_backup(
        &self,
        data: &[u8],
        phrase: &str,
    ) -> Result<RestoreSummary, RuntimeError> {
        self.stop_lan();
        self.end_ble_links();
        let summary = self.lock().restore_backup(data, phrase);
        if self.read(|rt| rt.agent().channel_enabled(ClaId::Iroh)) {
            self.start_lan();
        }
        self.emit(NodeEvent::Contacts);
        summary
    }

    /// A new device's join request.
    ///
    /// # Errors
    ///
    /// See [`Runtime::device_join`].
    pub fn device_join(&self) -> Result<mem_core::DeviceJoin, RuntimeError> {
        self.read(Runtime::device_join)
    }

    /// Starts an in-person attestation.
    ///
    /// # Errors
    ///
    /// See [`Runtime::start_attestation`].
    pub fn start_attestation(
        &self,
        request_text: &str,
    ) -> Result<mem_core::attest::AttestSession, RuntimeError> {
        self.read(|rt| rt.start_attestation(request_text))
    }

    /// Becomes another device of the identity whose main device showed `setup`; channels
    /// restart with the new identity's tags.
    ///
    /// # Errors
    ///
    /// See [`Runtime::join_device`].
    pub fn join_device(
        &self,
        join: &mem_core::DeviceJoin,
        setup: &[u8],
    ) -> Result<(), RuntimeError> {
        self.stop_lan();
        self.end_ble_links();
        let result = self.lock().join_device(join, setup);
        if self.read(|rt| rt.agent().channel_enabled(ClaId::Iroh)) {
            self.start_lan();
        }
        self.sync_hub();
        self.emit(NodeEvent::Contacts);
        result
    }

    /// Replaces this retired device's state with a fresh identity.
    ///
    /// # Errors
    ///
    /// See [`Runtime::forget_this_device`].
    pub fn forget_this_device(&self) -> Result<(), RuntimeError> {
        self.stop_lan();
        self.end_ble_links();
        self.stop_hub();
        let result = self.lock().forget_this_device();
        if self.read(|rt| rt.agent().channel_enabled(ClaId::Iroh)) {
            self.start_lan();
        }
        self.emit(NodeEvent::Contacts);
        result
    }

    /// Turns the hub channel on or off; the choice is persisted.
    ///
    /// # Errors
    ///
    /// See [`Runtime::handle`].
    pub fn set_hub_enabled(&self, enabled: bool) -> Result<(), RuntimeError> {
        self.handle(Input::SetChannelEnabled {
            cla: ClaId::Hub,
            enabled,
        })?;
        self.sync_hub();
        Ok(())
    }

    /// Sets the hub's address (validated by the agent) and reconnects.
    ///
    /// # Errors
    ///
    /// See [`Runtime::handle`].
    pub fn set_hub_url(&self, url: Option<String>) -> Result<Vec<Output>, RuntimeError> {
        let out = self.handle(Input::SetHubUrl(url))?;
        self.sync_hub();
        Ok(out)
    }

    /// Turns the email bridge on or off (persisted).
    ///
    /// # Errors
    ///
    /// See [`Runtime::handle`].
    pub fn set_email_enabled(&self, enabled: bool) -> Result<(), RuntimeError> {
        self.handle(Input::SetChannelEnabled {
            cla: ClaId::Email,
            enabled,
        })?;
        self.stop_email();
        self.sync_email();
        Ok(())
    }

    /// Sets (or removes) the mail account and reconnects.
    ///
    /// # Errors
    ///
    /// See [`Runtime::handle`].
    pub fn set_email_account(
        &self,
        account: Option<mem_core::EmailAccount>,
    ) -> Result<Vec<Output>, RuntimeError> {
        let out = self.handle(Input::SetEmailAccount(account))?;
        self.stop_email();
        self.sync_email();
        Ok(out)
    }

    /// Checks the mailbox now (e.g. after a wake-up).
    pub fn check_email(&self) {
        if let Some(c) = self.email_client() {
            c.check_now();
        }
    }

    fn sync_email(&self) {
        let (on, account) = self.read(|rt| {
            (
                rt.agent().channel_enabled(ClaId::Email),
                rt.agent().email_account(),
            )
        });
        let running = self.email_client().is_some();
        match (on, account) {
            (true, Some(a)) if !running => self.start_email(a),
            (true, Some(_)) => {}
            _ => self.stop_email(),
        }
    }

    fn start_email(&self, a: mem_core::EmailAccount) {
        let Some(tokio) = &self.shared.tokio else {
            return;
        };
        let config = cla_email::EmailConfig {
            address: a.address,
            imap_host: a.imap_host,
            imap_port: a.imap_port,
            smtp_host: a.smtp_host,
            smtp_port: a.smtp_port,
            username: a.username,
            password: a.password,
        };
        let (client, events) = {
            let _guard = tokio.enter();
            cla_email::EmailClient::start(config)
        };
        *self
            .shared
            .email
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(client);
        self.spawn(Self::email_loop(Arc::downgrade(&self.shared), events));
    }

    fn stop_email(&self) {
        let client = self
            .shared
            .email
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(c) = client {
            c.shutdown();
            let _ = self.handle(Input::EmailDisconnected);
        }
    }

    fn email_client(&self) -> Option<cla_email::EmailClient> {
        self.shared
            .email
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    async fn email_loop(
        weak: Weak<Shared>,
        mut events: mpsc::UnboundedReceiver<cla_email::EmailEvent>,
    ) {
        use cla_email::EmailEvent;
        while let Some(event) = events.recv().await {
            let input = match event {
                EmailEvent::Connected => Input::EmailConnected,
                EmailEvent::Disconnected => Input::EmailDisconnected,
                EmailEvent::Bundle(bytes) => Input::Received {
                    cla: ClaId::Email,
                    bytes,
                },
                EmailEvent::SendResult { id, ok } => Input::TransmitResult {
                    op: mem_core::model::OpId(id),
                    result: if ok { Ok(()) } else { Err(ClaError::Failed) },
                },
            };
            Self::with_node(weak.clone(), move |node| {
                let _ = node.handle(input);
            })
            .await;
        }
    }

    /// Sets (or clears) the push endpoint the hub wakes us at.
    ///
    /// # Errors
    ///
    /// See [`Runtime::handle`].
    pub fn set_push_endpoint(&self, endpoint: Option<String>) -> Result<Vec<Output>, RuntimeError> {
        self.handle(Input::SetPushEndpoint(endpoint))
    }

    /// Whether the hub channel is connected now.
    #[must_use]
    pub fn hub_online(&self) -> bool {
        self.read(|rt| rt.agent().hub_online())
    }

    /// Connects to the hub if the channel is on and an address is set; disconnects otherwise.
    /// Connects to the hubs the agent names (a manual address, or the signed list), restarting
    /// the client when that set changes; disconnects when the channel is off or there is none.
    fn sync_hub(&self) {
        let (on, urls) = self.read(|rt| {
            (
                rt.agent().channel_enabled(ClaId::Hub),
                rt.agent().hub_candidates(),
            )
        });
        let current = self
            .shared
            .hub_urls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        let running = self.hub_client().is_some();
        if !on || urls.is_empty() {
            self.stop_hub();
        } else if !running || current != urls {
            self.stop_hub();
            self.start_hub(urls);
        }
    }

    fn start_hub(&self, urls: Vec<String>) {
        let Some(tokio) = &self.shared.tokio else {
            return;
        };
        self.shared
            .hub_urls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone_from(&urls);
        let (client, events) = {
            let _guard = tokio.enter();
            cla_hub::HubClient::start_many(urls)
        };
        *self
            .shared
            .hub
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(client);
        self.spawn(Self::hub_loop(Arc::downgrade(&self.shared), events));
    }

    fn stop_hub(&self) {
        let client = self
            .shared
            .hub
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        if let Some(c) = client {
            c.shutdown();
            let _ = self.handle(Input::HubDisconnected);
        }
    }

    fn hub_client(&self) -> Option<cla_hub::HubClient> {
        self.shared
            .hub
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    async fn hub_loop(weak: Weak<Shared>, mut events: mpsc::UnboundedReceiver<cla_hub::HubEvent>) {
        use cla_hub::HubEvent;
        while let Some(event) = events.recv().await {
            let input = match event {
                HubEvent::Connected(_) => Input::HubConnected,
                HubEvent::Disconnected => Input::HubDisconnected,
                HubEvent::FetchDone => Input::HubFetchDone,
                HubEvent::Bundles(bundles) => Input::HubBundles { bundles },
                HubEvent::HubList(bytes) => Input::HubListReceived { bytes },
                HubEvent::PutResult { id, ok } => Input::TransmitResult {
                    op: mem_core::model::OpId(id),
                    result: if ok { Ok(()) } else { Err(ClaError::Failed) },
                },
            };
            Self::with_node(weak.clone(), move |node| {
                let _ = node.handle(input);
            })
            .await;
        }
    }

    /// Turns Bluetooth LE on or off; the choice is persisted. Turning it off ends every BLE
    /// encounter; the app starts and stops the radio plugin itself (it needs permissions).
    ///
    /// # Errors
    ///
    /// See [`Runtime::handle`].
    pub fn set_ble_enabled(&self, enabled: bool) -> Result<(), RuntimeError> {
        self.handle(Input::SetChannelEnabled {
            cla: ClaId::Ble,
            enabled,
        })?;
        if !enabled {
            self.end_ble_links();
        }
        Ok(())
    }

    /// Where commands for the radio plugin go, until `f` returns `false`. Replaces an earlier
    /// sink; links opened under it are ended (a new sink means a restarted plugin).
    pub fn on_ble_command(&self, f: impl FnMut(BleCommand) -> bool + Send + 'static) {
        self.end_ble_links();
        self.ble().sink = Some(Box::new(f));
    }

    /// The plugin connected a link (either role). Starts an encounter, or closes the link if
    /// it cannot be used (BLE off, too many links).
    pub fn ble_link_up(&self, link: u64) {
        let peer = self.ble().links.open(link);
        let Some(peer) = peer else {
            self.ble().command(BleCommand::Close { link });
            return;
        };
        let _ = self.handle(Input::EncounterStarted {
            peer: PeerHandle(peer),
            cla: ClaId::Ble,
        });
    }

    /// Bytes arrived on a link, in stream order. A broken stream closes the link.
    pub fn ble_received(&self, link: u64, bytes: &[u8]) {
        let received = self.ble().links.receive(link, bytes);
        let Some(peer) = cla_ble::Links::peer(link).map(PeerHandle) else {
            return;
        };
        match received {
            cla_ble::Received::Messages(msgs) => {
                for msg in msgs {
                    let _ = self.handle(Input::EncounterMessage { peer, msg });
                }
            }
            cla_ble::Received::Broken(_) => {
                let mut ble = self.ble();
                ble.close(link);
                ble.command(BleCommand::Close { link });
                drop(ble);
                let _ = self.handle(Input::EncounterEnded { peer });
            }
            cla_ble::Received::Unknown => {}
        }
    }

    /// The link is gone (lost, or closed by either side).
    pub fn ble_link_down(&self, link: u64) {
        self.ble().close(link);
        if let Some(peer) = cla_ble::Links::peer(link) {
            let _ = self.handle(Input::EncounterEnded {
                peer: PeerHandle(peer),
            });
        }
    }

    /// The plugin finished a [`BleCommand::Send`]: `ok` if every byte went to the radio.
    pub fn ble_sent(&self, op: u64, ok: bool) {
        if let Some((_, done)) = self.ble().pending.remove(&op) {
            let _ = done.send(ok);
        }
    }

    /// The radio stopped (turned off, permission withdrawn): every BLE encounter is over.
    pub fn ble_stopped(&self) {
        self.end_ble_links();
    }

    fn end_ble_links(&self) {
        let peers = {
            let mut ble = self.ble();
            let peers = ble.links.clear();
            for p in &peers {
                if let Some(link) = cla_ble::Links::link(*p) {
                    ble.command(BleCommand::Close { link });
                }
            }
            for (_, (_, done)) in ble.pending.drain() {
                let _ = done.send(false);
            }
            peers
        };
        for p in peers {
            let _ = self.handle(Input::EncounterEnded {
                peer: PeerHandle(p),
            });
        }
    }

    fn ble(&self) -> std::sync::MutexGuard<'_, Ble> {
        self.shared
            .ble
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    /// Sends one encounter message to a peer over whichever channel it is on. The receiver, if
    /// any, tells when the message was handed to the transport (never "delivered").
    fn send_to(
        &self,
        lan: Option<&IrohCla>,
        peer: PeerHandle,
        msg: &EncounterMsg,
    ) -> Option<oneshot::Receiver<bool>> {
        match cla_ble::Links::link(peer.0) {
            Some(link) => {
                let bytes = cla_ble::frame(msg)?;
                let mut ble = self.ble();
                ble.next_op = ble.next_op.wrapping_add(1);
                let op = ble.next_op;
                let (tx, rx) = oneshot::channel();
                ble.pending.insert(op, (link, tx));
                if ble.command(BleCommand::Send { link, op, bytes }) {
                    Some(rx)
                } else {
                    ble.pending.remove(&op);
                    None
                }
            }
            None if is_wifi(peer.0) => self
                .wifi_handle()?
                .send(cla_iroh::PeerHandle(peer.0), msg)
                .ok(),
            None => lan?.send(cla_iroh::PeerHandle(peer.0), msg).ok(),
        }
    }

    /// Whether the local-network channel is running.
    #[must_use]
    pub fn lan_running(&self) -> bool {
        self.shared
            .lan
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .is_some()
    }

    /// Subscribes to [`NodeEvent`]s.
    #[must_use]
    pub fn subscribe(&self) -> broadcast::Receiver<NodeEvent> {
        self.shared.events.subscribe()
    }

    /// Calls `f` for every [`NodeEvent`] until `f` returns `false` or the node is dropped.
    pub fn on_event(&self, mut f: impl FnMut(NodeEvent) -> bool + Send + 'static) {
        let mut rx = self.subscribe();
        self.spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(e) => {
                        if !f(e) {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        // Coalesced: the UI re-queries anyway.
                        if !f(NodeEvent::Reachability) {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    /// This node's connect hint: its current endpoint id and up to two local IPv4
    /// addresses. `None` while the local network channel is off or there is no such address.
    #[must_use]
    pub fn connect_hint(&self) -> Option<mem_proto::hint::ConnectHint> {
        let lan = self.lan_handle()?;
        let addrs: Vec<mem_proto::hint::HintAddr> = lan
            .lan_ipv4(mem_proto::hint::MAX_HINT_ADDRS)
            .into_iter()
            .map(|a| mem_proto::hint::HintAddr::new(a.ip().octets(), a.port()))
            .collect();
        if addrs.is_empty() {
            return None;
        }
        Some(mem_proto::hint::ConnectHint {
            endpoint_id: lan.local().0,
            addrs,
        })
    }

    /// Dials the phone a connect hint names (heard or scanned). The hint is untrusted:
    /// at worst this reaches a stranger, who is an untrusted encounter peer like any other.
    /// Returns `false` while the local network channel is off.
    #[must_use]
    pub fn dial_hint(&self, hint: &mem_proto::hint::ConnectHint) -> bool {
        let (Some(lan), Some(tokio)) = (self.lan_handle(), &self.shared.tokio) else {
            return false;
        };
        let addrs: Vec<std::net::SocketAddr> = hint
            .addrs
            .iter()
            .map(|a| std::net::SocketAddr::from((a.ip(), a.port())))
            .collect();
        let _guard = tokio.enter();
        lan.connect_at(hint.endpoint_id, &addrs);
        true
    }

    /// Test hook: the local-network address of this node.
    #[doc(hidden)]
    #[must_use]
    pub fn lan_addr(&self) -> Option<iroh::EndpointAddr> {
        self.lan_handle().map(|l| l.addr())
    }

    /// Test hook: dial a node directly (mDNS does this in the app).
    #[doc(hidden)]
    pub fn lan_connect(&self, addr: iroh::EndpointAddr) {
        if let (Some(lan), Some(tokio)) = (self.lan_handle(), &self.shared.tokio) {
            let _guard = tokio.enter();
            lan.connect(addr);
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Runtime> {
        self.shared
            .rt
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    fn lan_handle(&self) -> Option<IrohCla> {
        self.shared
            .lan
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn spawn<F>(&self, f: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        if let Some(tokio) = &self.shared.tokio {
            tokio.spawn(f);
        }
    }

    /// Runs `f` on a fresh [`Node`] handle in the blocking pool, if the node still exists.
    async fn with_node(weak: Weak<Shared>, f: impl FnOnce(&Self) + Send + 'static) {
        let _ = tokio::task::spawn_blocking(move || {
            if let Some(shared) = weak.upgrade() {
                f(&Self { shared });
            }
        })
        .await;
    }

    fn spawn_clock(&self) {
        let weak = Arc::downgrade(&self.shared);
        self.spawn(async move {
            let mut tick = tokio::time::interval(TICK_CONNECTED);
            let idle = Arc::new(std::sync::atomic::AtomicU32::new(0));
            loop {
                tick.tick().await;
                if weak.strong_count() == 0 {
                    break;
                }
                let idle = Arc::clone(&idle);
                Self::with_node(weak.clone(), move |node| {
                    use std::sync::atomic::Ordering;
                    // Ack and retransmission timers need one-second resolution whenever
                    // something can carry them: a nearby peer or the hub.
                    let connected = {
                        let rt = node.lock();
                        rt.agent().peer_count() > 0
                            || rt.agent().hub_online()
                            || rt.agent().email_online()
                    };
                    let waited = if connected {
                        0
                    } else {
                        idle.fetch_add(1, Ordering::Relaxed) + 1
                    };
                    if connected || waited >= TICK_IDLE_EVERY {
                        idle.store(0, Ordering::Relaxed);
                        let out = node.lock().tick();
                        if let Ok(out) = out {
                            node.dispatch(&out);
                        }
                    }
                })
                .await;
            }
        });
    }

    /// Starts `cla-iroh` (if not running) and the loop feeding its events to the agent.
    fn start_lan(&self) {
        if self.lan_running() {
            return;
        }
        let Some(tokio) = &self.shared.tokio else {
            return;
        };
        let config = self.shared.config.lan.clone();
        // `block_on` keeps start-up ordered for callers; never called from a Tokio thread.
        let started = tokio.block_on(IrohCla::start(config));
        let Ok((lan, events)) = started else {
            tracing::warn!("local network channel failed to start");
            return;
        };
        *self
            .shared
            .lan
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(lan);
        self.spawn(Self::lan_loop(
            Arc::downgrade(&self.shared),
            events,
            ClaId::Iroh,
        ));
    }

    /// Feeds one Iroh instance's events to the agent, labelled with its channel (the local
    /// network, or Wi-Fi Direct).
    async fn lan_loop(
        weak: Weak<Shared>,
        mut events: mpsc::UnboundedReceiver<IrohEvent>,
        cla: ClaId,
    ) {
        while let Some(event) = events.recv().await {
            let input = match event {
                IrohEvent::Connected(p) => Input::EncounterStarted {
                    peer: PeerHandle(p.0),
                    cla,
                },
                IrohEvent::Message(p, msg) => Input::EncounterMessage {
                    peer: PeerHandle(p.0),
                    msg,
                },
                IrohEvent::Disconnected(p) => Input::EncounterEnded {
                    peer: PeerHandle(p.0),
                },
                IrohEvent::HubIncoming(stream) => {
                    tokio::spawn(Self::serve_hub(weak.clone(), stream, cla));
                    continue;
                }
            };
            // Agent work is CPU-bound and short; run it off the async workers.
            Self::with_node(weak.clone(), move |node| {
                let _ = node.handle(input);
            })
            .await;
        }
    }

    /// Carries out network outputs and publishes events. Called without the runtime lock.
    #[allow(clippy::too_many_lines, reason = "one arm per kind of output")]
    fn dispatch(&self, outputs: &[Output]) {
        let lan = self.lan_handle();
        for o in outputs {
            match o {
                // Reports the send for the live view, then falls through (always false).
                Output::Transmit { message, cla, .. } if self.traffic(*message, Some(*cla)) => {}
                Output::Encounter { peer, msg } => {
                    // A failed send surfaces as a disconnect from the reader.
                    let _ = self.send_to(lan.as_ref(), *peer, msg);
                }
                Output::Transmit {
                    op,
                    cla: ClaId::Hub,
                    bytes,
                    crossing,
                    ..
                } => {
                    // Reported done on the hub's `put_ok` (cla-hub), not on the write.
                    match self.hub_client() {
                        Some(hub) => hub.put(
                            op.0,
                            bytes.clone(),
                            crossing.unwrap_or(mem_core::Crossing::OwnOperator),
                        ),
                        None => self.fail_later(*op),
                    }
                }
                Output::EmailSend { op, to, bytes, .. } => match self.email_client() {
                    Some(c) => c.send(op.0, to.clone(), bytes.clone()),
                    None => self.fail_later(*op),
                },
                o @ (Output::Transmit {
                    cla: ClaId::DeviceHub,
                    ..
                }
                | Output::DeviceHubFetch { .. }
                | Output::DeviceHubPurge { .. }
                | Output::CloseDeviceHub { .. }
                | Output::WantDeviceHub { .. }) => self.dispatch_devhub(o),
                o @ (Output::HubWakeRegister { .. }
                | Output::HubWakeUnregister { .. }
                | Output::HubPurge { .. }
                | Output::HubFetch { .. }
                | Output::HubGetList
                | Output::HubCandidatesChanged) => self.dispatch_hub(o),
                Output::Transmit {
                    op,
                    peer: Some(peer),
                    bytes,
                    proof,
                    ..
                } => self.transmit(
                    lan.as_ref(),
                    *op,
                    *peer,
                    BundleEntry {
                        bytes: bytes.clone(),
                        proof: *proof,
                    },
                ),
                Output::Forward {
                    peer,
                    key,
                    hops,
                    copies,
                    age_ms,
                } => {
                    let relayed = self.read(|rt| rt.relayed(key, *hops, *copies, *age_ms));
                    if let Some(bytes) = relayed {
                        let _ = self.send_to(
                            lan.as_ref(),
                            *peer,
                            &EncounterMsg::Bundles(vec![BundleEntry::bare(bytes)]),
                        );
                    }
                }
                Output::EndEncounter { peer } => match cla_ble::Links::link(peer.0) {
                    Some(link) => {
                        // Reported down by the plugin afterwards, like Iroh's reader does.
                        let mut ble = self.ble();
                        ble.close(link);
                        ble.command(BleCommand::Close { link });
                    }
                    None if is_wifi(peer.0) => {
                        if let Some(w) = self.wifi_handle() {
                            w.close(cla_iroh::PeerHandle(peer.0));
                        }
                    }
                    None => {
                        if let Some(lan) = &lan {
                            lan.close(cla_iroh::PeerHandle(peer.0));
                        }
                    }
                },
                Output::Deliver { contact, message } => {
                    self.emit(NodeEvent::MessageReceived(*contact));
                    self.traffic(*message, None);
                }
                Output::StatusChanged { message, .. } | Output::Queued { message, .. } => {
                    self.emit(NodeEvent::MessageStatus(*message));
                }
                Output::ReachabilityChanged => self.emit(NodeEvent::Reachability),
                Output::ContactPaired { .. } | Output::ContactsChanged => {
                    self.emit(NodeEvent::Contacts);
                }
                Output::DeviceRetired => self.emit(NodeEvent::DeviceRetired),
                o @ Output::CallSignal { .. } => self.keep_call(o),
                Output::ContactRequest { .. } => self.emit(NodeEvent::Requests),
                Output::GroupInvitation { .. } | Output::GroupChanged { .. } => {
                    self.emit(NodeEvent::Groups);
                }
                Output::GroupMessage { group, .. } => self.emit(NodeEvent::GroupMessage(*group)),
                Output::SolvePow(job) => self.solve_pow(*job),
                o @ (Output::ShareSaved { .. }
                | Output::SharesChanged
                | Output::ShareItemWanted { .. }) => self.share_output(o),
                _ => {}
            }
        }
    }

    /// Hub requests other than uploads.
    /// Reports a transmit failed, off this thread (the agent is busy here).
    fn fail_later(&self, op: mem_core::model::OpId) {
        let weak = Arc::downgrade(&self.shared);
        self.spawn(Self::with_node(weak, move |node| {
            let _ = node.handle(Input::TransmitResult {
                op,
                result: Err(ClaError::Failed),
            });
        }));
    }

    fn dispatch_hub(&self, o: &Output) {
        // A new list or address: reconnect off this thread (the agent is busy here).
        if *o == Output::HubCandidatesChanged {
            let weak = Arc::downgrade(&self.shared);
            self.spawn(Self::with_node(weak, Node::sync_hub));
            return;
        }
        let Some(hub) = self.hub_client() else {
            return;
        };
        match o {
            Output::HubFetch { prefixes } => hub.fetch(prefixes.clone()),
            Output::HubPurge { tokens } => hub.purge(tokens.clone()),
            Output::HubWakeRegister { endpoint, prefixes } => {
                hub.wake_register(endpoint.clone(), prefixes.clone());
            }
            Output::HubWakeUnregister { endpoint } => hub.wake_unregister(endpoint.clone()),
            Output::HubGetList => hub.get_hubs(),
            _ => {}
        }
    }

    /// Solves a contact request's proof of work in the blocking pool (seconds of Argon2id)
    /// and hands the nonce back to the agent.
    fn solve_pow(&self, job: mem_core::PowJob) {
        let weak = Arc::downgrade(&self.shared);
        self.spawn(async move {
            let alive = weak.clone();
            let found = tokio::task::spawn_blocking(move || {
                // Chunks keep a dropped node from solving on for long.
                let mut start = 0u64;
                loop {
                    if let Some(n) = mem_crypto::intro::pow_solve(
                        &job.bundle_id,
                        &job.dst_tag,
                        job.week,
                        job.difficulty,
                        start,
                        64,
                    ) {
                        return Some(n);
                    }
                    if alive.strong_count() == 0 {
                        return None;
                    }
                    start = start.wrapping_add(64);
                }
            })
            .await;
            let Ok(Some(nonce)) = found else {
                return;
            };
            Self::with_node(weak, move |node| {
                let _ = node.handle(Input::PowSolved {
                    message: job.message,
                    nonce,
                });
            })
            .await;
        });
    }

    /// Sends a bundle to a peer and reports the outcome to the agent when the write finishes.
    fn transmit(
        &self,
        lan: Option<&IrohCla>,
        op: mem_core::model::OpId,
        peer: PeerHandle,
        entry: BundleEntry,
    ) {
        let sent = self.send_to(lan, peer, &EncounterMsg::Bundles(vec![entry]));
        let weak = Arc::downgrade(&self.shared);
        self.spawn(async move {
            let ok = match sent {
                Some(done) => done.await.unwrap_or(false),
                None => false,
            };
            let result = if ok { Ok(()) } else { Err(ClaError::Failed) };
            Self::with_node(weak, move |node| {
                let _ = node.handle(Input::TransmitResult { op, result });
            })
            .await;
        });
    }

    /// Keeps a call setup message for the app (bounded by [`CALL_INBOX`]) and says so.
    fn keep_call(&self, o: &Output) {
        let Output::CallSignal {
            contact,
            signal,
            via,
        } = o
        else {
            return;
        };
        let (contact, signal) = (
            *contact,
            IncomingCallSignal {
                contact: *contact,
                signal: signal.clone(),
                via: *via,
            },
        );
        {
            let mut calls = self
                .shared
                .calls
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if calls.len() >= CALL_INBOX {
                calls.pop_front();
            }
            calls.push_back(signal);
        }
        self.emit(NodeEvent::Call(contact));
    }

    /// Takes the call setup messages that arrived since the last call, oldest first.
    #[must_use]
    pub fn take_call_signals(&self) -> Vec<IncomingCallSignal> {
        self.shared
            .calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .drain(..)
            .collect()
    }

    /// Reports traffic for the live view: `cla` is the channel of a send (`None` for an
    /// arrival, whose channel the message records). Always `false`, so dispatch goes on.
    fn traffic(&self, message: MsgId, cla: Option<ClaId>) -> bool {
        let route = self.read(|rt| rt.agent().message_route(message));
        if let Some((contact, last, visible)) = route
            && let Some(channel) = cla.or(last)
        {
            self.emit(NodeEvent::Traffic {
                contact,
                cla: channel,
                outgoing: cla.is_some(),
                visible,
            });
        }
        false
    }

    fn share_output(&self, o: &Output) {
        let Output::ShareItemWanted {
            contact,
            share,
            item,
            handle,
        } = o
        else {
            self.emit(NodeEvent::Shares);
            return;
        };
        let r = ShareRequest {
            contact: *contact,
            share: *share,
            item: *item,
            handle: handle.clone(),
        };
        {
            let mut q = self
                .shared
                .share_wants
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            if q.len() >= SHARE_INBOX {
                q.pop_front();
            }
            q.push_back(r);
        }
        self.emit(NodeEvent::ShareWanted);
    }

    /// Takes the shared files contacts asked for since the last call, oldest first.
    pub fn take_share_requests(&self) -> Vec<ShareRequest> {
        self.shared
            .share_wants
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .drain(..)
            .collect()
    }

    fn emit(&self, event: NodeEvent) {
        let _ = self.shared.events.send(event);
    }
}

impl Drop for Shared {
    fn drop(&mut self) {
        let lan = self.lan.get_mut().map(Option::take).ok().flatten();
        let wifi = self.wifi.get_mut().map(Option::take).ok().flatten();
        let Some(runtime) = self.tokio.take() else {
            return;
        };
        let stop = move || {
            // Best effort: close connections so peers notice quickly.
            for cla in [lan, wifi].into_iter().flatten() {
                runtime.block_on(async {
                    let _ = tokio::time::timeout(Duration::from_secs(1), cla.shutdown()).await;
                });
            }
            drop(runtime);
        };
        // A Tokio runtime cannot be dropped or blocked on from inside one (the last handle may
        // be released by a background task): finish on a plain thread instead.
        if tokio::runtime::Handle::try_current().is_ok() {
            std::thread::spawn(stop);
        } else {
            stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mem_core::model::DeliveryStatus;

    fn tmp(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("mem-node-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn direct() -> NodeConfig {
        NodeConfig {
            lan: IrohConfig {
                mdns: false,
                ..IrohConfig::default()
            },
            ..NodeConfig::default()
        }
    }

    fn wait_for(rx: &mut broadcast::Receiver<NodeEvent>, want: impl Fn(&NodeEvent) -> bool) {
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while std::time::Instant::now() < deadline {
            match rx.try_recv() {
                Ok(e) if want(&e) => return,
                Ok(_) => {}
                Err(_) => std::thread::sleep(Duration::from_millis(20)),
            }
        }
        panic!("event not seen in time");
    }

    /// A fake radio between two nodes: each command goes through a queue (as the plugin's
    /// thread would), and the bytes arrive in 20-byte pieces, as GATT writes at the minimum
    /// ATT MTU of 23 do. With `bring_up`, the far side's link is reported up just
    /// before its first bytes, as a real plugin does: no link ever receives bytes before "up".
    fn ble_wire(from: &Node, to: &Node, link_there: u64, bring_up: bool) {
        let (tx, rx) = std::sync::mpsc::channel::<BleCommand>();
        from.on_ble_command(move |c| tx.send(c).is_ok());
        let (from, to) = (from.clone(), to.clone());
        std::thread::spawn(move || {
            let mut up = !bring_up;
            while let Ok(c) = rx.recv() {
                match c {
                    BleCommand::Send { op, bytes, .. } => {
                        if !up {
                            to.ble_link_up(link_there);
                            up = true;
                        }
                        for piece in bytes.chunks(20) {
                            to.ble_received(link_there, piece);
                        }
                        from.ble_sent(op, true);
                    }
                    BleCommand::Close { .. } => to.ble_link_down(link_there),
                }
            }
        });
    }

    #[test]
    fn two_nodes_deliver_over_bluetooth_in_small_chunks() {
        let (da, db) = (tmp("ble-a"), tmp("ble-b"));
        let off = NodeConfig {
            lan: IrohConfig {
                mdns: false,
                ..IrohConfig::default()
            },
            ..NodeConfig::default()
        };
        let a = Node::open(&da, off.clone()).unwrap();
        let b = Node::open(&db, off).unwrap();
        a.set_lan_enabled(false).unwrap();
        b.set_lan_enabled(false).unwrap();
        let card_b = b.read(Runtime::own_card_text);
        let card_a = a.read(Runtime::own_card_text);
        a.receive_text(&card_b, ClaId::Qr).unwrap();
        b.receive_text(&card_a, ClaId::Qr).unwrap();
        let b_id = a.read(|r| r.agent().contacts()[0].id);
        let a_id = b.read(|r| r.agent().contacts()[0].id);

        // Off by default: a link that comes up is closed at once.
        let (tx, rx) = std::sync::mpsc::channel();
        a.on_ble_command(move |c| tx.send(c).is_ok());
        a.ble_link_up(5);
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            BleCommand::Close { link: 5 }
        );
        a.ble_link_down(5);
        assert_eq!(a.read(|r| r.agent().peer_count()), 0);

        a.set_ble_enabled(true).unwrap();
        b.set_ble_enabled(true).unwrap();
        a.handle(Input::UserSend {
            contact: b_id,
            text: "over Bluetooth".into(),
        })
        .unwrap();
        let mut evb = b.subscribe();
        // A's link 1 is B's link 2 (each plugin numbers its own links). A's side comes up first;
        // B's comes up as A's first bytes reach it.
        ble_wire(&a, &b, 2, true);
        ble_wire(&b, &a, 1, false);
        a.ble_link_up(1);
        wait_for(&mut evb, |e| *e == NodeEvent::MessageReceived(a_id));
        let got = b.read(|r| r.agent().messages(a_id));
        assert_eq!(got[0].text, "over Bluetooth");
        assert_eq!(got[0].channels[0].cla, ClaId::Ble);
        assert_eq!(got[0].channels[0].grade, mem_core::PrivacyGrade::P0);
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while a.read(|r| r.agent().messages(b_id)[0].status) != DeliveryStatus::Delivered {
            assert!(std::time::Instant::now() < deadline, "never confirmed");
            std::thread::sleep(Duration::from_millis(20));
        }

        // Turning BLE off ends the encounter and is remembered across a restart.
        a.set_ble_enabled(false).unwrap();
        assert_eq!(a.read(|r| r.agent().peer_count()), 0);
        drop(a);
        let a = Node::open(&da, direct()).unwrap();
        assert!(!a.read(|r| r.agent().channel_enabled(ClaId::Ble)));
        drop((a, b));
        let _ = std::fs::remove_dir_all(da);
        let _ = std::fs::remove_dir_all(db);
    }

    /// Two nodes with no local network between them exchange a message and its ack
    /// through a hub, which holds nothing afterwards (purge tokens).
    #[test]
    fn two_nodes_deliver_through_the_hub() {
        // MEM_HUB_URL points the test at a real hub (the Elixir one in `hub/`) instead.
        let external = std::env::var("MEM_HUB_URL").ok();
        let hub_rt = tokio::runtime::Runtime::new().unwrap();
        let hub = hub_rt.block_on(cla_hub::test_hub::TestHub::start());
        let url = external.clone().unwrap_or_else(|| hub.url());
        // MEM_HUB_URL_B puts B on another hub of the same network.
        let url_b = std::env::var("MEM_HUB_URL_B").unwrap_or_else(|_| url.clone());
        let (da, db) = (tmp("hub-a"), tmp("hub-b"));
        let a = Node::open(&da, direct()).unwrap();
        let b = Node::open(&db, direct()).unwrap();
        a.set_lan_enabled(false).unwrap();
        b.set_lan_enabled(false).unwrap();
        let card_b = b.read(Runtime::own_card_text);
        let card_a = a.read(Runtime::own_card_text);
        a.receive_text(&card_b, ClaId::Qr).unwrap();
        b.receive_text(&card_a, ClaId::Qr).unwrap();
        let b_id = a.read(|r| r.agent().contacts()[0].id);
        let a_id = b.read(|r| r.agent().contacts()[0].id);
        for (n, url) in [(&a, &url), (&b, &url_b)] {
            let out = n.set_hub_url(Some(url.clone())).unwrap();
            assert!(!out.iter().any(|o| matches!(o, Output::Rejected(_))));
            n.set_hub_enabled(true).unwrap();
        }
        let mut evb = b.subscribe();
        a.handle(Input::UserSend {
            contact: b_id,
            text: "through the hub".into(),
        })
        .unwrap();
        wait_for(&mut evb, |e| *e == NodeEvent::MessageReceived(a_id));
        let got = b.read(|r| r.agent().messages(a_id));
        assert_eq!(got[0].text, "through the hub");
        assert_eq!(got[0].channels[0].cla, ClaId::Hub);
        assert_eq!(got[0].channels[0].grade, mem_core::PrivacyGrade::P2);
        // B's ack comes back by hub after the ack delay.
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while a.read(|r| r.agent().messages(b_id)[0].status) != DeliveryStatus::Delivered {
            assert!(std::time::Instant::now() < deadline, "never confirmed");
            std::thread::sleep(Duration::from_millis(50));
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while external.is_none() && !hub.is_empty() {
            assert!(
                std::time::Instant::now() < deadline,
                "hub kept {}",
                hub.len()
            );
            std::thread::sleep(Duration::from_millis(50));
        }
        a.set_hub_enabled(false).unwrap();
        assert!(!a.hub_online());
        drop((a, b));
        let _ = std::fs::remove_dir_all(da);
        let _ = std::fs::remove_dir_all(db);
    }

    /// A fresh profile joins as another device of B through the node API; a
    /// profile with contacts refuses to; a retired device forgets everything.
    #[test]
    fn a_fresh_profile_joins_as_another_device() {
        let (da, db, dn) = (tmp("join-a"), tmp("join-b"), tmp("join-n"));
        let a = Node::open(&da, direct()).unwrap();
        let b = Node::open(&db, direct()).unwrap();
        let n = Node::open(&dn, direct()).unwrap();
        for x in [&a, &b, &n] {
            x.set_lan_enabled(false).unwrap();
        }
        a.receive_text(&b.read(Runtime::own_card_text), ClaId::Qr)
            .unwrap();
        b.receive_text(&a.read(Runtime::own_card_text), ClaId::Qr)
            .unwrap();
        // B has a contact: it cannot become someone else's device.
        assert!(matches!(b.device_join(), Err(RuntimeError::NotEmpty)));
        let join = n.device_join().unwrap();
        let out = b
            .handle(Input::ApproveDevice {
                request: join.request(),
            })
            .unwrap();
        let setup = out
            .iter()
            .find_map(|o| match o {
                Output::DeviceSetupReady { setup } => Some(setup.clone()),
                _ => None,
            })
            .unwrap();
        // A setup for someone else's request leaves this request open.
        let other = n.device_join().unwrap();
        assert!(matches!(
            n.join_device(&other, &setup),
            Err(RuntimeError::Join(_))
        ));
        n.join_device(&join, &setup).unwrap();
        assert!(!n.read(|r| r.agent().is_main_device()));
        assert_eq!(
            n.read(|r| r.agent().fingerprint()),
            b.read(|r| r.agent().fingerprint())
        );
        assert_eq!(n.read(|r| r.agent().contacts().len()), 1);
        // Survives a restart (the profile is saved).
        drop(n);
        let n = Node::open(&dn, direct()).unwrap();
        assert!(!n.read(|r| r.agent().is_main_device()));
        assert_eq!(n.read(|r| r.agent().contacts().len()), 1);
        n.forget_this_device().unwrap();
        assert!(n.read(|r| r.agent().is_main_device()));
        assert_eq!(n.read(|r| r.agent().contacts().len()), 0);
        drop((a, b, n));
        for d in [da, db, dn] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    /// A and B use hubs of two operators that name each other as partners. A's
    /// message and B's ack cross between them and are recorded as P3 on both sides.
    ///
    /// In-process, the test hub stands in for both operators (it delivers "may cross" uploads
    /// as from another operator). `hub/scripts/network-test.sh` runs it against real hubs with
    /// `MEM_OP1_URL`, `MEM_OP2_URL` (one hub of each operator), `MEM_OP1_LIST`, `MEM_OP2_LIST`
    /// (their signed lists) and `MEM_OP1_KEY`, `MEM_OP2_KEY` (operator public keys, hex).
    #[test]
    fn two_nodes_deliver_across_operators() {
        use mem_crypto::IdentityKey;
        use mem_proto::hublist::{HUB_LIST_DOMAIN, HubEntry, HubList, PartnerEntry};
        let env = |k: &str| std::env::var(k).ok();
        let hex = |s: &str| -> [u8; 32] {
            let v: Vec<u8> = (0..s.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
                .collect();
            v.try_into().unwrap()
        };
        let hub_rt = tokio::runtime::Runtime::new().unwrap();
        let hub = hub_rt.block_on(cla_hub::test_hub::TestHub::start());
        // (url, signed list, operator key) for each side.
        let sides: [(String, Vec<u8>, [u8; 32]); 2] =
            if let (Some(u1), Some(u2)) = (env("MEM_OP1_URL"), env("MEM_OP2_URL")) {
                let side = |url: String, list: &str, key: &str| {
                    let list = std::fs::read(env(list).unwrap()).unwrap();
                    (url, list, hex(&env(key).unwrap()))
                };
                [
                    side(u1, "MEM_OP1_LIST", "MEM_OP1_KEY"),
                    side(u2, "MEM_OP2_LIST", "MEM_OP2_KEY"),
                ]
            } else {
                let op = IdentityKey::from_seed(&[42; 32]);
                let mut list = HubList {
                    serial: 1,
                    issued_unix_ms: 0,
                    expires_unix_ms: u64::MAX,
                    hubs: vec![HubEntry {
                        url: hub.url(),
                        key: *IdentityKey::from_seed(&[43; 32]).public().as_bytes(),
                        region: None,
                    }],
                    signature: [0; 64],
                    partners: vec![PartnerEntry {
                        url: "wss://partner.invalid/v1".into(),
                        key: *IdentityKey::from_seed(&[44; 32]).public().as_bytes(),
                        operator: "Partner".into(),
                    }],
                };
                list.signature = op.sign(HUB_LIST_DOMAIN, &[&list.signed_bytes().unwrap()]);
                let side = (hub.url(), list.encode().unwrap(), *op.public().as_bytes());
                [side.clone(), side]
            };
        let (da, db) = (tmp("partner-a"), tmp("partner-b"));
        let a = Node::open(&da, direct()).unwrap();
        let b = Node::open(&db, direct()).unwrap();
        a.receive_text(&b.read(Runtime::own_card_text), ClaId::Qr)
            .unwrap();
        b.receive_text(&a.read(Runtime::own_card_text), ClaId::Qr)
            .unwrap();
        let b_id = a.read(|r| r.agent().contacts()[0].id);
        let a_id = b.read(|r| r.agent().contacts()[0].id);
        for (n, (url, list, key)) in [(&a, &sides[0]), (&b, &sides[1])] {
            n.set_lan_enabled(false).unwrap();
            n.handle(Input::SetOperatorKeys(vec![*key])).unwrap();
            n.handle(Input::HubListReceived {
                bytes: list.clone(),
            })
            .unwrap();
            assert!(
                !n.read(|r| r.agent().hub_partners().is_empty()),
                "list taken"
            );
            n.set_hub_url(Some(url.clone())).unwrap();
            n.set_hub_enabled(true).unwrap();
        }
        let mut evb = b.subscribe();
        a.handle(Input::UserSend {
            contact: b_id,
            text: "across operators".into(),
        })
        .unwrap();
        wait_for(&mut evb, |e| *e == NodeEvent::MessageReceived(a_id));
        let got = b.read(|r| r.agent().messages(a_id));
        assert_eq!(got[0].text, "across operators");
        assert_eq!(got[0].channels[0].cla, ClaId::Hub);
        assert_eq!(got[0].channels[0].grade, mem_core::PrivacyGrade::P3);
        let sent = a.read(|r| r.agent().messages(b_id));
        assert_eq!(sent[0].channels[0].grade, mem_core::PrivacyGrade::P3);
        // B's ack crosses back.
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while a.read(|r| r.agent().messages(b_id)[0].status) != DeliveryStatus::Delivered {
            assert!(std::time::Instant::now() < deadline, "never confirmed");
            std::thread::sleep(Duration::from_millis(50));
        }
        drop((a, b));
        let _ = std::fs::remove_dir_all(da);
        let _ = std::fs::remove_dir_all(db);
    }

    /// The name the user gives a contact: stored with the state, kept when the contact pairs
    /// again, cleared by an empty name, refused when too long or for an unknown contact.
    #[test]
    fn contact_alias_is_kept_and_persisted() {
        let (da, db) = (tmp("alias-a"), tmp("alias-b"));
        let b_id;
        {
            let a = Node::open(&da, direct()).unwrap();
            let b = Node::open(&db, direct()).unwrap();
            let card_b = b.read(Runtime::own_card_text);
            a.receive_text(&card_b, ClaId::Qr).unwrap();
            b_id = a.read(|r| r.agent().contacts()[0].id);
            let out = a
                .handle(Input::SetContactAlias {
                    contact: b_id,
                    alias: Some("  Bo from work ".into()),
                })
                .unwrap();
            assert!(!out.iter().any(|o| matches!(o, Output::Rejected(_))));
            // Pairing again keeps the user's name.
            a.receive_text(&card_b, ClaId::Qr).unwrap();
            assert_eq!(
                a.read(|r| r.agent().contacts()[0].alias.clone()).as_deref(),
                Some("Bo from work")
            );
            a.handle(Input::SetContactRingtone {
                contact: b_id,
                ringtone: Some("pulse".into()),
            })
            .unwrap();
            let long = a
                .handle(Input::SetContactAlias {
                    contact: b_id,
                    alias: Some("x".repeat(65)),
                })
                .unwrap();
            assert!(long.iter().any(|o| matches!(
                o,
                Output::Rejected(mem_core::model::RejectReason::InvalidName)
            )));
            let unknown = a
                .handle(Input::SetContactAlias {
                    contact: ContactId::from_bytes([7; 32]),
                    alias: Some("me".into()),
                })
                .unwrap();
            assert!(unknown.iter().any(|o| matches!(
                o,
                Output::Rejected(mem_core::model::RejectReason::UnknownContact)
            )));
        }
        let a = Node::open(&da, direct()).unwrap();
        assert_eq!(
            a.read(|r| r.agent().contacts()[0].alias.clone()).as_deref(),
            Some("Bo from work")
        );
        assert_eq!(
            a.read(|r| r.agent().contacts()[0].ringtone.clone())
                .as_deref(),
            Some("pulse")
        );
        a.handle(Input::SetContactAlias {
            contact: b_id,
            alias: Some("   ".into()),
        })
        .unwrap();
        assert_eq!(a.read(|r| r.agent().contacts()[0].alias.clone()), None);
        drop(a);
        let _ = std::fs::remove_dir_all(da);
        let _ = std::fs::remove_dir_all(db);
    }

    /// The email bridge end to end through a real mail server on this machine (`GreenMail` with users
    /// ana@example.org and bo@example.net, password `pw`; set `MEM_TEST_MAIL=127.0.0.1`).
    #[test]
    fn two_nodes_deliver_by_email() {
        let Ok(host) = std::env::var("MEM_TEST_MAIL") else {
            return;
        };
        let account = |user: &str| mem_core::EmailAccount {
            address: user.into(),
            imap_host: host.clone(),
            imap_port: 3143,
            smtp_host: host.clone(),
            smtp_port: 3025,
            username: user.into(),
            password: zeroize::Zeroizing::new("pw".into()),
        };
        let (da, db) = (tmp("mail-a"), tmp("mail-b"));
        let a = Node::open(&da, direct()).unwrap();
        let b = Node::open(&db, direct()).unwrap();
        a.set_lan_enabled(false).unwrap();
        b.set_lan_enabled(false).unwrap();
        let card_b = b.read(Runtime::own_card_text);
        let card_a = a.read(Runtime::own_card_text);
        a.receive_text(&card_b, ClaId::Qr).unwrap();
        b.receive_text(&card_a, ClaId::Qr).unwrap();
        let b_id = a.read(|r| r.agent().contacts()[0].id);
        let a_id = b.read(|r| r.agent().contacts()[0].id);
        a.handle(Input::SetContactEmail {
            contact: b_id,
            address: Some("bo@example.net".into()),
        })
        .unwrap();
        a.set_email_account(Some(account("ana@example.org")))
            .unwrap();
        b.set_email_account(Some(account("bo@example.net")))
            .unwrap();
        a.set_email_enabled(true).unwrap();
        b.set_email_enabled(true).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while !a.read(|r| r.agent().email_online()) {
            assert!(std::time::Instant::now() < deadline, "never connected");
            std::thread::sleep(Duration::from_millis(50));
        }
        let mut evb = b.subscribe();
        let out = a
            .handle(Input::UserSend {
                contact: b_id,
                text: "by mail".into(),
            })
            .unwrap();
        let msg = out
            .iter()
            .find_map(|o| match o {
                Output::Queued { message, .. } => Some(*message),
                _ => None,
            })
            .unwrap();
        // P4: the user consents (once).
        let out = a
            .handle(Input::RequestTransmit {
                message: msg,
                cla: ClaId::Email,
            })
            .unwrap();
        let offer = out
            .iter()
            .find_map(|o| match o {
                Output::ConsentRequired(o) => Some(o.clone()),
                _ => None,
            })
            .unwrap();
        a.handle(Input::ConsentGiven(mem_core::model::ConsentRecord {
            offer: offer.id,
            message: msg,
            cla: ClaId::Email,
            grade: offer.grade,
            scope: mem_core::model::ConsentScope::Once,
        }))
        .unwrap();
        b.check_email();
        wait_for(&mut evb, |e| *e == NodeEvent::MessageReceived(a_id));
        let got = b.read(|r| r.agent().messages(a_id));
        assert_eq!(got[0].text, "by mail");
        assert_eq!(got[0].channels[0].cla, ClaId::Email);
        assert_eq!(got[0].channels[0].grade, mem_core::PrivacyGrade::P4);
        assert_eq!(
            a.read(|r| r.agent().messages(b_id)[0].status),
            DeliveryStatus::Sent
        );
        drop((a, b));
        let _ = std::fs::remove_dir_all(da);
        let _ = std::fs::remove_dir_all(db);
    }

    /// Wi-Fi Direct: off by default; when on, a node dials the endpoint and port the
    /// other advertises (loopback here, the P2P group's address on phones), and the message
    /// arrives labelled Wi-Fi Direct, P0, with the local-network channel off.
    #[test]
    fn two_nodes_deliver_over_wifi_direct() {
        let (da, db) = (tmp("wd-a"), tmp("wd-b"));
        let a = Node::open(&da, direct()).unwrap();
        let b = Node::open(&db, direct()).unwrap();
        assert!(a.wifi_local().is_none(), "off by default");
        for n in [&a, &b] {
            n.set_lan_enabled(false).unwrap();
            n.set_wifi_direct_enabled(true).unwrap();
        }
        let card_b = b.read(Runtime::own_card_text);
        let card_a = a.read(Runtime::own_card_text);
        a.receive_text(&card_b, ClaId::Qr).unwrap();
        b.receive_text(&card_a, ClaId::Qr).unwrap();
        let b_id = a.read(|r| r.agent().contacts()[0].id);
        let a_id = b.read(|r| r.agent().contacts()[0].id);
        let mut evb = b.subscribe();
        a.handle(Input::UserSend {
            contact: b_id,
            text: "over Wi-Fi Direct".into(),
        })
        .unwrap();
        let (id, port) = b.wifi_local().unwrap();
        a.wifi_peer(id, std::net::SocketAddr::from(([127, 0, 0, 1], port)));
        wait_for(&mut evb, |e| *e == NodeEvent::MessageReceived(a_id));
        let got = b.read(|r| r.agent().messages(a_id));
        assert_eq!(got[0].text, "over Wi-Fi Direct");
        assert_eq!(got[0].channels[0].cla, ClaId::WifiDirect);
        assert_eq!(got[0].channels[0].grade, mem_core::PrivacyGrade::P0);
        // The acknowledgement comes back over the same link.
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while a.read(|r| r.agent().messages(b_id)[0].status) != DeliveryStatus::Delivered {
            assert!(std::time::Instant::now() < deadline, "never confirmed");
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(a.wifi_connections(), 1);
        b.set_wifi_direct_enabled(false).unwrap();
        assert!(b.wifi_local().is_none());
        drop((a, b));
        let _ = std::fs::remove_dir_all(da);
        let _ = std::fs::remove_dir_all(db);
    }

    /// Connect by sound or code: without mDNS, one phone dials what the other's
    /// hint names, through the hint's wire form, and a queued message goes through.
    #[test]
    fn a_connect_hint_brings_two_nodes_together() {
        let (da, db) = (tmp("hint-a"), tmp("hint-b"));
        let a = Node::open(&da, direct()).unwrap();
        let b = Node::open(&db, direct()).unwrap();
        let card_b = b.read(Runtime::own_card_text);
        let card_a = a.read(Runtime::own_card_text);
        a.receive_text(&card_b, ClaId::Qr).unwrap();
        b.receive_text(&card_a, ClaId::Qr).unwrap();
        let b_id = a.read(|r| r.agent().contacts()[0].id);
        let a_id = b.read(|r| r.agent().contacts()[0].id);
        let mut evb = b.subscribe();
        a.handle(Input::UserSend {
            contact: b_id,
            text: "found you by sound".into(),
        })
        .unwrap();
        let Some(hint) = b.connect_hint() else {
            // A machine with no non-loopback IPv4 address has nothing to put in a hint.
            return;
        };
        let heard = mem_proto::hint::ConnectHint::decode(&hint.encode().unwrap()).unwrap();
        // Our own hint (heard through our own microphone) is ignored.
        assert!(b.dial_hint(&heard));
        assert!(a.dial_hint(&heard));
        wait_for(&mut evb, |e| *e == NodeEvent::MessageReceived(a_id));
        assert_eq!(
            b.read(|r| r.agent().messages(a_id))[0].text,
            "found you by sound"
        );
        drop((a, b));
        let _ = std::fs::remove_dir_all(da);
        let _ = std::fs::remove_dir_all(db);
    }

    /// A device hub over a real Iroh connection: C uploads a message for R to H's device while
    /// R is away; later R meets H (H dials this time) and fetches it. C and R never connect.
    #[test]
    fn a_contacts_device_carries_a_message_as_a_hub() {
        use mem_core::DeviceHubSettings;
        let dirs = [tmp("dh-h"), tmp("dh-c"), tmp("dh-r")];
        let h = Node::open(&dirs[0], direct()).unwrap();
        let c = Node::open(&dirs[1], direct()).unwrap();
        let r = Node::open(&dirs[2], direct()).unwrap();
        for (x, y) in [(&h, &c), (&h, &r), (&c, &r)] {
            let (cx, cy) = (
                x.read(Runtime::own_card_text),
                y.read(Runtime::own_card_text),
            );
            x.receive_text(&cy, ClaId::Qr).unwrap();
            y.receive_text(&cx, ClaId::Qr).unwrap();
        }
        // Contact ids are the same from every side; find each by its fingerprint.
        let id_of = |n: &Node, seen_by: &Node| {
            let fp = n.read(|rt| rt.agent().fingerprint());
            seen_by.read(|rt| {
                rt.agent()
                    .contacts()
                    .into_iter()
                    .find(|x| x.fingerprint == fp)
                    .unwrap()
                    .id
            })
        };
        let (h_id, c_id, r_id) = (id_of(&h, &c), id_of(&c, &r), id_of(&r, &c));
        // H carries nothing as a courier, so only its hub can bring the message to R.
        h.handle(Input::SetCourierLimit { bytes: 0 }).unwrap();
        h.handle(Input::SetDeviceHubSettings(DeviceHubSettings {
            serve: true,
            ..DeviceHubSettings::default()
        }))
        .unwrap();
        for n in [&c, &r] {
            n.handle(Input::SetUseDeviceHub {
                contact: h_id,
                enabled: true,
            })
            .unwrap();
        }
        c.handle(Input::UserSend {
            contact: r_id,
            text: "left at the tablet".into(),
        })
        .unwrap();
        c.lan_connect(h.lan_addr().unwrap());
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while !c
            .read(|rt| rt.agent().messages(r_id))
            .first()
            .is_some_and(|m| m.channels.iter().any(|u| u.cla == ClaId::DeviceHub))
        {
            assert!(std::time::Instant::now() < deadline, "never uploaded");
            std::thread::sleep(Duration::from_millis(50));
        }
        assert_eq!(h.read(|rt| rt.hub_usage().unwrap()).1, 1);

        let mut evr = r.subscribe();
        h.lan_connect(r.lan_addr().unwrap());
        wait_for(&mut evr, |e| *e == NodeEvent::MessageReceived(c_id));
        let got = r.read(|rt| rt.agent().messages(c_id));
        assert_eq!(got[0].text, "left at the tablet");
        assert_eq!(got[0].channels[0].cla, ClaId::DeviceHub);
        assert_eq!(got[0].channels[0].grade, mem_core::PrivacyGrade::P3);
        drop((h, c, r));
        for d in dirs {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    /// A shared file too large for parts: offered, asked for, read by the owner's app, pulled
    /// directly, and kept in "Shared with me" rather than in the chat.
    #[test]
    fn a_shared_file_is_requested_and_pulled_over_the_local_network() {
        use mem_core::model::{
            AttachmentData, IntroducedMembers, NewAttachment, NewShareItem, ShareAudience,
            SharedItemState,
        };
        let (da, db) = (tmp("share-a"), tmp("share-b"));
        let a = Node::open(&da, direct()).unwrap();
        let b = Node::open(&db, direct()).unwrap();
        let card_b = b.read(Runtime::own_card_text);
        let card_a = a.read(Runtime::own_card_text);
        a.receive_text(&card_b, ClaId::Qr).unwrap();
        b.receive_text(&card_a, ClaId::Qr).unwrap();
        let b_id = a.read(|r| r.agent().contacts()[0].id);
        let a_id = b.read(|r| r.agent().contacts()[0].id);
        let file = da.join("report.bin");
        let body: Vec<u8> = (0..300_000u32).map(|i| (i % 253) as u8).collect();
        std::fs::write(&file, &body).unwrap();
        let handle = file.to_string_lossy().into_owned();

        let mut eva = a.subscribe();
        let mut evb = b.subscribe();
        a.lan_connect(b.lan_addr().unwrap());
        a.handle(Input::SaveShare {
            share: None,
            name: "Reports".into(),
            audience: ShareAudience::Contact(b_id),
            introduced: IntroducedMembers::Excluded,
            items: vec![NewShareItem {
                name: "report.bin".into(),
                size: body.len() as u64,
                media_type: "application/octet-stream".into(),
                modified_s: None,
                handle: handle.clone(),
            }],
        })
        .unwrap();
        wait_for(&mut evb, |e| *e == NodeEvent::Shares);
        let offered = b.read(|r| r.agent().shared_with_me(Some(a_id)));
        assert_eq!(offered.len(), 1);
        let (share, item) = (offered[0].id, offered[0].items[0].id);

        b.handle(Input::RequestSharedItem {
            contact: a_id,
            share,
            item,
        })
        .unwrap();
        wait_for(&mut eva, |e| *e == NodeEvent::ShareWanted);
        let wanted = a.take_share_requests();
        assert_eq!(wanted.len(), 1);
        assert_eq!(wanted[0].handle, handle);
        let data = a.read(|r| r.seal_file(&file)).unwrap();
        assert!(matches!(data, AttachmentData::Stored { .. }));
        a.handle(Input::ServeSharedItem {
            contact: b_id,
            share,
            item,
            file: NewAttachment {
                name: "report.bin".into(),
                media_type: "application/octet-stream".into(),
                data,
                preview: None,
            },
        })
        .unwrap();

        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        let opened = loop {
            let state = b.read(|r| r.agent().shared_with_me(Some(a_id))[0].items[0].state);
            if let SharedItemState::Received(m) = state
                && let Some(o) = b.read(|r| r.open_attachment(m, 0)).unwrap()
            {
                break o;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "never arrived: {state:?}"
            );
            std::thread::sleep(Duration::from_millis(50));
        };
        assert_eq!(&opened.data[..], &body[..]);
        // Not a chat message on either side: B's chat has only the quiet line.
        let chat = b.read(|r| r.agent().messages(a_id));
        assert_eq!(chat.len(), 1);
        assert_eq!(
            chat[0].kind,
            mem_core::model::MessageKind::ShareNotice { new_items: 1 }
        );
        assert_eq!(a.read(|r| r.agent().messages(b_id)).len(), 0);
        // The owner sees who downloaded it.
        assert_eq!(
            a.read(|r| r.agent().own_shares()[0].items[0].downloads.len()),
            1
        );
        drop((a, b));
        let _ = std::fs::remove_dir_all(da);
        let _ = std::fs::remove_dir_all(db);
    }

    /// A message and its ack over a real Iroh connection (direct address, not mDNS).
    #[test]
    fn two_nodes_deliver_over_the_local_network() {
        let (da, db) = (tmp("a"), tmp("b"));
        let a = Node::open(&da, direct()).unwrap();
        let b = Node::open(&db, direct()).unwrap();
        // Pair by card text as if scanned.
        let card_b = b.read(Runtime::own_card_text);
        let card_a = a.read(Runtime::own_card_text);
        a.receive_text(&card_b, ClaId::Qr).unwrap();
        b.receive_text(&card_a, ClaId::Qr).unwrap();
        let b_id = a.read(|r| r.agent().contacts()[0].id);
        let a_id = b.read(|r| r.agent().contacts()[0].id);

        let mut eva = a.subscribe();
        let mut evb = b.subscribe();
        // Queue while disconnected.
        a.handle(Input::UserSend {
            contact: b_id,
            text: "over Wi-Fi".into(),
        })
        .unwrap();
        a.lan_connect(b.lan_addr().unwrap());
        wait_for(&mut evb, |e| *e == NodeEvent::MessageReceived(a_id));
        let got = b.read(|r| r.agent().messages(a_id));
        assert_eq!(got[0].text, "over Wi-Fi");
        assert_eq!(got[0].channels[0].cla, ClaId::Iroh);
        wait_for(&mut eva, |e| matches!(e, NodeEvent::MessageStatus(_)));
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while a.read(|r| r.agent().messages(b_id)[0].status) == DeliveryStatus::Queued {
            assert!(std::time::Instant::now() < deadline, "never sent");
            std::thread::sleep(Duration::from_millis(20));
        }
        // B sends nothing: only its standalone ack, after the ack delay, can confirm.
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while a.read(|r| r.agent().messages(b_id)[0].status) != DeliveryStatus::Delivered {
            assert!(
                std::time::Instant::now() < deadline,
                "standalone ack never confirmed: {:?}",
                a.read(|r| r.agent().messages(b_id)[0].status)
            );
            std::thread::sleep(Duration::from_millis(20));
        }

        // Reply while connected goes out immediately.
        b.handle(Input::UserSend {
            contact: a_id,
            text: "back".into(),
        })
        .unwrap();
        wait_for(&mut eva, |e| *e == NodeEvent::MessageReceived(b_id));
        // The reply carried B's ack (or B sent one on its own): A's message is confirmed.
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while a.read(|r| r.agent().messages(b_id)[0].status) != DeliveryStatus::Delivered {
            assert!(std::time::Instant::now() < deadline, "never confirmed");
            std::thread::sleep(Duration::from_millis(20));
        }

        // Turning the channel off ends the encounter and is remembered.
        a.set_lan_enabled(false).unwrap();
        assert!(!a.lan_running());
        drop(a);
        let a = Node::open(&da, direct()).unwrap();
        assert!(!a.lan_running());
        drop((a, b));
        let _ = std::fs::remove_dir_all(da);
        let _ = std::fs::remove_dir_all(db);
    }
}
