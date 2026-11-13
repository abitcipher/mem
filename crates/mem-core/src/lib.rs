//! MEM sans-IO bundle agent.
//!
//! `Agent::handle(Input, rng) -> Vec<Output>` is deterministic given its inputs and RNG. It performs
//! no I/O, reads no clocks and spawns no tasks. All persistence is expressed as
//! `Output::Persist`.
//!
//! Scope: pairing by card, 1:1 text messages, the consent engine with standing rules and a privacy
//! log; channels QR (P0), Iroh on the local network (P1, direct encounters with automatic outbox
//! flush) and share-sheet (P4). Couriers, the hub, acks and groups build on these.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

mod agent;
pub mod attest;
mod backup;
pub mod courier;
pub mod disclosure;
pub mod model;
mod snapshot;

pub use agent::{
    ACK_DELAY_MS, ACK_LIFETIME_S, Agent, CONTACT_KEY_STALE_MS, CardPreview, ClaError, EmailAccount,
    INTRO_COPIES, INTRO_LIFETIME_S, Input, KEY_UPDATE_AFTER_MS, MAILBOX_RETAIN_MS,
    MAILBOX_ROTATE_MS, MAX_ATTEST_LOG, MAX_BLOCKED, MAX_GROUPS, MAX_INVITATIONS, MAX_PENDING_ACKS,
    MAX_REQUESTS, MAX_SENDS_PER_ENCOUNTER, MESSAGE_LIFETIME_S, MIN_ADVERTISED_TAGS,
    MIN_MESSAGE_LIFETIME_S, Output, PEER_BUNDLE_QUOTA, POW_CHECKS_PER_ENCOUNTER, PowJob,
    RETRANSMIT_AFTER_MS, SEEN_CAPACITY, StorageOp, TOMBSTONE_CAPACITY, clamp_lifetime,
};
pub use agent::{
    AttachmentSecret, DeviceJoin, JoinError, MAX_PENDING_PART_BYTES, PULL_WINDOW, SMALL_SEALED_MAX,
};
pub use agent::{CALL_LIFETIME_S, CallPolicy, MAX_OFFERS_PER_MINUTE};
pub use backup::{BackupFailure, Restored};
pub use mem_cla::{ClaId, PrivacyGrade};
pub use mem_crypto::backup::{BackupError, KdfParams, PhraseForm};
pub use mem_proto::call::{CallSignal, EndReason};
pub use mem_proto::encounter::EncounterMsg;
pub use mem_proto::hub::{Crossing, HubOrigin};
pub use snapshot::SnapshotError;
