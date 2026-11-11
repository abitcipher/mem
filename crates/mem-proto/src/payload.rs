//! HPKE payload wrapper (`spec/payload.cddl`).

use alloc::vec::Vec;
use core::fmt;

use minicbor::Decoder;

use crate::cbor::{self, DecodeLimits, encode_with};
use crate::envelope::MAX_BUNDLE_LEN;
use crate::error::{ProtoError, SchemaError};

/// Length of the X25519 HPKE encapsulated key (suite 1).
pub const ENC_LEN: usize = 32;
/// Length of the X-Wing HPKE encapsulated key (suite 2).
pub const XWING_ENC_LEN: usize = 1120;

/// The encapsulated-key length of a known suite; `None` for a suite this version does not know.
#[must_use]
pub const fn enc_len(suite_id: u8) -> Option<usize> {
    match suite_id {
        1 => Some(ENC_LEN),
        2 => Some(XWING_ENC_LEN),
        _ => None,
    }
}

const LIMITS: DecodeLimits = DecodeLimits {
    max_input_len: MAX_BUNDLE_LEN,
    max_container_len: 4,
};

/// `[suite_id, mailbox_key_id, enc, ct]`.
#[derive(Clone, PartialEq, Eq)]
pub struct Payload {
    /// Cipher suite: 1 (X25519) or 2 (X-Wing).
    pub suite_id: u8,
    /// Opaque per-contact mailbox key handle.
    pub mailbox_key_id: [u8; 8],
    /// HPKE encapsulated key, of the suite's length ([`enc_len`]).
    pub enc: Vec<u8>,
    /// HPKE ciphertext of the padded inner payload.
    pub ciphertext: Vec<u8>,
}

impl fmt::Debug for Payload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Payload")
            .field("suite_id", &self.suite_id)
            .field("ciphertext_len", &self.ciphertext.len())
            .finish_non_exhaustive()
    }
}

impl Payload {
    /// Canonical CBOR encoding.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        encode_with(self.ciphertext.len() + self.enc.len() + 24, |e| {
            e.array(4)?
                .u8(self.suite_id)?
                .bytes(&self.mailbox_key_id)?
                .bytes(&self.enc)?
                .bytes(&self.ciphertext)?;
            Ok(())
        })
    }

    /// Decodes an untrusted payload.
    ///
    /// # Errors
    ///
    /// [`ProtoError`] for malformed or non-canonical input.
    pub fn decode(input: &[u8]) -> Result<Self, ProtoError> {
        if input.len() > LIMITS.max_input_len {
            return Err(ProtoError::InputTooLarge {
                len: input.len(),
                max: LIMITS.max_input_len,
            });
        }
        cbor::scan(input, &LIMITS)?;
        let mut d = Decoder::new(input);
        if d.array().ok() != Some(Some(4)) {
            return Err(SchemaError::MapShape.into());
        }
        let suite_id = d.u8().map_err(|_| SchemaError::WrongType { key: 0 })?;
        let len = enc_len(suite_id).ok_or(SchemaError::UnknownValue { key: 0 })?;
        let mailbox_key_id = fixed::<8>(&mut d, 1)?;
        let enc = d.bytes().map_err(|_| SchemaError::WrongType { key: 2 })?;
        if enc.len() != len {
            return Err(SchemaError::WrongLength { key: 2 }.into());
        }
        let enc = enc.to_vec();
        let ciphertext = d
            .bytes()
            .map_err(|_| SchemaError::WrongType { key: 3 })?
            .to_vec();
        Ok(Self {
            suite_id,
            mailbox_key_id,
            enc,
            ciphertext,
        })
    }
}

fn fixed<const N: usize>(d: &mut Decoder<'_>, index: u64) -> Result<[u8; N], SchemaError> {
    let b = d
        .bytes()
        .map_err(|_| SchemaError::WrongType { key: index })?;
    <[u8; N]>::try_from(b).map_err(|_| SchemaError::WrongLength { key: index })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn roundtrip_and_shape() {
        let p = Payload {
            suite_id: 1,
            mailbox_key_id: [1; 8],
            enc: vec![2; 32],
            ciphertext: vec![3; 272],
        };
        let bytes = p.encode();
        assert_eq!(bytes[0], 0x84);
        assert_eq!(Payload::decode(&bytes).unwrap(), p);
        assert!(Payload::decode(&bytes[..bytes.len() - 1]).is_err());
        let mut three = bytes.clone();
        three[0] = 0x83;
        assert!(Payload::decode(&three).is_err());
    }

    /// Published vectors (`spec/vectors/payload/`), validated against `spec/payload.cddl` in CI.
    #[test]
    fn published_vectors() {
        extern crate std;
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/vectors/payload");
        for (name, suite_id, enc) in [("suite1", 1, ENC_LEN), ("suite2", 2, XWING_ENC_LEN)] {
            let bytes = Payload {
                suite_id,
                mailbox_key_id: [1; 8],
                enc: vec![2; enc],
                ciphertext: vec![3; 48],
            }
            .encode();
            let file = dir.join(std::format!("{name}.cbor"));
            if std::env::var_os("MEM_UPDATE_VECTORS").is_some() {
                std::fs::create_dir_all(&dir).unwrap();
                std::fs::write(&file, &bytes).unwrap();
            }
            assert_eq!(std::fs::read(&file).unwrap(), bytes);
        }
    }

    /// Suite 2 carries a 1 120-byte X-Wing encapsulation; each suite's length is enforced and an
    /// unknown suite is refused.
    #[test]
    fn suites_and_their_enc_lengths() {
        let p = Payload {
            suite_id: 2,
            mailbox_key_id: [1; 8],
            enc: vec![2; XWING_ENC_LEN],
            ciphertext: vec![3; 272],
        };
        assert_eq!(Payload::decode(&p.encode()).unwrap(), p);
        let wrong = Payload {
            suite_id: 2,
            enc: vec![2; ENC_LEN],
            ..p.clone()
        };
        assert!(Payload::decode(&wrong.encode()).is_err());
        let unknown = Payload {
            suite_id: 3,
            ..p.clone()
        };
        assert!(Payload::decode(&unknown.encode()).is_err());
    }
}
