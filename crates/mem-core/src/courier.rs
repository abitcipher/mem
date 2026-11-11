//! The courier store index: which other people's bundles this device carries, within
//! quotas, with priority classes and eviction.
//!
//! Sans-IO: this module keeps metadata only. Bundle bytes live in the runtime's encrypted
//! database; every change is returned as a [`StoreWrite`] for the runtime to apply.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use mem_cla::ClaId;
use mem_crypto::purge::PurgeCommitment;
use mem_proto::envelope::{Bundle, BundleKind};

/// Default total courier storage for new installs.
pub const DEFAULT_COURIER_BYTES: u64 = 32 * 1024 * 1024;
/// Largest total courier storage the user can choose.
pub const MAX_COURIER_BYTES: u64 = 512 * 1024 * 1024;
/// Per destination tag.
pub const PER_TAG_BYTES: u64 = 512 * 1024;
/// All intro bundles together.
pub const INTRO_BYTES: u64 = 1024 * 1024;
/// Per encounter: peers are anonymous, so this replaces "per peer per day".
pub const PER_ENCOUNTER_BYTES: u64 = 2 * 1024 * 1024;
/// Per encounter, bundle count.
pub const PER_ENCOUNTER_BUNDLES: u32 = 200;
/// Per BLE encounter: links are slow and short.
pub const BLE_ENCOUNTER_BYTES: u64 = 1024 * 1024;
/// Per BLE encounter, bundle count.
pub const BLE_ENCOUNTER_BUNDLES: u32 = 64;
/// Most bytes from any one arrival encounter that we hand on as spray copies in one encounter
/// (deliveries to the recipient are not limited). A flood therefore mostly stays where it landed
/// instead of spreading through honest couriers (`mem-sim`).
pub const SPRAY_PER_BATCH_BYTES: u64 = 64 * 1024;

/// A courier entry's identity: dedup is on `(bundle_id, purge_commitment)`, so a junk bundle that
/// reuses a real bundle id with another commitment cannot block the real one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CourierKey {
    /// Envelope `bundle_id`.
    pub bundle_id: [u8; 16],
    /// Envelope `purge_commitment`.
    pub commitment: [u8; 16],
}

/// Priority class; a higher number is evicted first. Class 1 (own outbox)
/// never enters the courier store; class 3 is dropped in v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CourierClass {
    /// Addressed to one of our contacts' current receiving tags.
    ToContact = 2,
    /// Anything else.
    Other = 4,
    /// Contact requests (`intro` bundles).
    Intro = 5,
}

impl CourierClass {
    /// Stored form.
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// From the stored form; unknown values count as [`CourierClass::Other`].
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            2 => Self::ToContact,
            5 => Self::Intro,
            _ => Self::Other,
        }
    }
}

/// What the index knows about one carried bundle (no bytes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CourierMeta {
    /// Identity.
    pub key: CourierKey,
    /// Envelope destination tag.
    pub dst_tag: [u8; 16],
    /// Priority class.
    pub class: CourierClass,
    /// Encoded size in bytes, as accounted against quotas.
    pub size: u32,
    /// When we admitted it (wall clock, injected).
    pub admitted_unix_ms: u64,
    /// Envelope `age_ms` at admission; holding time is added on top.
    pub age_at_admission_ms: u64,
    /// Envelope `lifetime_s` in milliseconds.
    pub lifetime_ms: u64,
    /// Envelope `copies` this device holds.
    pub copies: u8,
    /// Envelope `hops_remaining` at admission.
    pub hops: u8,
    /// The encounter it arrived in (a local counter, never sent). Eviction prefers the encounter
    /// that contributed the most stored bytes.
    pub batch: u64,
}

impl CourierMeta {
    /// The bundle's age now, including how long we have held it.
    #[must_use]
    pub const fn age_ms(&self, now_unix_ms: u64) -> u64 {
        self.age_at_admission_ms
            .saturating_add(now_unix_ms.saturating_sub(self.admitted_unix_ms))
    }

    /// Whether its lifetime has run out.
    #[must_use]
    pub const fn expired(&self, now_unix_ms: u64) -> bool {
        self.age_ms(now_unix_ms) >= self.lifetime_ms
    }

    /// Eviction rank: the entry with the greatest rank goes first: expired, then lowest class,
    /// then from the encounter with the most stored bytes, then oldest, then largest.
    /// `batch_bytes` is what its encounter has stored.
    fn rank(&self, now: u64, batch_bytes: u64) -> (bool, CourierClass, u64, u64, u32) {
        (
            self.expired(now),
            self.class,
            batch_bytes,
            self.age_ms(now),
            self.size,
        )
    }
}

/// A change for the runtime to apply to the database (`mem-store`), in order, in one transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreWrite {
    /// Store a newly admitted bundle.
    Put {
        /// Metadata.
        meta: CourierMeta,
        /// The bundle exactly as received.
        bytes: Vec<u8>,
    },
    /// Forget these bundles.
    Delete(Vec<CourierKey>),
    /// Change how many spray copies we still hold of a carried bundle.
    SetCopies {
        /// Which bundle.
        key: CourierKey,
        /// Copies now held.
        copies: u8,
    },
    /// Add or refresh a purge tombstone.
    TombstonePut(TombstoneRow),
    /// Forget these tombstones (expired or evicted by quota).
    TombstoneDelete(Vec<[u8; 16]>),
}

/// A stored purge tombstone.
#[derive(Clone, PartialEq, Eq)]
pub struct TombstoneRow {
    /// Purge commitment.
    pub commitment: [u8; 16],
    /// The released token (gossiped on).
    pub token: [u8; 16],
    /// When it may be forgotten.
    pub expires_unix_ms: u64,
}

impl core::fmt::Debug for TombstoneRow {
    /// Never prints the token or the full commitment.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("TombstoneRow")
            .field("expires_unix_ms", &self.expires_unix_ms)
            .finish_non_exhaustive()
    }
}

/// Why a bundle was not admitted. Nothing is stored in any case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CourierRefusal {
    /// Courier mode is off (total quota 0).
    Off,
    /// Already carried.
    Duplicate,
    /// `hops_remaining` is 0: it could never be handed on.
    NoHopsLeft,
    /// Already expired.
    Expired,
    /// The per-encounter allowance is used up.
    EncounterQuota,
    /// Larger than the quota it falls under, or it would itself be the first to be evicted.
    NoRoom,
}

/// Per-encounter usage, kept by the agent for each peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncounterUsage {
    /// Bytes admitted from this encounter.
    pub bytes: u64,
    /// Bundles admitted from this encounter.
    pub bundles: u32,
    /// The channel's allowance: at most this many bytes...
    pub max_bytes: u64,
    /// ...and this many bundles.
    pub max_bundles: u32,
}

impl Default for EncounterUsage {
    fn default() -> Self {
        Self::for_channel(ClaId::Iroh)
    }
}

impl EncounterUsage {
    /// Nothing used yet, with the allowance of an encounter over `cla`.
    #[must_use]
    pub const fn for_channel(cla: ClaId) -> Self {
        let (max_bytes, max_bundles) = match cla {
            ClaId::Ble => (BLE_ENCOUNTER_BYTES, BLE_ENCOUNTER_BUNDLES),
            _ => (PER_ENCOUNTER_BYTES, PER_ENCOUNTER_BUNDLES),
        };
        Self {
            bytes: 0,
            bundles: 0,
            max_bytes,
            max_bundles,
        }
    }

    /// Whether one more bundle of `size` bytes fits the allowance.
    #[must_use]
    pub fn allows(&self, size: u32) -> bool {
        self.bundles < self.max_bundles
            && self.bytes.saturating_add(u64::from(size)) <= self.max_bytes
    }
}

/// The courier index.
#[derive(Debug, Default)]
pub struct CourierStore {
    entries: BTreeMap<CourierKey, CourierMeta>,
    // Non-unique: one commitment may map to several bundles.
    by_commitment: BTreeMap<[u8; 16], BTreeSet<CourierKey>>,
    total: u64,
    /// Stored bytes per arrival encounter.
    batch_bytes: BTreeMap<u64, u64>,
    next_batch: u64,
}

/// The scope a quota applies to.
#[derive(Clone, Copy)]
enum Scope {
    Total,
    Tag([u8; 16]),
    Intro,
}

impl CourierStore {
    /// Rebuilds the index from rows the runtime loaded at start.
    #[must_use]
    pub fn from_loaded(rows: Vec<CourierMeta>) -> Self {
        let mut s = Self::default();
        for m in rows {
            s.insert(m);
        }
        s
    }

    /// A fresh arrival-encounter id for the bundles one peer hands us.
    pub fn new_batch(&mut self) -> u64 {
        self.next_batch = self.next_batch.saturating_add(1);
        self.next_batch
    }

    /// Bytes stored from one arrival encounter.
    #[must_use]
    pub fn batch_bytes(&self, batch: u64) -> u64 {
        self.batch_bytes.get(&batch).copied().unwrap_or(0)
    }

    /// Number of carried bundles.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing is carried.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Bytes used.
    #[must_use]
    pub const fn used_bytes(&self) -> u64 {
        self.total
    }

    /// Every carried entry (for queries and the next encounter step).
    pub fn entries(&self) -> impl Iterator<Item = &CourierMeta> {
        self.entries.values()
    }

    /// Admits `bundle` (already decoded and checked against tombstones by the caller), evicting
    /// lower-ranked entries to make room. Returns the database writes on success.
    ///
    /// # Errors
    ///
    /// [`CourierRefusal`] when the bundle is not stored.
    ///
    /// # Security
    ///
    /// Keeps courier resources bounded: after a successful call every quota (total, per tag, intro) holds, and the
    /// per-encounter allowance is charged. Never stores a bundle that cannot be handed on.
    #[allow(
        clippy::too_many_arguments,
        reason = "each argument is a separate input of the admission decision"
    )]
    pub fn admit(
        &mut self,
        bundle: &Bundle,
        bytes: &[u8],
        class: CourierClass,
        limit_total: u64,
        usage: &mut EncounterUsage,
        batch: u64,
        now_unix_ms: u64,
    ) -> Result<Vec<StoreWrite>, CourierRefusal> {
        if limit_total == 0 {
            return Err(CourierRefusal::Off);
        }
        let key = CourierKey {
            bundle_id: *bundle.bundle_id().as_bytes(),
            commitment: *bundle.purge_commitment(),
        };
        if self.entries.contains_key(&key) {
            return Err(CourierRefusal::Duplicate);
        }
        if bundle.hops_remaining() == 0 {
            return Err(CourierRefusal::NoHopsLeft);
        }
        let size = u32::try_from(bytes.len()).map_err(|_| CourierRefusal::NoRoom)?;
        let meta = CourierMeta {
            key,
            dst_tag: *bundle.dst_tag().as_bytes(),
            class: if bundle.kind() == BundleKind::Intro {
                CourierClass::Intro
            } else {
                class
            },
            size,
            admitted_unix_ms: now_unix_ms,
            age_at_admission_ms: bundle.age_ms(),
            lifetime_ms: u64::from(bundle.lifetime_s()) * 1000,
            copies: bundle.copies(),
            hops: bundle.hops_remaining(),
            batch,
        };
        if meta.expired(now_unix_ms) {
            return Err(CourierRefusal::Expired);
        }
        if !usage.allows(size) {
            return Err(CourierRefusal::EncounterQuota);
        }

        let mut scopes = alloc::vec![
            (Scope::Total, limit_total),
            (Scope::Tag(meta.dst_tag), PER_TAG_BYTES)
        ];
        if meta.class == CourierClass::Intro {
            scopes.push((Scope::Intro, INTRO_BYTES));
        }
        // Plan evictions without touching state, so a refusal changes nothing. Batch sizes are
        // tracked as planned, so evicting from a big batch makes it less likely to be next.
        let mut victims: BTreeSet<CourierKey> = BTreeSet::new();
        let mut planned = self.batch_bytes.clone();
        let bb = |p: &BTreeMap<u64, u64>, b: u64| p.get(&b).copied().unwrap_or(0);
        for (scope, limit) in scopes {
            if u64::from(size) > limit {
                return Err(CourierRefusal::NoRoom);
            }
            let mut used = self.used_in(scope, &victims);
            while used.saturating_add(u64::from(size)) > limit {
                let Some(victim) = self
                    .entries
                    .values()
                    .filter(|m| !victims.contains(&m.key) && Self::in_scope(m, scope))
                    .max_by_key(|m| m.rank(now_unix_ms, bb(&planned, m.batch)))
                else {
                    return Err(CourierRefusal::NoRoom);
                };
                // The newcomer would be the first to go: do not admit it. Its own encounter
                // counts with it, so one peer's flood refuses itself before evicting others.
                let newcomer = meta.rank(
                    now_unix_ms,
                    bb(&planned, batch).saturating_add(u64::from(size)),
                );
                if newcomer >= victim.rank(now_unix_ms, bb(&planned, victim.batch)) {
                    return Err(CourierRefusal::NoRoom);
                }
                used = used.saturating_sub(u64::from(victim.size));
                if let Some(v) = planned.get_mut(&victim.batch) {
                    *v = v.saturating_sub(u64::from(victim.size));
                }
                victims.insert(victim.key);
            }
        }

        let mut writes = Vec::new();
        if !victims.is_empty() {
            let gone: Vec<CourierKey> = victims.into_iter().collect();
            for k in &gone {
                self.remove(k);
            }
            writes.push(StoreWrite::Delete(gone));
        }
        usage.bytes = usage.bytes.saturating_add(u64::from(size));
        usage.bundles = usage.bundles.saturating_add(1);
        self.insert(meta.clone());
        writes.push(StoreWrite::Put {
            meta,
            bytes: bytes.to_vec(),
        });
        Ok(writes)
    }

    /// One carried entry.
    #[must_use]
    pub fn get(&self, key: &CourierKey) -> Option<&CourierMeta> {
        self.entries.get(key)
    }

    /// Records that we now hold `copies` of a carried bundle (after a spray hand-off).
    pub fn set_copies(&mut self, key: &CourierKey, copies: u8) -> Option<StoreWrite> {
        let m = self.entries.get_mut(key)?;
        m.copies = copies;
        Some(StoreWrite::SetCopies { key: *key, copies })
    }

    /// Drops every entry with `commitment` (a purge token arrived).
    pub fn purge(&mut self, commitment: &PurgeCommitment) -> Option<StoreWrite> {
        let keys: Vec<CourierKey> = self
            .by_commitment
            .get(commitment.as_bytes())?
            .iter()
            .copied()
            .collect();
        for k in &keys {
            self.remove(k);
        }
        (!keys.is_empty()).then_some(StoreWrite::Delete(keys))
    }

    /// Drops expired entries.
    pub fn expire(&mut self, now_unix_ms: u64) -> Option<StoreWrite> {
        let keys: Vec<CourierKey> = self
            .entries
            .values()
            .filter(|m| m.expired(now_unix_ms))
            .map(|m| m.key)
            .collect();
        for k in &keys {
            self.remove(k);
        }
        (!keys.is_empty()).then_some(StoreWrite::Delete(keys))
    }

    /// Shrinks to a new total limit (the user lowered the setting, or turned courier mode off).
    pub fn shrink_to(&mut self, limit_total: u64, now_unix_ms: u64) -> Option<StoreWrite> {
        let mut gone = Vec::new();
        while self.total > limit_total {
            let Some(k) = self
                .entries
                .values()
                .max_by_key(|m| {
                    m.rank(
                        now_unix_ms,
                        self.batch_bytes.get(&m.batch).copied().unwrap_or(0),
                    )
                })
                .map(|m| m.key)
            else {
                break;
            };
            self.remove(&k);
            gone.push(k);
        }
        (!gone.is_empty()).then_some(StoreWrite::Delete(gone))
    }

    fn in_scope(m: &CourierMeta, scope: Scope) -> bool {
        match scope {
            Scope::Total => true,
            Scope::Tag(t) => m.dst_tag == t,
            Scope::Intro => m.class == CourierClass::Intro,
        }
    }

    fn used_in(&self, scope: Scope, except: &BTreeSet<CourierKey>) -> u64 {
        self.entries
            .values()
            .filter(|m| !except.contains(&m.key) && Self::in_scope(m, scope))
            .map(|m| u64::from(m.size))
            .sum()
    }

    fn insert(&mut self, m: CourierMeta) {
        self.total = self.total.saturating_add(u64::from(m.size));
        *self.batch_bytes.entry(m.batch).or_default() += u64::from(m.size);
        self.next_batch = self.next_batch.max(m.batch);
        self.by_commitment
            .entry(m.key.commitment)
            .or_default()
            .insert(m.key);
        self.entries.insert(m.key, m);
    }

    fn remove(&mut self, k: &CourierKey) {
        if let Some(m) = self.entries.remove(k) {
            self.total = self.total.saturating_sub(u64::from(m.size));
            if let Some(b) = self.batch_bytes.get_mut(&m.batch) {
                *b = b.saturating_sub(u64::from(m.size));
                if *b == 0 {
                    self.batch_bytes.remove(&m.batch);
                }
            }
            if let Some(set) = self.by_commitment.get_mut(&k.commitment) {
                set.remove(k);
                if set.is_empty() {
                    self.by_commitment.remove(&k.commitment);
                }
            }
        }
    }
}
