//! The single error type of this crate.

/// Errors from decoding, encoding or validating MEM wire types.
///
/// Variants never carry raw input bytes, identifiers or tags, so they are safe to log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ProtoError {
    /// Input is larger than the configured cap; nothing was parsed.
    #[error("input too large: {len} > {max} bytes")]
    InputTooLarge {
        /// Input length in bytes.
        len: usize,
        /// Configured maximum.
        max: usize,
    },
    /// Input is not well-formed, canonical CBOR within the decoder limits.
    #[error("malformed CBOR: {0}")]
    Cbor(#[from] CborError),
    /// Well-formed CBOR that does not match the expected schema.
    #[error("schema violation: {0}")]
    Schema(#[from] SchemaError),
    /// Well-formed, schema-conforming input that violates a protocol rule.
    #[error("protocol violation: {0}")]
    Invalid(#[from] InvalidBundle),
}

/// Low-level CBOR well-formedness and determinism violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CborError {
    /// Input ended in the middle of an item.
    #[error("truncated input")]
    Truncated,
    /// Bytes remain after the top-level item.
    #[error("trailing bytes after top-level item")]
    TrailingBytes,
    /// Indefinite-length item or `break` marker; only definite lengths are allowed.
    #[error("indefinite-length item")]
    IndefiniteLength,
    /// Additional-information values 28..=30 are reserved by RFC 8949.
    #[error("reserved additional information")]
    Reserved,
    /// An integer or length was not encoded in its shortest form (RFC 8949 §4.2.1).
    #[error("non-minimal integer or length encoding")]
    NonMinimal,
    /// Containers are nested deeper than the limit.
    #[error("nesting depth exceeds {max}")]
    DepthExceeded {
        /// Maximum allowed depth.
        max: usize,
    },
    /// An array or map declares more entries than the limit.
    #[error("container length exceeds {max}")]
    ContainerTooLong {
        /// Maximum allowed number of entries.
        max: u64,
    },
    /// Semantic tags are not used in MEM v1.
    #[error("CBOR tags are not allowed")]
    TagNotAllowed,
    /// Floats, `undefined` and unassigned simple values are not used in MEM v1.
    #[error("simple value or float not allowed")]
    SimpleNotAllowed,
    /// A text string is not valid UTF-8.
    #[error("invalid UTF-8 in text string")]
    InvalidUtf8,
}

/// Structural mismatches against the CDDL schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SchemaError {
    /// Item has the wrong CBOR type or is out of range for the field type.
    #[error("wrong type or out-of-range value for field {key}")]
    WrongType {
        /// Map key of the offending field.
        key: u64,
    },
    /// The top level is not a map, or the map has an unexpected number of entries.
    #[error("unexpected map shape")]
    MapShape,
    /// A required field is missing, a key is unknown, or keys are not in canonical order.
    #[error("unexpected key at position of field {expected}")]
    UnexpectedKey {
        /// Key the decoder expected at this position.
        expected: u64,
    },
    /// A byte string has the wrong length for its field.
    #[error("wrong length for field {key}")]
    WrongLength {
        /// Map key of the offending field.
        key: u64,
    },
    /// An enum discriminant is not defined in this protocol version.
    #[error("unknown value for field {key}")]
    UnknownValue {
        /// Map key of the offending field.
        key: u64,
    },
}

/// Protocol-rule violations of an otherwise well-formed bundle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum InvalidBundle {
    /// Only version 1 is supported.
    #[error("unsupported protocol version {0}")]
    UnsupportedVersion(u64),
    /// `hops_remaining` exceeds the hop limit.
    #[error("hops_remaining {0} exceeds the limit of 5")]
    HopLimit(u8),
    /// `copies` is outside `1..=64`.
    #[error("copy budget {0} outside 1..=64")]
    Copies(u8),
    /// `lifetime_s` exceeds the cap for this bundle kind.
    #[error("lifetime {got_s}s exceeds the maximum of {max_s}s")]
    Lifetime {
        /// Declared lifetime.
        got_s: u32,
        /// Maximum for this kind.
        max_s: u32,
    },
    /// `age_ms >= lifetime_s * 1000`.
    #[error("bundle expired")]
    Expired,
    /// The encoded bundle exceeds the maximum bundle size.
    #[error("encoded bundle is {len} bytes, maximum is {max}")]
    TooLarge {
        /// Encoded length.
        len: usize,
        /// Maximum.
        max: usize,
    },
    /// The proof-of-work field is not 9 bytes.
    #[error("proof-of-work field length {0}, expected 9")]
    PowLength(usize),
    /// An intro bundle without a proof of work.
    #[error("intro bundle without proof of work")]
    PowMissing,
    /// A proof of work on a bundle other than an intro.
    #[error("proof of work on a non-intro bundle")]
    PowNotAllowed,
}
