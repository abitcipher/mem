//! Cross-crate invariant tests and a two-agent test harness.
//!
//! Each invariant has its own test file under `tests/`. Missing coverage for any invariant is
//! a release blocker.

use std::collections::BTreeMap;

use mem_core::courier::{CourierKey, CourierMeta, StoreWrite, TombstoneRow};
use mem_core::model::{ContactId, MsgId};
use mem_core::{Agent, ClaId, Input, Output, StorageOp};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

/// Fixed wall clock for tests: 2026-10-15T00:00:00Z.
pub const NOW: u64 = 1_792_022_400_000;

/// One simulated device: an agent plus its deterministic RNG.
/// An in-memory stand-in for the runtime's database (`mem-store`): applies the same writes.
#[derive(Default)]
pub struct MemStore {
    /// Carried bundles with their metadata.
    pub courier: BTreeMap<CourierKey, (CourierMeta, Vec<u8>)>,
    /// Purge tombstones by commitment.
    pub tombstones: BTreeMap<[u8; 16], TombstoneRow>,
}

impl MemStore {
    /// Applies every `Persist(Store(..))` in `outputs`, in order.
    pub fn apply(&mut self, outputs: &[Output]) {
        for o in outputs {
            let Output::Persist(StorageOp::Store(writes)) = o else {
                continue;
            };
            for w in writes {
                match w {
                    StoreWrite::Put { meta, bytes } => {
                        self.courier.insert(meta.key, (meta.clone(), bytes.clone()));
                    }
                    StoreWrite::Delete(keys) => {
                        for k in keys {
                            self.courier.remove(k);
                        }
                    }
                    StoreWrite::SetCopies { key, copies } => {
                        if let Some((m, _)) = self.courier.get_mut(key) {
                            m.copies = *copies;
                        }
                    }
                    StoreWrite::TombstonePut(t) => {
                        self.tombstones.insert(t.commitment, t.clone());
                    }
                    StoreWrite::TombstoneDelete(cs) => {
                        for c in cs {
                            self.tombstones.remove(c);
                        }
                    }
                }
            }
        }
    }
}

/// One simulated device: an agent with injected time, randomness and database.
pub struct Device {
    /// The agent under test.
    pub agent: Agent,
    /// Deterministic randomness.
    pub rng: ChaCha20Rng,
    /// Injected wall time.
    pub now: u64,
    /// What the runtime would hold in its database.
    pub store: MemStore,
}

impl Device {
    /// A fresh device seeded with `seed`.
    #[must_use]
    pub fn new(seed: u8) -> Self {
        let mut rng = ChaCha20Rng::from_seed([seed; 32]);
        let agent = Agent::new(NOW, &mut rng);
        Self {
            agent,
            rng,
            now: NOW,
            store: MemStore::default(),
        }
    }

    /// Restarts like the runtime does: state from the snapshot, then the database contents.
    ///
    /// # Errors
    ///
    /// If the snapshot does not restore.
    pub fn restart(&mut self) -> Result<Vec<Output>, mem_core::SnapshotError> {
        let snap = self.agent.snapshot();
        self.agent = Agent::restore(&snap, self.now)?;
        let courier = self
            .store
            .courier
            .values()
            .map(|(m, _)| m.clone())
            .collect();
        let tombstones = self.store.tombstones.values().cloned().collect();
        Ok(self.handle(Input::StoreLoaded {
            courier,
            tombstones,
        }))
    }

    /// Ticks the clock, then handles `input`. Returns the outputs of both.
    pub fn handle(&mut self, input: Input) -> Vec<Output> {
        let mut out = self.agent.handle(
            Input::Tick {
                wall_unix_ms: self.now,
            },
            &mut self.rng,
        );
        out.extend(self.agent.handle(input, &mut self.rng));
        self.store.apply(&out);
        self.solve_pow(&mut out);
        out
    }

    /// Does what the runtime does with `SolvePow`: finds a nonce and hands it back.
    fn solve_pow(&mut self, out: &mut Vec<Output>) {
        let mut i = 0;
        while i < out.len() {
            if let Output::SolvePow(job) = out[i]
                && let Some(nonce) = mem_crypto::intro::pow_solve(
                    &job.bundle_id,
                    &job.dst_tag,
                    job.week,
                    job.difficulty,
                    0,
                    100_000,
                )
            {
                let more = self.agent.handle(
                    Input::PowSolved {
                        message: job.message,
                        nonce,
                    },
                    &mut self.rng,
                );
                self.store.apply(&more);
                out.extend(more);
            }
            i += 1;
        }
    }

    /// Sends a contact request to `other` from its card and returns the requested
    /// contact's id.
    pub fn request(&mut self, other: &Self, note: Option<&str>) -> Option<ContactId> {
        self.request_card(other.card(), note)
    }

    /// Sends a contact request from card bytes (possibly an old card).
    pub fn request_card(&mut self, card: Vec<u8>, note: Option<&str>) -> Option<ContactId> {
        self.handle(Input::SendContactRequest {
            card,
            note: note.map(Into::into),
        })
        .into_iter()
        .find_map(|o| match o {
            Output::ContactPaired { contact, .. } => Some(contact),
            _ => None,
        })
    }

    /// Moves this device's clock forward.
    pub fn advance(&mut self, ms: u64) {
        self.now += ms;
    }

    /// Runs one clock tick (timers: retransmission, standalone acks, expiry) and returns its outputs.
    pub fn tick(&mut self) -> Vec<Output> {
        let mut out = self.agent.handle(
            Input::Tick {
                wall_unix_ms: self.now,
            },
            &mut self.rng,
        );
        self.store.apply(&out);
        self.solve_pow(&mut out);
        out
    }

    /// Our pairing card bytes.
    ///
    /// # Panics
    ///
    /// If the card cannot be built (test harness only).
    #[must_use]
    pub fn card(&self) -> Vec<u8> {
        self.agent.own_card().unwrap_or_default()
    }

    /// Imports `other`'s card over `cla` and returns the new contact id.
    pub fn import(&mut self, other: &Self, cla: ClaId) -> Option<ContactId> {
        self.handle(Input::ImportCard {
            cla,
            bytes: other.card(),
        })
        .into_iter()
        .find_map(|o| match o {
            Output::ContactPaired { contact, .. } => Some(contact),
            _ => None,
        })
    }

    /// Queues a text message and returns its id.
    pub fn send(&mut self, contact: ContactId, text: &str) -> Option<MsgId> {
        self.handle(Input::UserSend {
            contact,
            text: text.into(),
        })
        .into_iter()
        .find_map(|o| match o {
            Output::Queued { message, .. } => Some(message),
            _ => None,
        })
    }
}

/// Pairs two devices in person (both scan each other's QR) and returns
/// (`a`'s id for `b`, `b`'s id for `a`).
pub fn pair(a: &mut Device, b: &mut Device, cla: ClaId) -> Option<(ContactId, ContactId)> {
    let b_in_a = a.import(b, cla)?;
    let a_in_b = b.import(a, cla)?;
    Some((b_in_a, a_in_b))
}

/// Extracts the bytes of the first `Transmit` output.
#[must_use]
pub fn transmitted(outputs: &[Output]) -> Option<(mem_core::model::OpId, Vec<u8>)> {
    outputs.iter().find_map(|o| match o {
        Output::Transmit { op, bytes, .. } => Some((*op, bytes.clone())),
        _ => None,
    })
}

/// The handle each side of a simulated link uses for the other.
pub const LINK_PEER: mem_core::model::PeerHandle = mem_core::model::PeerHandle(1);

/// Starts an encounter between `a` and `b` over a simulated network channel and runs it until
/// quiet. Returns every output each side produced.
pub fn encounter(a: &mut Device, b: &mut Device, cla: ClaId) -> (Vec<Output>, Vec<Output>) {
    let oa = a.handle(Input::EncounterStarted {
        peer: LINK_PEER,
        cla,
    });
    let ob = b.handle(Input::EncounterStarted {
        peer: LINK_PEER,
        cla,
    });
    pump(a, b, oa, ob)
}

/// Delivers encounter messages and network transmissions between `a` and `b` (through their
/// wire encodings) until neither side has anything left to send. `oa`/`ob` are outputs not yet
/// routed. Returns all outputs seen on each side, including the initial ones.
pub fn pump(
    a: &mut Device,
    b: &mut Device,
    oa: Vec<Output>,
    ob: Vec<Output>,
) -> (Vec<Output>, Vec<Output>) {
    let (mut all_a, mut all_b) = (Vec::new(), Vec::new());
    let (mut pa, mut pb) = (oa, ob);
    for _ in 0..64 {
        if pa.is_empty() && pb.is_empty() {
            break;
        }
        let (to_b, a_own) = route(&pa, &a.store);
        let (to_a, b_own) = route(&pb, &b.store);
        all_a.append(&mut pa);
        all_b.append(&mut pb);
        for input in a_own.into_iter().chain(to_a) {
            pa.extend(a.handle(input));
        }
        for input in b_own.into_iter().chain(to_b) {
            pb.extend(b.handle(input));
        }
    }
    (all_a, all_b)
}

/// Splits one side's outputs into inputs for the other side and results for itself, like the
/// runtime: proofs travel with direct sends, and `Forward` reads the sender's database.
#[must_use]
pub fn route(outputs: &[Output], store: &MemStore) -> (Vec<Input>, Vec<Input>) {
    use mem_core::EncounterMsg;
    use mem_proto::encounter::BundleEntry;
    let (mut remote, mut local) = (Vec::new(), Vec::new());
    let wire = |msg: &EncounterMsg| {
        msg.encode()
            .ok()
            .and_then(|bytes| EncounterMsg::decode(&bytes).ok())
    };
    for o in outputs {
        match o {
            Output::Encounter { msg, .. } => {
                if let Some(msg) = wire(msg) {
                    remote.push(Input::EncounterMessage {
                        peer: LINK_PEER,
                        msg,
                    });
                }
            }
            Output::Forward {
                key,
                hops,
                copies,
                age_ms,
                ..
            } => {
                let relayed = store.courier.get(key).and_then(|(_, stored)| {
                    mem_proto::Bundle::relayed(stored, *hops, *copies, *age_ms).ok()
                });
                if let Some(msg) =
                    relayed.and_then(|b| wire(&EncounterMsg::Bundles(vec![BundleEntry::bare(b)])))
                {
                    remote.push(Input::EncounterMessage {
                        peer: LINK_PEER,
                        msg,
                    });
                }
            }
            Output::Transmit {
                op,
                peer: Some(_),
                bytes,
                proof,
                ..
            } => {
                if let Some(msg) = wire(&EncounterMsg::Bundles(vec![BundleEntry {
                    bytes: bytes.clone(),
                    proof: *proof,
                }])) {
                    remote.push(Input::EncounterMessage {
                        peer: LINK_PEER,
                        msg,
                    });
                }
                local.push(Input::TransmitResult {
                    op: *op,
                    result: Ok(()),
                });
            }
            Output::EndEncounter { .. } => remote.push(Input::EncounterEnded { peer: LINK_PEER }),
            _ => {}
        }
    }
    (remote, local)
}
