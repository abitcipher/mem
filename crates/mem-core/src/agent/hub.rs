//! The hub channel, sans-IO.
//!
//! The runtime keeps one connection to the hub while the channel is on; the agent decides what
//! goes over it:
//!
//! - **Uploads** are ordinary authorized transmits on [`ClaId::Hub`] (P2, within the default
//!   floor), so the consent gate, the privacy log and provenance apply unchanged.
//!   A bundle the hub accepted is not uploaded again; a resealed one is.
//! - **Fetching** reveals only 2-byte tag prefixes, padded with random decoys to a power of two.
//! On connecting, a backlog pass covers every day since the last completed
//!   fetch (at most the 31-day receive window), in batches; then a live set of the days around
//!   now stays active. The pass repeats hourly and when the day changes.
//! - **Purge tokens** of every bundle we tombstone go to the hub too, which is
//!   the only way it deletes anything.
//! - **Partner operators**: an upload may cross to partner operators' hubs only
//!   when the verified list names partners, "Only my operator's hubs" is off, and P3 is within
//!   the contact's floor; it is then recorded as P3. A bundle the hub marks as from
//!   another operator is recorded as P3 on arrival.

use alloc::collections::{BTreeSet, VecDeque};
use alloc::string::String;
use alloc::vec::Vec;

use mem_cla::{ClaId, PrivacyGrade};
use mem_crypto::contact::epoch_for_unix_ms;
use mem_crypto::intro::{intro_tag, week_for_unix_ms};
use mem_proto::encounter::MAX_PURGE_TOKENS;
use mem_proto::hub::{Crossing, HubOrigin, MAX_PREFIXES, MIN_PREFIXES};
use rand_core::CryptoRng;

use super::{Agent, Ask, Output, RECEIVE_TAG_DAYS, StorageOp};
use crate::model::{MsgId, RejectReason};

/// Real prefixes per fetch batch; decoys fill the rest up to a power of two (at most 64).
const REAL_PER_BATCH: usize = MAX_PREFIXES / 2;
/// How often the backlog pass repeats while connected.
const REFETCH_EVERY_MS: u64 = 60 * 60 * 1000;
/// Purge tokens kept for the hub while it is unreachable.
const MAX_QUEUED_TOKENS: usize = MAX_PURGE_TOKENS * 4;

/// Hub state. Volatile except `last_fetch_epoch` and the token queue (persisted).
#[derive(Default)]
pub(crate) struct HubState {
    pub(crate) online: bool,
    /// Fetch batches still to send; the last one is the live set.
    plan: VecDeque<Vec<[u8; 2]>>,
    /// Waiting for `fetch_done` of the batch in flight.
    fetching: bool,
    /// Epoch through which every day was fetched completely.
    pub(crate) last_fetch_epoch: u32,
    /// The epoch the plan in flight covers up to (recorded once it completes).
    plan_epoch: u32,
    last_plan_ms: u64,
    /// Purge tokens to hand to the hub.
    pub(crate) tokens: VecDeque<[u8; 16]>,
    /// Messages whose current bundle the hub accepted.
    pub(crate) uploaded: BTreeSet<MsgId>,
    /// The push endpoint registered on this connection.
    registered: Option<String>,
    /// How many contacts the last plan covered: a new contact needs a new plan at once.
    planned_contacts: usize,
}

/// Longest push endpoint (as `mem_proto::hub::MAX_ENDPOINT_LEN`).
const MAX_ENDPOINT_LEN: usize = mem_proto::hub::MAX_ENDPOINT_LEN;

/// A push endpoint the hub may POST to: `https://`, or `http://` to this machine (a local
/// distributor in development). Anything else could point the hub at private networks.
pub(crate) fn valid_endpoint(url: &str) -> bool {
    if url.len() > MAX_ENDPOINT_LEN || url.chars().any(char::is_whitespace) {
        return false;
    }
    if let Some(rest) = url.strip_prefix("https://") {
        return rest.split('/').next().is_some_and(|h| !h.is_empty());
    }
    url.strip_prefix("http://").is_some_and(|rest| {
        let host = rest.split(['/', ':']).next().unwrap_or("");
        matches!(host, "localhost" | "127.0.0.1")
    })
}

/// Longest hub address.
pub(crate) const MAX_HUB_URL_LEN: usize = 256;

/// A hub address the runtime may connect to: `wss://` (TLS), or plain `ws://` only to this
/// machine (a development hub; nothing leaves the device unencrypted on the network).
pub(crate) fn valid_url(url: &str) -> bool {
    if url.len() > MAX_HUB_URL_LEN || url.chars().any(char::is_whitespace) {
        return false;
    }
    let rest = if let Some(r) = url.strip_prefix("wss://") {
        r
    } else if let Some(r) = url.strip_prefix("ws://") {
        let loopback_v6 = r
            .strip_prefix("[::1]")
            .is_some_and(|t| t.is_empty() || t.starts_with([':', '/']));
        let host = r.split(['/', ':']).next().unwrap_or("");
        if !loopback_v6 && !matches!(host, "localhost" | "127.0.0.1") {
            return false;
        }
        r
    } else {
        return false;
    };
    let host = rest.split('/').next().unwrap_or("");
    !host.is_empty()
}

/// Checks a hub list against the operator keys built into the app: signature,
/// not expired at `now_unix_ms`, and well-formed URLs.
///
/// # Security
///
/// Only a list signed by a pinned operator key is ever used to pick hubs; a list from a hub is
/// otherwise just untrusted bytes.
#[must_use]
pub(crate) fn verify_hub_list(
    bytes: &[u8],
    operator_keys: &[[u8; 32]],
    now_unix_ms: u64,
) -> Option<mem_proto::hublist::HubList> {
    use mem_proto::hublist::{HUB_LIST_DOMAIN, HubList};
    let list = HubList::decode(bytes).ok()?;
    if list.expires_unix_ms <= now_unix_ms || !list.hubs.iter().all(|h| valid_url(&h.url)) {
        return None;
    }
    let signed = list.signed_bytes().ok()?;
    operator_keys
        .iter()
        .filter_map(|k| mem_crypto::IdentityPublic::from_bytes(k).ok())
        .any(|k| {
            k.verify(HUB_LIST_DOMAIN, &[&signed], &list.signature)
                .is_ok()
        })
        .then_some(list)
}

/// The grade of a send over `cla`: a hub upload that may cross to partner operators is P3
/// (couriers we do not run), otherwise the channel's own grade.
pub(crate) fn send_grade(cla: ClaId, crossing: Option<Crossing>) -> PrivacyGrade {
    match crossing {
        Some(Crossing::Partners) => PrivacyGrade::P3,
        _ => cla.grade(),
    }
}

/// The grade of a bundle that arrived from the hub.
const fn arrival_grade(origin: HubOrigin) -> PrivacyGrade {
    match origin {
        HubOrigin::OwnOperator => PrivacyGrade::P2,
        HubOrigin::OtherOperator => PrivacyGrade::P3,
    }
}

impl HubState {
    /// The channel was turned off: forget the connection, queued tokens and uploads; keep how
    /// far fetching got.
    pub(crate) fn reset(&mut self) {
        *self = Self {
            last_fetch_epoch: self.last_fetch_epoch,
            ..Self::default()
        };
    }
}

impl Agent {
    /// The hub's address, if set.
    #[must_use]
    pub fn hub_url(&self) -> Option<&str> {
        self.hub_url.as_deref()
    }

    /// Hubs to try, in order: the user's own address if set, otherwise the
    /// verified list starting at this device's random pick, so devices spread over the hubs and
    /// each stays with one while it works.
    #[must_use]
    pub fn hub_candidates(&self) -> Vec<String> {
        if let Some(u) = &self.hub_url {
            return alloc::vec![u.clone()];
        }
        let Some(list) = &self.hub_list else {
            return Vec::new();
        };
        let n = list.hubs.len();
        let start = usize::try_from(self.hub_pick).unwrap_or(0) % n.max(1);
        (0..n)
            .map(|i| list.hubs[(start + i) % n].url.clone())
            .collect()
    }

    /// The verified hub list, if any.
    #[must_use]
    pub const fn hub_list(&self) -> Option<&mem_proto::hublist::HubList> {
        self.hub_list.as_ref()
    }

    /// Partner operators' hubs named by the verified list, none without a list.
    #[must_use]
    pub fn hub_partners(&self) -> &[mem_proto::hublist::PartnerEntry] {
        self.hub_list
            .as_ref()
            .map_or(&[], |l| l.partners.as_slice())
    }

    /// Whether "Only my operator's hubs" is on.
    #[must_use]
    pub const fn own_operator_only(&self) -> bool {
        self.own_operator_only
    }

    /// Whether an upload for `contact` may cross to partner operators: only
    /// with partners listed, the user's switch off, and P3 within the contact's floor (a
    /// stricter contact is never crossed without asking).
    pub(super) fn hub_crossing(&self, contact: crate::model::ContactId) -> Crossing {
        let floor = self
            .contacts
            .get(&contact)
            .map_or(self.default_floor, |c| c.floor);
        if !self.own_operator_only
            && !self.hub_partners().is_empty()
            && PrivacyGrade::P3.within(floor)
        {
            Crossing::Partners
        } else {
            Crossing::OwnOperator
        }
    }

    /// Takes a hub list (from a hub or built into the app) if it verifies and is newer.
    pub(super) fn hub_list_received(&mut self, bytes: &[u8]) -> Vec<Output> {
        let Some(list) = verify_hub_list(bytes, &self.operator_keys, self.now_unix_ms) else {
            return Vec::new();
        };
        if self
            .hub_list
            .as_ref()
            .is_some_and(|old| old.serial >= list.serial && old.expires_unix_ms > self.now_unix_ms)
        {
            return Vec::new();
        }
        self.hub_list = Some(list);
        alloc::vec![
            Output::HubCandidatesChanged,
            Output::Persist(StorageOp::SaveSnapshot),
        ]
    }

    /// The push endpoint, if set.
    #[must_use]
    pub fn push_endpoint(&self) -> Option<&str> {
        self.push_endpoint.as_deref()
    }

    /// Whether the hub channel is connected now.
    #[must_use]
    pub const fn hub_online(&self) -> bool {
        self.hub.online
    }

    /// Dispatches the hub inputs.
    pub(super) fn hub_input(
        &mut self,
        input: crate::Input,
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        use crate::Input;
        match input {
            Input::HubConnected => {
                if self.disabled.contains(&ClaId::Hub) {
                    return Vec::new();
                }
                self.hub.online = true;
                self.hub.fetching = false;
                let since = self.hub.last_fetch_epoch;
                let mut out = alloc::vec![Output::ReachabilityChanged];
                // Ask for the current list only when the app can check it.
                if !self.operator_keys.is_empty() {
                    out.push(Output::HubGetList);
                }
                out.extend(self.plan_fetch(since, rng));
                out.extend(self.hub_purge());
                out.extend(self.flush_hub());
                out
            }
            Input::HubDisconnected => {
                self.hub.registered = None;
                let was = core::mem::replace(&mut self.hub.online, false);
                self.hub.fetching = false;
                self.hub.plan.clear();
                if was {
                    alloc::vec![Output::ReachabilityChanged]
                } else {
                    Vec::new()
                }
            }
            Input::HubFetchDone => {
                self.hub.fetching = false;
                if self.hub.plan.is_empty() {
                    // The live set is active and everything before it was fetched.
                    if self.hub.plan_epoch > self.hub.last_fetch_epoch {
                        self.hub.last_fetch_epoch = self.hub.plan_epoch;
                        return alloc::vec![Output::Persist(StorageOp::SaveSnapshot)];
                    }
                    return Vec::new();
                }
                self.next_fetch()
            }
            Input::HubBundles { bundles } => {
                let mut out = Vec::new();
                for (b, origin) in bundles {
                    let grade = arrival_grade(origin);
                    // Prefix buckets bring other people's bundles too: drop those quietly.
                    out.extend(
                        self.received_as(ClaId::Hub, grade, &b, rng)
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
                out.extend(self.hub_purge());
                out
            }
            _ => Vec::new(),
        }
    }

    /// While connected: upload what the hub does not have yet, refetch hourly and at day change,
    /// and pass on new purge tokens. Called on every tick.
    pub(super) fn hub_tick(&mut self, rng: &mut impl CryptoRng) -> Vec<Output> {
        if !self.hub.online {
            return Vec::new();
        }
        let mut out = Vec::new();
        let epoch = epoch_for_unix_ms(self.now_unix_ms);
        let stale = self.now_unix_ms.saturating_sub(self.hub.last_plan_ms) >= REFETCH_EVERY_MS
            || epoch > self.hub.plan_epoch
            || self.contacts.len() != self.hub.planned_contacts;
        if stale && !self.hub.fetching && self.hub.plan.is_empty() {
            out.extend(self.plan_fetch(epoch.saturating_sub(1), rng));
        }
        out.extend(self.hub_purge());
        out.extend(self.flush_hub());
        out
    }

    /// Uploads every outbox item the hub does not hold yet, if the gate allows it without
    /// asking (the hub is P2, within the default floor).
    pub(super) fn flush_hub(&mut self) -> Vec<Output> {
        if !self.hub.online || self.disabled.contains(&ClaId::Hub) {
            return Vec::new();
        }
        let due: Vec<MsgId> = self
            .outbox
            .keys()
            .filter(|id| !self.hub.uploaded.contains(id))
            .filter(|id| !self.ops.values().any(|op| op.is_upload(**id, ClaId::Hub)))
            .copied()
            .collect();
        let mut out = Vec::new();
        for message in due {
            if let Ok(auth) = self.authorize(message, ClaId::Hub, Ask::Never) {
                out.extend(self.transmit(auth, None));
            }
        }
        out
    }

    /// Queues a purge token for the hub (every tombstone).
    pub(super) fn queue_hub_token(&mut self, token: [u8; 16]) {
        if self.disabled.contains(&ClaId::Hub) {
            return;
        }
        if self.hub.tokens.len() >= MAX_QUEUED_TOKENS {
            self.hub.tokens.pop_front();
        }
        self.hub.tokens.push_back(token);
    }

    fn hub_purge(&mut self) -> Vec<Output> {
        if !self.hub.online || self.hub.tokens.is_empty() {
            return Vec::new();
        }
        let n = self.hub.tokens.len().min(MAX_PURGE_TOKENS);
        let tokens: Vec<[u8; 16]> = self.hub.tokens.drain(..n).collect();
        alloc::vec![
            Output::HubPurge { tokens },
            Output::Persist(StorageOp::SaveSnapshot),
        ]
    }

    /// Plans fetches for every day from `since` (within the receive window) to tomorrow, then
    /// the live set, and sends the first batch.
    fn plan_fetch(&mut self, since: u32, rng: &mut impl CryptoRng) -> Vec<Output> {
        let now = epoch_for_unix_ms(self.now_unix_ms);
        let oldest = now.saturating_sub(RECEIVE_TAG_DAYS).max(since);
        let backlog = self.receiving_prefixes(oldest..=now.saturating_add(1));
        let live = self.receiving_prefixes(now.saturating_sub(1)..=now.saturating_add(1));
        // A live set that does not fit one fetch keeps only today (the backlog pass, repeated
        // hourly, covers the rest).
        let live = if live.len() > REAL_PER_BATCH {
            let mut today = self.receiving_prefixes(now..=now);
            today.truncate(REAL_PER_BATCH);
            today
        } else {
            live
        };
        self.hub.plan.clear();
        self.hub.planned_contacts = self.contacts.len();
        self.hub.last_plan_ms = self.now_unix_ms;
        self.hub.plan_epoch = now;
        // No contacts: nothing to fetch and nothing to be woken for (yet).
        if live.is_empty() {
            return Vec::new();
        }
        if backlog != live {
            for chunk in backlog.chunks(REAL_PER_BATCH) {
                self.hub.plan.push_back(padded(chunk, rng));
            }
        }
        let live = padded(&live, rng);
        // Wake-ups for the same prefixes the live fetch uses, renewed with every plan (at
        // least hourly; the hub forgets them after 48 h).
        let wake = self.push_endpoint.clone().map(|endpoint| {
            self.hub.registered = Some(endpoint.clone());
            Output::HubWakeRegister {
                endpoint,
                prefixes: live.clone(),
            }
        });
        self.hub.plan.push_back(live);
        self.hub.plan_epoch = now;
        self.hub.last_plan_ms = self.now_unix_ms;
        let mut out = self.next_fetch();
        out.extend(wake);
        out
    }

    /// Sets (or clears) the push endpoint. While connected, the old registration is
    /// withdrawn and the new one made at once.
    pub(super) fn set_push_endpoint(
        &mut self,
        endpoint: Option<String>,
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        let endpoint = endpoint.map(|e| String::from(e.trim()));
        if endpoint.as_deref().is_some_and(|e| !valid_endpoint(e)) {
            return alloc::vec![Output::Rejected(RejectReason::InvalidPushEndpoint)];
        }
        let mut out = alloc::vec![Output::Persist(StorageOp::SaveSnapshot)];
        if self.hub.online
            && let Some(old) = self.hub.registered.take()
            && Some(&old) != endpoint.as_ref()
        {
            out.push(Output::HubWakeUnregister { endpoint: old });
        }
        self.push_endpoint = endpoint;
        if self.push_endpoint.is_some() {
            out.extend(self.refetch(rng));
        }
        out
    }

    /// A new fetch plan at once while connected, when the prefixes we want changed (a push
    /// endpoint, contact requests switched on or off), instead of at the next reconnect.
    pub(super) fn refetch(&mut self, rng: &mut impl CryptoRng) -> Vec<Output> {
        if !self.hub.online || self.hub.fetching {
            return Vec::new();
        }
        let since = self.hub.last_fetch_epoch;
        self.plan_fetch(since, rng)
    }

    fn next_fetch(&mut self) -> Vec<Output> {
        match self.hub.plan.pop_front() {
            Some(prefixes) => {
                self.hub.fetching = true;
                alloc::vec![Output::HubFetch { prefixes }]
            }
            None => Vec::new(),
        }
    }

    /// Prefixes of our receiving tags (every contact, every day in `epochs`), and of our intro
    /// tag while contact requests are on.
    fn receiving_prefixes(&self, epochs: core::ops::RangeInclusive<u32>) -> Vec<[u8; 2]> {
        let me = self.identity.public();
        let mut out: BTreeSet<[u8; 2]> = BTreeSet::new();
        for e in epochs {
            for c in self.contacts.values() {
                let tag = self.my_tag(c, e);
                out.insert([tag[0], tag[1]]);
            }
        }
        if self.accept_intros {
            let tag = intro_tag(&me, week_for_unix_ms(self.now_unix_ms));
            out.insert([tag[0], tag[1]]);
        }
        out.into_iter().collect()
    }
}

/// Real prefixes plus random decoys, padded to a power of two between 8 and 64, sorted so the
/// order reveals nothing.
fn padded(real: &[[u8; 2]], rng: &mut impl CryptoRng) -> Vec<[u8; 2]> {
    let target = real
        .len()
        .next_power_of_two()
        .clamp(MIN_PREFIXES, MAX_PREFIXES);
    let mut set: BTreeSet<[u8; 2]> = real.iter().copied().collect();
    while set.len() < target {
        let mut p = [0u8; 2];
        rng.fill_bytes(&mut p);
        set.insert(p);
    }
    set.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::valid_url;

    #[test]
    fn hub_urls() {
        assert!(valid_url("wss://hub.example.org/v1"));
        assert!(valid_url("wss://hub.example.org:8443/v1"));
        assert!(valid_url("ws://127.0.0.1:4000/v1"));
        assert!(valid_url("ws://localhost:4000/v1"));
        assert!(valid_url("ws://[::1]:4000/v1"));
        // Plain text over a real network never.
        assert!(!valid_url("ws://hub.example.org/v1"));
        assert!(!valid_url("ws://192.168.1.5/v1"));
        assert!(!valid_url("ws://[::2]:4000/v1"));
        assert!(!valid_url("ws://[::1]x/v1"));
        assert!(!valid_url("ws://localhost.evil.example/v1"));
        assert!(!valid_url("https://hub.example.org/v1"));
        assert!(!valid_url("wss:///v1"));
        assert!(!valid_url("wss://hub example/v1"));
        assert!(!valid_url(&alloc::format!("wss://{}", "a".repeat(300))));
    }

    #[test]
    fn push_endpoints() {
        use super::valid_endpoint;
        assert!(valid_endpoint("https://ntfy.sh/upAbC123?up=1"));
        assert!(valid_endpoint("http://127.0.0.1:8080/up"));
        assert!(!valid_endpoint("http://ntfy.sh/up"));
        assert!(!valid_endpoint("http://10.0.0.1/up"));
        assert!(!valid_endpoint("https:///up"));
        assert!(!valid_endpoint("ftp://x/y"));
    }
}
