//! MEM cryptography.
//!
//! Composes only audited primitives. No home-grown constructions.
//!
//! # Security
//!
//! Key material lives in `zeroize`-on-drop types, is never logged and never appears in `Debug`
//! output. Randomness is always injected.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod attachment;
pub mod attest;
pub mod backup;
pub mod bloom;
pub mod contact;
mod error;
pub mod identity;
pub mod intro;
pub mod mailbox;
pub mod purge;
pub mod storage;

pub use contact::{ContactSecret, Direction, PairingNonce};
pub use error::CryptoError;
pub use identity::{Fingerprint, IdentityKey, IdentityPublic};
pub use mailbox::{
    MailboxKey, MailboxKeyId, MailboxPublic, SUITE_ID, SUITE_XWING, XWING_ENC_LEN,
    XWING_PUBLIC_LEN, XWingPublic,
};
pub use purge::{PurgeCommitment, PurgeToken, SenderPurgeKey};
