//! Call setup (content type [`CONTENT_TYPE_CALL`], `spec/inner.cddl`): offers, answers, ICE
//! candidates and the end of a call, end to end between two contacts.
//!
//! The session descriptions carry the media's DTLS fingerprints; because they travel inside
//! signed, encrypted payloads, nobody on the media path can intercept the call.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use minicbor::Decoder;

use crate::cbor::{self, DecodeLimits, encode_with};
use crate::error::{ProtoError, SchemaError};

/// Content type: call setup.
pub const CONTENT_TYPE_CALL: u64 = 10;
/// Longest session description, in bytes.
pub const MAX_SDP_LEN: usize = 12 * 1024;
/// Most candidates in one message.
pub const MAX_CANDIDATES: usize = 32;
/// Longest candidate line, in bytes.
pub const MAX_CANDIDATE_LEN: usize = 512;

const KIND_OFFER: u8 = 0;
const KIND_ANSWER: u8 = 1;
const KIND_CANDIDATES: u8 = 2;
const KIND_END: u8 = 3;

/// Why a call ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    /// Hung up after talking.
    HungUp,
    /// The callee declined.
    Declined,
    /// The callee is in another call.
    Busy,
    /// Media could not connect.
    Failed,
    /// Nobody answered in time.
    Unanswered,
}

impl EndReason {
    const fn code(self) -> u8 {
        match self {
            Self::HungUp => 0,
            Self::Declined => 1,
            Self::Busy => 2,
            Self::Failed => 3,
            Self::Unanswered => 4,
        }
    }

    const fn from_code(c: u8) -> Option<Self> {
        Some(match c {
            0 => Self::HungUp,
            1 => Self::Declined,
            2 => Self::Busy,
            3 => Self::Failed,
            4 => Self::Unanswered,
            _ => return None,
        })
    }
}

/// One call setup message.
#[derive(Clone, PartialEq, Eq)]
pub enum CallSignal {
    /// The caller's session description.
    Offer {
        /// Random per call.
        call_id: [u8; 16],
        /// SDP, 1..=[`MAX_SDP_LEN`] bytes.
        sdp: String,
    },
    /// The callee's session description.
    Answer {
        /// The call answered.
        call_id: [u8; 16],
        /// SDP, 1..=[`MAX_SDP_LEN`] bytes.
        sdp: String,
    },
    /// ICE candidates found after the description went out (trickle ICE).
    Candidates {
        /// The call.
        call_id: [u8; 16],
        /// 1..=[`MAX_CANDIDATES`] candidate lines, each 1..=[`MAX_CANDIDATE_LEN`] bytes.
        candidates: Vec<String>,
    },
    /// The call is over.
    End {
        /// The call.
        call_id: [u8; 16],
        /// Why.
        reason: EndReason,
    },
}

impl fmt::Debug for CallSignal {
    /// Never prints descriptions or candidates (they hold network addresses).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Offer { .. } => f.write_str("CallSignal::Offer"),
            Self::Answer { .. } => f.write_str("CallSignal::Answer"),
            Self::Candidates { candidates, .. } => {
                write!(f, "CallSignal::Candidates({})", candidates.len())
            }
            Self::End { reason, .. } => write!(f, "CallSignal::End({reason:?})"),
        }
    }
}

fn sdp_ok(sdp: &str) -> bool {
    (1..=MAX_SDP_LEN).contains(&sdp.len())
}

fn candidates_ok(c: &[String]) -> bool {
    (1..=MAX_CANDIDATES).contains(&c.len())
        && c.iter().all(|x| (1..=MAX_CANDIDATE_LEN).contains(&x.len()))
}

impl CallSignal {
    /// The call this message belongs to.
    #[must_use]
    pub const fn call_id(&self) -> &[u8; 16] {
        match self {
            Self::Offer { call_id, .. }
            | Self::Answer { call_id, .. }
            | Self::Candidates { call_id, .. }
            | Self::End { call_id, .. } => call_id,
        }
    }

    /// Canonical CBOR: `[kind, call_id, ...]`.
    ///
    /// # Errors
    ///
    /// [`SchemaError::WrongLength`] for an empty or oversized description or candidate list.
    pub fn encode(&self) -> Result<Vec<u8>, ProtoError> {
        let bad = || ProtoError::from(SchemaError::WrongLength { key: 2 });
        match self {
            Self::Offer { call_id, sdp } | Self::Answer { call_id, sdp } => {
                if !sdp_ok(sdp) {
                    return Err(bad());
                }
                let kind = if matches!(self, Self::Offer { .. }) {
                    KIND_OFFER
                } else {
                    KIND_ANSWER
                };
                Ok(encode_with(sdp.len() + 32, |e| {
                    e.array(3)?.u8(kind)?.bytes(call_id)?.str(sdp)?;
                    Ok(())
                }))
            }
            Self::Candidates {
                call_id,
                candidates,
            } => {
                if !candidates_ok(candidates) {
                    return Err(bad());
                }
                let len = candidates.iter().map(|c| c.len() + 3).sum::<usize>();
                Ok(encode_with(len + 32, |e| {
                    e.array(3)?
                        .u8(KIND_CANDIDATES)?
                        .bytes(call_id)?
                        .array(candidates.len() as u64)?;
                    for c in candidates {
                        e.str(c)?;
                    }
                    Ok(())
                }))
            }
            Self::End { call_id, reason } => Ok(encode_with(24, |e| {
                e.array(3)?
                    .u8(KIND_END)?
                    .bytes(call_id)?
                    .u8(reason.code())?;
                Ok(())
            })),
        }
    }

    /// Decodes untrusted content.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for malformed, non-canonical, oversized or unknown messages.
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        let limits = DecodeLimits {
            max_input_len: MAX_SDP_LEN + 64,
            max_container_len: MAX_CANDIDATES as u64,
        };
        if input.len() > limits.max_input_len {
            return Err(ProtoError::InputTooLarge {
                len: input.len(),
                max: limits.max_input_len,
            });
        }
        cbor::scan(input, &limits)?;
        let mut d = Decoder::new(input);
        if d.array().ok() != Some(Some(3)) {
            return Err(SchemaError::MapShape.into());
        }
        let kind = d.u8().map_err(|_| SchemaError::WrongType { key: 0 })?;
        let call_id = d
            .bytes()
            .ok()
            .and_then(|b| <[u8; 16]>::try_from(b).ok())
            .ok_or(SchemaError::WrongLength { key: 1 })?;
        let wrong = SchemaError::WrongType { key: 2 };
        let signal = match kind {
            KIND_OFFER | KIND_ANSWER => {
                let sdp = String::from(d.str().map_err(|_| wrong)?);
                if !sdp_ok(&sdp) {
                    return Err(SchemaError::WrongLength { key: 2 }.into());
                }
                if kind == KIND_OFFER {
                    Self::Offer { call_id, sdp }
                } else {
                    Self::Answer { call_id, sdp }
                }
            }
            KIND_CANDIDATES => {
                let n = match d.array() {
                    Ok(Some(n)) => usize::try_from(n).map_err(|_| wrong)?,
                    _ => return Err(wrong.into()),
                };
                if !(1..=MAX_CANDIDATES).contains(&n) {
                    return Err(SchemaError::WrongLength { key: 2 }.into());
                }
                let mut candidates = Vec::with_capacity(n);
                for _ in 0..n {
                    candidates.push(String::from(d.str().map_err(|_| wrong)?));
                }
                if !candidates_ok(&candidates) {
                    return Err(SchemaError::WrongLength { key: 2 }.into());
                }
                Self::Candidates {
                    call_id,
                    candidates,
                }
            }
            KIND_END => {
                let code = d.u8().map_err(|_| wrong)?;
                Self::End {
                    call_id,
                    reason: EndReason::from_code(code)
                        .ok_or(SchemaError::UnknownValue { key: 2 })?,
                }
            }
            _ => return Err(SchemaError::UnknownValue { key: 0 }.into()),
        };
        if d.position() != input.len() {
            return Err(SchemaError::MapShape.into());
        }
        Ok(signal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn round_trip(s: &CallSignal) {
        assert_eq!(&CallSignal::decode(&s.encode().unwrap()).unwrap(), s);
    }

    #[test]
    fn every_kind_round_trips() {
        let id = [7; 16];
        round_trip(&CallSignal::Offer {
            call_id: id,
            sdp: "v=0\r\n".into(),
        });
        round_trip(&CallSignal::Answer {
            call_id: id,
            sdp: "v=0\r\n".into(),
        });
        round_trip(&CallSignal::Candidates {
            call_id: id,
            candidates: vec!["candidate:1 1 udp 1 10.0.0.2 5000 typ host".into()],
        });
        round_trip(&CallSignal::End {
            call_id: id,
            reason: EndReason::Busy,
        });
    }

    #[test]
    fn out_of_bounds_is_refused() {
        let id = [7; 16];
        let long = CallSignal::Offer {
            call_id: id,
            sdp: "x".repeat(MAX_SDP_LEN + 1),
        };
        assert!(long.encode().is_err());
        let none = CallSignal::Candidates {
            call_id: id,
            candidates: Vec::new(),
        };
        assert!(none.encode().is_err());
        // Unknown kind and unknown reason.
        assert!(
            CallSignal::decode(&[
                0x83, 0x09, 0x50, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x00
            ])
            .is_err()
        );
        let mut end = CallSignal::End {
            call_id: id,
            reason: EndReason::HungUp,
        }
        .encode()
        .unwrap();
        *end.last_mut().unwrap() = 9;
        assert!(CallSignal::decode(&end).is_err());
    }
}
