//! A contact's device acting as a hub (`spec/hub.cddl`, device hub login).
//!
//! Both sides of the login, and the client: with "use this contact's device as a hub" on, an
//! encounter with that contact over Iroh (local network or Wi-Fi Direct) makes the runtime dial
//! the same device for its hub. After both proofs check, we upload every own undelivered
//! bundle the gate allows without asking (grade P3: the hub's owner sees which of their
//! contacts sent and fetched what, when) and fetch our prefixes. The hub itself (store and
//! quotas) is in the runtime; the core only says who may log in.

use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::vec::Vec;

use mem_cla::ClaId;
use mem_crypto::DeviceHubSide;
use mem_proto::encounter::MAX_PURGE_TOKENS;
use rand_core::CryptoRng;
use serde::{Deserialize, Serialize};

use super::{Agent, Ask, Output, StorageOp};
use crate::model::{ContactId, MsgId, PeerHandle, RejectReason};

/// Default space for other people's bundles on a device acting as a hub.
pub const DEFAULT_HUB_TOTAL_BYTES: u64 = 256 * 1024 * 1024;
/// Default share of it for one contact.
pub const DEFAULT_HUB_PER_CONTACT_BYTES: u64 = 32 * 1024 * 1024;
const MIN_HUB_BYTES: u64 = 1024 * 1024;
const MAX_HUB_BYTES: u64 = 4 * 1024 * 1024 * 1024;
/// Purge tokens remembered to pass on to device hubs.
const RECENT_TOKENS: usize = MAX_PURGE_TOKENS;

/// Whether this device acts as a hub for its contacts, and how much it keeps for them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceHubSettings {
    /// Serving on (off by default).
    pub serve: bool,
    /// Space for everyone's bundles, in bytes.
    pub total_bytes: u64,
    /// Space for one contact's bundles, in bytes.
    pub per_contact_bytes: u64,
}

impl Default for DeviceHubSettings {
    fn default() -> Self {
        Self {
            serve: false,
            total_bytes: DEFAULT_HUB_TOTAL_BYTES,
            per_contact_bytes: DEFAULT_HUB_PER_CONTACT_BYTES,
        }
    }
}

impl DeviceHubSettings {
    pub(crate) fn is_default(&self) -> bool {
        *self == Self::default()
    }

    /// Within bounds: total 1 MiB to 4 GiB, a contact's share at most the total.
    fn clamped(self) -> Self {
        let total = self.total_bytes.clamp(MIN_HUB_BYTES, MAX_HUB_BYTES);
        Self {
            serve: self.serve,
            total_bytes: total,
            per_contact_bytes: self.per_contact_bytes.clamp(MIN_HUB_BYTES, total),
        }
    }
}

/// Both proofs of one login, for the client.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct DeviceHubProofs {
    /// Ours, to send in `hub_auth`.
    pub client: [u8; 32],
    /// What the hub must answer in `hub_auth_ok`.
    pub hub: [u8; 32],
}

impl core::fmt::Debug for DeviceHubProofs {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("DeviceHubProofs")
    }
}

/// One logged-in connection to a contact's device hub (volatile).
struct Link {
    contact: ContactId,
    /// Messages it accepted.
    uploaded: BTreeSet<MsgId>,
    /// Prefix batches still to fetch, the live set last.
    plan: VecDeque<Vec<[u8; 2]>>,
    fetching: bool,
}

/// Client state: connections and what to tell them (volatile), plus the serving settings.
#[derive(Default)]
pub(crate) struct DeviceHubs {
    pub(crate) settings: DeviceHubSettings,
    links: BTreeMap<PeerHandle, Link>,
    /// Encounters we already asked the runtime to dial for a hub.
    asked: BTreeSet<PeerHandle>,
    /// Recent purge tokens, passed on to every device hub we use.
    tokens: VecDeque<[u8; 16]>,
}

impl DeviceHubs {
    pub(crate) fn with_settings(settings: DeviceHubSettings) -> Self {
        Self {
            settings,
            ..Self::default()
        }
    }
}

impl Agent {
    /// This device's hub settings.
    #[must_use]
    pub const fn device_hub_settings(&self) -> DeviceHubSettings {
        self.devhub.settings
    }

    /// Whether the user lets us use `contact`'s device as a hub.
    #[must_use]
    pub fn uses_device_hub(&self, contact: ContactId) -> bool {
        self.contacts
            .get(&contact)
            .is_some_and(|c| c.prefs.use_as_hub)
    }

    /// Contacts whose device hub we are logged in to.
    #[must_use]
    pub fn device_hubs_online(&self) -> Vec<ContactId> {
        self.devhub.links.values().map(|l| l.contact).collect()
    }

    /// Client side of a login to `contact`'s device hub at `hub_endpoint` (the transport key we
    /// dialled): our proof and the one the hub must answer. `None` unless the user turned the
    /// hub on for this contact.
    #[must_use]
    pub fn device_hub_proofs(
        &self,
        contact: ContactId,
        hub_endpoint: &[u8; 32],
        server_nonce: &[u8; 16],
        client_nonce: &[u8; 16],
    ) -> Option<DeviceHubProofs> {
        if self.disabled.contains(&ClaId::DeviceHub) {
            return None;
        }
        let c = self.contacts.get(&contact)?;
        if !c.prefs.use_as_hub || c.requested || c.group_only {
            return None;
        }
        let proof = |side| {
            c.secret
                .device_hub_proof(side, hub_endpoint, server_nonce, client_nonce)
        };
        Some(DeviceHubProofs {
            client: proof(DeviceHubSide::Client),
            hub: proof(DeviceHubSide::Hub),
        })
    }

    /// Hub side of a login on our endpoint `hub_endpoint`: which approved contact `proof`
    /// comes from, and our answer. `None` while serving is off or for anyone else.
    ///
    /// # Security
    ///
    /// Only contacts approved for 1:1 messages log in: not pending requests, not group members
    /// we never met. Every secret is tried (constant-time compare each).
    #[must_use]
    pub fn device_hub_login(
        &self,
        hub_endpoint: &[u8; 32],
        server_nonce: &[u8; 16],
        client_nonce: &[u8; 16],
        proof: &[u8; 32],
    ) -> Option<(ContactId, [u8; 32])> {
        if !self.devhub.settings.serve {
            return None;
        }
        self.contacts
            .iter()
            .filter(|(_, c)| !c.requested && !c.group_only)
            .find(|(_, c)| {
                c.secret.verify_device_hub_proof(
                    DeviceHubSide::Client,
                    hub_endpoint,
                    server_nonce,
                    client_nonce,
                    proof,
                )
            })
            .map(|(id, c)| {
                (
                    *id,
                    c.secret.device_hub_proof(
                        DeviceHubSide::Hub,
                        hub_endpoint,
                        server_nonce,
                        client_nonce,
                    ),
                )
            })
    }

    pub(super) fn set_device_hub_settings(&mut self, s: DeviceHubSettings) -> Vec<Output> {
        self.devhub.settings = s.clamped();
        alloc::vec![
            Output::Persist(StorageOp::SaveSnapshot),
            Output::DeviceHubSettingsChanged,
        ]
    }

    pub(super) fn set_use_device_hub(&mut self, contact: ContactId, enabled: bool) -> Vec<Output> {
        let Some(c) = self.contacts.get_mut(&contact) else {
            return alloc::vec![Output::Rejected(RejectReason::UnknownContact)];
        };
        c.prefs.use_as_hub = enabled;
        let mut out = alloc::vec![Output::Persist(StorageOp::SaveSnapshot)];
        if enabled {
            out.extend(self.want_device_hubs());
        } else {
            let gone: Vec<PeerHandle> = self
                .devhub
                .links
                .iter()
                .filter(|(_, l)| l.contact == contact)
                .map(|(p, _)| *p)
                .collect();
            for peer in gone {
                out.push(Output::CloseDeviceHub { peer });
            }
        }
        out
    }

    /// Asks the runtime to dial the hub of every contact we use as one and meet in an
    /// encounter over Iroh, once per encounter.
    pub(super) fn want_device_hubs(&mut self) -> Vec<Output> {
        if self.disabled.contains(&ClaId::DeviceHub) {
            return Vec::new();
        }
        let online: BTreeSet<ContactId> = self.devhub.links.values().map(|l| l.contact).collect();
        let wanted: Vec<ContactId> = self
            .contacts
            .iter()
            .filter(|(id, c)| c.prefs.use_as_hub && !online.contains(id))
            .map(|(id, _)| *id)
            .collect();
        let mut out = Vec::new();
        for contact in wanted {
            for cla in [ClaId::Iroh, ClaId::WifiDirect] {
                if let Some(peer) = self.peer_for(contact, cla)
                    && self.devhub.asked.insert(peer)
                {
                    out.push(Output::WantDeviceHub { peer, contact });
                }
            }
        }
        out
    }

    /// The channel was turned off: close every link.
    pub(super) fn close_device_hubs(&mut self) -> Vec<Output> {
        let peers: Vec<PeerHandle> = self.devhub.links.keys().copied().collect();
        peers
            .into_iter()
            .map(|peer| Output::CloseDeviceHub { peer })
            .collect()
    }

    /// An encounter ended: it may be dialled again in a later one.
    pub(super) fn device_hub_encounter_ended(&mut self, peer: PeerHandle) {
        self.devhub.asked.remove(&peer);
    }

    /// Remembers a purge token to pass on to the device hubs we use.
    pub(super) fn note_device_hub_token(&mut self, token: [u8; 16]) {
        if self.devhub.tokens.len() >= RECENT_TOKENS {
            self.devhub.tokens.pop_front();
        }
        self.devhub.tokens.push_back(token);
    }

    pub(super) fn device_hub_up(
        &mut self,
        peer: PeerHandle,
        contact: ContactId,
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        if !self.uses_device_hub(contact) || self.disabled.contains(&ClaId::DeviceHub) {
            return alloc::vec![Output::CloseDeviceHub { peer }];
        }
        let now = mem_crypto::contact::epoch_for_unix_ms(self.now_unix_ms);
        let oldest = now.saturating_sub(super::RECEIVE_TAG_DAYS);
        let mut plan = VecDeque::new();
        for chunk in self
            .receiving_prefixes(oldest..=now.saturating_add(1))
            .chunks(super::hub::REAL_PER_BATCH)
        {
            plan.push_back(super::hub::padded(chunk, rng));
        }
        // The live set last, so the hub pushes what arrives for today's tags while connected.
        let mut live = self.receiving_prefixes(now.saturating_sub(1)..=now.saturating_add(1));
        live.truncate(super::hub::REAL_PER_BATCH);
        if !live.is_empty() {
            plan.push_back(super::hub::padded(&live, rng));
        }
        self.devhub.links.insert(
            peer,
            Link {
                contact,
                uploaded: BTreeSet::new(),
                plan,
                fetching: false,
            },
        );
        let mut out = alloc::vec![Output::ReachabilityChanged];
        out.extend(self.next_device_hub_fetch(peer));
        out.extend(self.device_hub_purge(peer));
        out.extend(self.flush_device_hub(peer));
        out
    }

    pub(super) fn device_hub_down(&mut self, peer: PeerHandle) -> Vec<Output> {
        if self.devhub.links.remove(&peer).is_none() {
            return Vec::new();
        }
        self.forget_peer_ops(peer);
        alloc::vec![Output::ReachabilityChanged]
    }

    pub(super) fn device_hub_fetch_done(&mut self, peer: PeerHandle) -> Vec<Output> {
        if let Some(l) = self.devhub.links.get_mut(&peer) {
            l.fetching = false;
        }
        self.next_device_hub_fetch(peer)
    }

    pub(super) fn device_hub_bundles(
        &mut self,
        peer: PeerHandle,
        bundles: Vec<Vec<u8>>,
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        if !self.devhub.links.contains_key(&peer) {
            return Vec::new();
        }
        let mut out = Vec::new();
        for b in bundles {
            // Prefix buckets bring other people's bundles too: drop those quietly.
            out.extend(
                self.received_as(ClaId::DeviceHub, ClaId::DeviceHub.grade(), &b, rng)
                    .into_iter()
                    .filter(|o| {
                        !matches!(
                            o,
                            Output::Rejected(
                                RejectReason::NotForUs
                                    | RejectReason::Purged
                                    | RejectReason::Duplicate
                            )
                        )
                    }),
            );
        }
        out.extend(self.device_hub_purge(peer));
        out
    }

    /// Uploads on every link (new messages, ticks).
    pub(super) fn flush_device_hubs(&mut self) -> Vec<Output> {
        let peers: Vec<PeerHandle> = self.devhub.links.keys().copied().collect();
        peers
            .into_iter()
            .flat_map(|p| self.flush_device_hub(p))
            .collect()
    }

    /// Uploads to one hub every own bundle it does not hold yet that may travel by couriers
    /// (one-copy items such as call setup and shared files never do) and that the gate allows
    /// without asking.
    fn flush_device_hub(&mut self, peer: PeerHandle) -> Vec<Output> {
        let Some(link) = self.devhub.links.get(&peer) else {
            return Vec::new();
        };
        let due: Vec<MsgId> = self
            .outbox
            .iter()
            .filter(|(id, item)| {
                item.copies > 1
                    && item.contact != link.contact
                    && !link.uploaded.contains(id)
                    && !self
                        .ops
                        .values()
                        .any(|op| op.is_upload(**id, ClaId::DeviceHub))
            })
            .map(|(id, _)| *id)
            .collect();
        let mut out = Vec::new();
        for message in due {
            if let Ok(auth) = self.authorize(message, ClaId::DeviceHub, Ask::Never) {
                out.extend(self.transmit(auth, Some(peer)));
            }
        }
        out
    }

    /// The hub took (or refused) an upload.
    pub(super) fn device_hub_uploaded(&mut self, peer: Option<PeerHandle>, message: MsgId) {
        if let Some(l) = peer.and_then(|p| self.devhub.links.get_mut(&p)) {
            l.uploaded.insert(message);
        }
    }

    fn next_device_hub_fetch(&mut self, peer: PeerHandle) -> Vec<Output> {
        let Some(l) = self.devhub.links.get_mut(&peer) else {
            return Vec::new();
        };
        if l.fetching {
            return Vec::new();
        }
        match l.plan.pop_front() {
            Some(prefixes) => {
                l.fetching = true;
                alloc::vec![Output::DeviceHubFetch { peer, prefixes }]
            }
            None => Vec::new(),
        }
    }

    fn device_hub_purge(&self, peer: PeerHandle) -> Vec<Output> {
        if self.devhub.tokens.is_empty() {
            return Vec::new();
        }
        alloc::vec![Output::DeviceHubPurge {
            peer,
            tokens: self.devhub.tokens.iter().copied().collect(),
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::Input;
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    const NOW: u64 = 1_792_022_400_000;
    const HUB_KEY: [u8; 32] = [7; 32];

    struct Node {
        a: Agent,
        rng: ChaCha20Rng,
    }

    impl Node {
        fn new(seed: u8) -> Self {
            let mut rng = ChaCha20Rng::from_seed([seed; 32]);
            let mut a = Agent::new(NOW, &mut rng);
            let _ = a.handle(Input::Tick { wall_unix_ms: NOW }, &mut rng);
            Self { a, rng }
        }

        fn run(&mut self, input: Input) -> Vec<Output> {
            self.a.handle(input, &mut self.rng)
        }

        fn id(&self) -> ContactId {
            ContactId(*self.a.identity.public().as_bytes())
        }
    }

    fn pair(x: &mut Node, y: &mut Node) {
        let (cx, cy) = (x.a.own_card().unwrap(), y.a.own_card().unwrap());
        let _ = x.run(Input::ImportCard {
            cla: ClaId::Qr,
            bytes: cy,
        });
        let _ = y.run(Input::ImportCard {
            cla: ClaId::Qr,
            bytes: cx,
        });
    }

    /// Hub owner H, client C, recipient R, all contacts of each other.
    fn three() -> (Node, Node, Node) {
        let (mut h, mut c, mut r) = (Node::new(1), Node::new(2), Node::new(3));
        pair(&mut h, &mut c);
        pair(&mut h, &mut r);
        pair(&mut c, &mut r);
        (h, c, r)
    }

    #[test]
    fn login_needs_serving_the_users_choice_and_a_real_contact() {
        let (mut h, mut c, _) = three();
        let (sn, cn) = ([1; 16], [2; 16]);
        // Off until C chooses H's device as a hub.
        assert!(c.a.device_hub_proofs(h.id(), &HUB_KEY, &sn, &cn).is_none());
        let _ = c.run(Input::SetUseDeviceHub {
            contact: h.id(),
            enabled: true,
        });
        let p = c.a.device_hub_proofs(h.id(), &HUB_KEY, &sn, &cn).unwrap();
        // H does not serve yet.
        assert!(
            h.a.device_hub_login(&HUB_KEY, &sn, &cn, &p.client)
                .is_none()
        );
        let _ = h.run(Input::SetDeviceHubSettings(DeviceHubSettings {
            serve: true,
            ..DeviceHubSettings::default()
        }));
        let (who, answer) = h.a.device_hub_login(&HUB_KEY, &sn, &cn, &p.client).unwrap();
        assert_eq!(who, c.id());
        assert_eq!(answer, p.hub);
        // Relayed to another hub key, or replayed with other nonces: refused.
        assert!(
            h.a.device_hub_login(&[8; 32], &sn, &cn, &p.client)
                .is_none()
        );
        assert!(
            h.a.device_hub_login(&HUB_KEY, &[9; 16], &cn, &p.client)
                .is_none()
        );
        // The hub's answer never passes as a client proof.
        assert!(h.a.device_hub_login(&HUB_KEY, &sn, &cn, &p.hub).is_none());
        // A stranger's proof is refused.
        let mut stranger = Node::new(4);
        pair(&mut stranger, &mut Node::new(5));
        let _ = stranger.run(Input::SetUseDeviceHub {
            contact: h.id(),
            enabled: true,
        });
        assert!(
            stranger
                .a
                .device_hub_proofs(h.id(), &HUB_KEY, &sn, &cn)
                .is_none()
        );
        // Settings stay within bounds and survive a snapshot.
        let _ = h.run(Input::SetDeviceHubSettings(DeviceHubSettings {
            serve: true,
            total_bytes: 10,
            per_contact_bytes: u64::MAX,
        }));
        let s = h.a.device_hub_settings();
        assert_eq!(s.total_bytes, MIN_HUB_BYTES);
        assert_eq!(s.per_contact_bytes, MIN_HUB_BYTES);
        let snap = h.a.snapshot();
        let back = Agent::restore(&snap, NOW).unwrap();
        assert_eq!(back.device_hub_settings(), s);
    }

    fn uploads(out: &[Output], link: PeerHandle) -> Vec<(crate::model::OpId, Vec<u8>)> {
        out.iter()
            .filter_map(|o| match o {
                Output::Transmit {
                    op,
                    cla: ClaId::DeviceHub,
                    peer: Some(p),
                    bytes,
                    ..
                } if *p == link => Some((*op, bytes.clone())),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_client_uploads_its_bundles_and_the_recipient_fetches_them_as_p3() {
        let (h, mut c, mut r) = three();
        for n in [&mut c, &mut r] {
            let _ = n.run(Input::SetUseDeviceHub {
                contact: h.id(),
                enabled: true,
            });
        }
        // C writes to R, and to H (H's own messages never go to H's hub).
        let _ = c.run(Input::UserSend {
            contact: r.id(),
            text: "through Olena's tablet".into(),
        });
        let _ = c.run(Input::UserSend {
            contact: h.id(),
            text: "for the hub owner".into(),
        });
        let link = PeerHandle(1 << 40);
        let out = c.run(Input::DeviceHubUp {
            peer: link,
            contact: h.id(),
        });
        assert!(
            out.iter()
                .any(|o| matches!(o, Output::DeviceHubFetch { peer, .. } if *peer == link))
        );
        let sent = uploads(&out, link);
        assert_eq!(sent.len(), 1, "only the message to R");
        let (op, bytes) = sent[0].clone();
        let _ = c.run(Input::TransmitResult { op, result: Ok(()) });
        let m = c.a.messages(r.id());
        assert_eq!(m[0].channels[0].cla, ClaId::DeviceHub);
        assert_eq!(m[0].channels[0].grade, crate::PrivacyGrade::P3);
        // Not uploaded twice.
        assert_eq!(
            uploads(&c.run(Input::Tick { wall_unix_ms: NOW }), link).len(),
            0
        );

        // R logs in later and fetches it.
        let rlink = PeerHandle((1 << 40) + 1);
        let _ = r.run(Input::DeviceHubUp {
            peer: rlink,
            contact: h.id(),
        });
        let _ = r.run(Input::DeviceHubBundles {
            peer: rlink,
            bundles: alloc::vec![bytes],
        });
        let got = r.a.messages(c.id());
        assert_eq!(got[0].text, "through Olena's tablet");
        assert_eq!(got[0].channels[0].cla, ClaId::DeviceHub);
        assert_eq!(got[0].channels[0].grade, crate::PrivacyGrade::P3);
    }

    #[test]
    fn one_copy_items_and_stricter_floors_never_reach_a_device_hub() {
        let (h, mut c, r) = three();
        let _ = c.run(Input::SetUseDeviceHub {
            contact: h.id(),
            enabled: true,
        });
        // R's floor is stricter than P3: nothing for R goes to a device hub without consent.
        let _ = c.run(Input::SetFloor {
            contact: r.id(),
            floor: crate::PrivacyGrade::P2,
        });
        let _ = c.run(Input::UserSend {
            contact: r.id(),
            text: "stays".into(),
        });
        let link = PeerHandle(1 << 40);
        let out = c.run(Input::DeviceHubUp {
            peer: link,
            contact: h.id(),
        });
        assert_eq!(uploads(&out, link).len(), 0);
        // Back to the default floor: a call setup to R (one copy) still never goes there.
        let _ = c.run(Input::SetFloor {
            contact: r.id(),
            floor: crate::PrivacyGrade::P3,
        });
        let before = uploads(&c.run(Input::Tick { wall_unix_ms: NOW }), link).len();
        assert_eq!(before, 1, "the text, now within the floor");
        let _ = c.run(Input::SendCallSignal {
            contact: r.id(),
            signal: mem_proto::call::CallSignal::End {
                call_id: [1; 16],
                reason: mem_proto::call::EndReason::HungUp,
            },
        });
        assert_eq!(
            uploads(&c.run(Input::Tick { wall_unix_ms: NOW }), link).len(),
            0
        );
        // Turning the hub off for H closes the link.
        let out = c.run(Input::SetUseDeviceHub {
            contact: h.id(),
            enabled: false,
        });
        assert!(out.contains(&Output::CloseDeviceHub { peer: link }));
    }
}
