//! MEM wire types.
//!
//! Deterministic CBOR (RFC 8949 §4.2.1) with integer map keys and definite lengths only.
//! Schemas are normative in `spec/*.cddl`; this crate must match them.
//!
//! # Security
//!
//! Every decoder in this crate handles hostile input: bounded size, bounded nesting depth,
//! no panics. All decoders are fuzzed.
#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod armor;
pub mod attachment;
pub mod attest;
pub mod backup;
pub mod call;
pub mod card;
pub mod cbor;
pub mod device;
pub mod edit;
pub mod encounter;
pub mod envelope;
mod error;
pub mod frame;
pub mod group;
pub mod hint;
pub mod hub;
pub mod hublist;
pub mod inner;
pub mod payload;

pub use envelope::{Bundle, BundleId, BundleKind, BundleParams, Pow, RendezvousTag};
pub use error::{CborError, InvalidBundle, ProtoError, SchemaError};
