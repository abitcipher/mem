//! Recovery secret, recovery phrase and backup sealing.
//!
//! The 128-bit secret is the root; a phrase is only a way to write it down. Keys come from the
//! secret, never from the words, so either phrase form restores any backup.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand_core::CryptoRng;
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

/// Length of the recovery secret.
pub const SECRET_LEN: usize = 16;
/// Default Argon2id memory in KiB.
pub const KDF_M_KIB: u32 = 65_536;
/// Default Argon2id passes.
pub const KDF_T: u32 = 3;
/// Default Argon2id parallelism.
pub const KDF_P: u32 = 1;
/// Accepted Argon2id parameter ranges: checked before any work, so a hostile file cannot make us
/// allocate gigabytes or run for minutes.
pub const KDF_M_KIB_RANGE: core::ops::RangeInclusive<u32> = 19_456..=262_144;
/// Accepted Argon2id passes.
pub const KDF_T_RANGE: core::ops::RangeInclusive<u32> = 1..=10;
/// Accepted Argon2id parallelism.
pub const KDF_P_RANGE: core::ops::RangeInclusive<u32> = 1..=4;
/// Words in an English phrase (BIP-39, 128 bits plus a 4-bit checksum).
pub const ENGLISH_WORDS: usize = 12;
/// Words in a proquint phrase: 8 for the secret plus 1 checksum word.
pub const PROQUINT_WORDS: usize = 9;

/// How a phrase is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhraseForm {
    /// 12 words from the BIP-39 English list.
    English,
    /// 9 pronounceable Latin syllables (proquints), language-neutral.
    Proquint,
}

/// Errors from phrases and backups. Deliberately coarse: they must not help an attacker tell
/// "wrong phrase" from "corrupt file" beyond what the user needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum BackupError {
    /// The phrase is not a valid phrase in either form.
    #[error("not a valid recovery phrase")]
    InvalidPhrase,
    /// The phrase has one wrong letter and more than one correction fits.
    #[error("the phrase has a typo that cannot be corrected unambiguously")]
    AmbiguousTypo,
    /// Key derivation parameters are outside the accepted ranges.
    #[error("unsupported key derivation parameters")]
    KdfParams,
    /// Not enough memory for key derivation.
    #[error("not enough memory to derive the backup key")]
    OutOfMemory,
    /// Wrong phrase for this backup, or the backup was modified.
    #[error("the phrase does not open this backup")]
    WrongPhraseOrCorrupt,
}

/// Argon2id parameters, stored in the backup header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    /// Memory in KiB.
    pub m_kib: u32,
    /// Passes.
    pub t: u32,
    /// Parallelism.
    pub p: u32,
}

impl KdfParams {
    /// The default parameters.
    pub const DEFAULT: Self = Self {
        m_kib: KDF_M_KIB,
        t: KDF_T,
        p: KDF_P,
    };

    /// Whether the parameters are within the accepted ranges.
    #[must_use]
    pub fn acceptable(&self) -> bool {
        KDF_M_KIB_RANGE.contains(&self.m_kib)
            && KDF_T_RANGE.contains(&self.t)
            && KDF_P_RANGE.contains(&self.p)
    }
}

/// The 128-bit recovery secret.
///
/// # Security
///
/// Zeroized on drop and never printed. Its phrase may cross FFI only to be shown or typed.
#[derive(Clone, Zeroize, ZeroizeOnDrop, PartialEq, Eq)]
pub struct RecoverySecret([u8; SECRET_LEN]);

impl fmt::Debug for RecoverySecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecoverySecret(…)")
    }
}

/// A phrase that was read back, with any correction applied.
#[derive(Debug)]
pub struct ParsedPhrase {
    /// The secret.
    pub secret: RecoverySecret,
    /// Which form it was written in.
    pub form: PhraseForm,
    /// `Some` if one mistyped letter was corrected: the corrected phrase, for the user to see.
    pub corrected: Option<Zeroizing<String>>,
}

impl RecoverySecret {
    /// Draws a fresh secret.
    pub fn generate(rng: &mut impl CryptoRng) -> Self {
        let mut s = [0u8; SECRET_LEN];
        rng.fill_bytes(&mut s);
        Self(s)
    }

    /// Restores a stored secret.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; SECRET_LEN]) -> Self {
        Self(bytes)
    }

    /// The raw secret, for storage in the encrypted state only.
    #[must_use]
    pub const fn expose(&self) -> &[u8; SECRET_LEN] {
        &self.0
    }

    /// The phrase in `form`, as words.
    ///
    /// # Security
    ///
    /// The words are the secret. Show them only on a capture-protected screen.
    #[must_use]
    pub fn phrase(&self, form: PhraseForm) -> Zeroizing<Vec<String>> {
        let mut words = Zeroizing::new(Vec::new());
        match form {
            PhraseForm::English => {
                // 16 bytes always form a valid 12-word mnemonic.
                if let Ok(m) = bip39::Mnemonic::from_entropy(&self.0) {
                    words.extend(m.words().map(String::from));
                }
            }
            PhraseForm::Proquint => {
                for pair in self.0.as_chunks::<2>().0 {
                    words.push(proquint(u16::from_be_bytes(*pair)));
                }
                words.push(proquint(proquint_checksum(&self.0)));
            }
        }
        words
    }

    /// Reads a phrase in either form. Case, extra spaces and the separators `-`, `·` and `,` are
    /// ignored. A proquint phrase with exactly one wrong letter is corrected when the checksum
    /// allows only one correction.
    ///
    /// # Errors
    ///
    /// [`BackupError::InvalidPhrase`] or [`BackupError::AmbiguousTypo`].
    pub fn from_phrase(text: &str) -> Result<ParsedPhrase, BackupError> {
        let normal = Zeroizing::new(text.to_lowercase());
        let words: Vec<&str> = normal
            .split(|c: char| c.is_whitespace() || matches!(c, '-' | '·' | ','))
            .filter(|w| !w.is_empty())
            .collect();
        match words.len() {
            ENGLISH_WORDS => {
                let joined = Zeroizing::new(words.join(" "));
                let m = bip39::Mnemonic::parse_in_normalized(bip39::Language::English, &joined)
                    .map_err(|_| BackupError::InvalidPhrase)?;
                let (entropy, len) = m.to_entropy_array();
                let mut entropy = Zeroizing::new(entropy);
                let secret = <[u8; SECRET_LEN]>::try_from(&entropy[..len])
                    .map_err(|_| BackupError::InvalidPhrase)?;
                entropy.zeroize();
                Ok(ParsedPhrase {
                    secret: Self(secret),
                    form: PhraseForm::English,
                    corrected: None,
                })
            }
            PROQUINT_WORDS => parse_proquints(&words),
            _ => Err(BackupError::InvalidPhrase),
        }
    }

    /// Derives the backup key with Argon2id.
    fn key(&self, salt: &[u8; 16], params: KdfParams) -> Result<Zeroizing<[u8; 32]>, BackupError> {
        if !params.acceptable() {
            return Err(BackupError::KdfParams);
        }
        let p = argon2::Params::new(params.m_kib, params.t, params.p, Some(32))
            .map_err(|_| BackupError::KdfParams)?;
        // Our own working memory, wiped on drop: argon2 frees its own without wiping, and the
        // last blocks give the key back.
        let mut memory: Zeroizing<Vec<argon2::Block>> = Zeroizing::new(Vec::new());
        memory
            .try_reserve_exact(p.block_count())
            .map_err(|_| BackupError::OutOfMemory)?;
        memory.resize(p.block_count(), argon2::Block::default());
        let argon = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, p);
        let mut key = Zeroizing::new([0u8; 32]);
        argon
            .hash_password_into_with_memory(&self.0, salt, key.as_mut(), &mut memory[..])
            .map_err(|_| BackupError::KdfParams)?;
        Ok(key)
    }

    /// Encrypts `plaintext` with XChaCha20-Poly1305 under the derived key, binding `header` as
    /// associated data. Returns the ciphertext.
    ///
    /// # Errors
    ///
    /// [`BackupError::KdfParams`] for out-of-range parameters.
    pub fn seal(
        &self,
        params: KdfParams,
        salt: &[u8; 16],
        nonce: &[u8; 24],
        header: &[u8],
        plaintext: &[u8],
    ) -> Result<Vec<u8>, BackupError> {
        let key = self.key(salt, params)?;
        let cipher =
            XChaCha20Poly1305::new_from_slice(key.as_ref()).map_err(|_| BackupError::KdfParams)?;
        cipher
            .encrypt(
                &XNonce::from(*nonce),
                Payload {
                    msg: plaintext,
                    aad: header,
                },
            )
            .map_err(|_| BackupError::WrongPhraseOrCorrupt)
    }

    /// Decrypts a backup ciphertext.
    ///
    /// # Errors
    ///
    /// [`BackupError::KdfParams`] before any work for out-of-range parameters;
    /// [`BackupError::WrongPhraseOrCorrupt`] if authentication fails.
    pub fn open(
        &self,
        params: KdfParams,
        salt: &[u8; 16],
        nonce: &[u8; 24],
        header: &[u8],
        ciphertext: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, BackupError> {
        let key = self.key(salt, params)?;
        let cipher =
            XChaCha20Poly1305::new_from_slice(key.as_ref()).map_err(|_| BackupError::KdfParams)?;
        cipher
            .decrypt(
                &XNonce::from(*nonce),
                Payload {
                    msg: ciphertext,
                    aad: header,
                },
            )
            .map(Zeroizing::new)
            .map_err(|_| BackupError::WrongPhraseOrCorrupt)
    }
}

// ------------------------------------------------------------------ proquints
//
// D. Wilkerson, "A Proposal for Proquints: Identifiers that are Readable, Spellable, and
// Pronounceable" (2009, public domain): 16 bits → consonant, vowel, consonant, vowel, consonant
// with 4-bit consonants and 2-bit vowels.

const CONSONANTS: &[u8; 16] = b"bdfghjklmnprstvz";
const VOWELS: &[u8; 4] = b"aiou";

fn proquint(v: u16) -> String {
    let c = |shift: u16| char::from(CONSONANTS[usize::from((v >> shift) & 0xf)]);
    let o = |shift: u16| char::from(VOWELS[usize::from((v >> shift) & 0x3)]);
    let mut s = String::with_capacity(5);
    s.push(c(12));
    s.push(o(10));
    s.push(c(6));
    s.push(o(4));
    s.push(c(0));
    s
}

fn unproquint(word: &[u8]) -> Option<u16> {
    let [c1, v1, c2, v2, c3] = <[u8; 5]>::try_from(word).ok()?;
    let index = |set: &[u8], x: u8| {
        set.iter()
            .position(|y| *y == x)
            .and_then(|i| u16::try_from(i).ok())
    };
    Some(
        index(CONSONANTS, c1)? << 12
            | index(VOWELS, v1)? << 10
            | index(CONSONANTS, c2)? << 6
            | index(VOWELS, v2)? << 4
            | index(CONSONANTS, c3)?,
    )
}

/// The 9th word: the first 16 bits of SHA-256 of the secret.
fn proquint_checksum(secret: &[u8; SECRET_LEN]) -> u16 {
    let h = Sha256::digest(secret);
    u16::from_be_bytes([h[0], h[1]])
}

/// Decodes 9 proquints; with a checksum mismatch, tries every single-letter substitution.
fn parse_proquints(words: &[&str]) -> Result<ParsedPhrase, BackupError> {
    let mut letters = Zeroizing::new(Vec::with_capacity(45));
    for w in words {
        if w.len() != 5 || !w.is_ascii() {
            return Err(BackupError::InvalidPhrase);
        }
        letters.extend_from_slice(w.as_bytes());
    }
    let decode = |l: &[u8]| -> Option<([u8; SECRET_LEN], u16)> {
        let mut secret = [0u8; SECRET_LEN];
        for (i, chunk) in l.as_chunks::<5>().0.iter().take(8).enumerate() {
            secret[2 * i..2 * i + 2].copy_from_slice(&unproquint(chunk)?.to_be_bytes());
        }
        Some((secret, unproquint(&l[40..45])?))
    };
    let valid = |l: &[u8]| -> Option<[u8; SECRET_LEN]> {
        let (secret, sum) = decode(l)?;
        (proquint_checksum(&secret) == sum).then_some(secret)
    };
    if let Some(secret) = valid(&letters) {
        return Ok(ParsedPhrase {
            secret: RecoverySecret(secret),
            form: PhraseForm::Proquint,
            corrected: None,
        });
    }
    // Not valid as typed: each letter position, each allowed letter of its class.
    let mut found: Option<(RecoverySecret, Zeroizing<Vec<u8>>)> = None;
    let mut ambiguous = false;
    for pos in 0..letters.len() {
        let class: &[u8] = if matches!(pos % 5, 1 | 3) {
            VOWELS
        } else {
            CONSONANTS
        };
        let original = letters[pos];
        for &alt in class {
            if alt == original {
                continue;
            }
            let mut trial = Zeroizing::new(letters.to_vec());
            trial[pos] = alt;
            if let Some(secret) = valid(&trial) {
                if found.is_some() {
                    ambiguous = true;
                } else {
                    found = Some((RecoverySecret(secret), trial));
                }
            }
        }
    }
    match found {
        Some(_) if ambiguous => Err(BackupError::AmbiguousTypo),
        Some((secret, trial)) => {
            let mut corrected = Zeroizing::new(String::with_capacity(53));
            for (i, chunk) in trial.as_chunks::<5>().0.iter().enumerate() {
                if i > 0 {
                    corrected.push('-');
                }
                corrected.extend(chunk.iter().map(|b| char::from(*b)));
            }
            Ok(ParsedPhrase {
                secret,
                form: PhraseForm::Proquint,
                corrected: Some(corrected),
            })
        }
        None => Err(BackupError::InvalidPhrase),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    fn secret(seed: u8) -> RecoverySecret {
        RecoverySecret::generate(&mut ChaCha20Rng::from_seed([seed; 32]))
    }

    #[test]
    fn proquint_spec_examples() {
        // From the proquint paper: 127.0.0.1 = lusab-babad, 63.84.220.193 = gutih-tugad.
        assert_eq!(proquint(0x7f00), "lusab");
        assert_eq!(proquint(0x0001), "babad");
        assert_eq!(proquint(0x3f54), "gutih");
        assert_eq!(proquint(0xdcc1), "tugad");
        for v in [0u16, 1, 0x7f00, 0xffff, 0x1234] {
            assert_eq!(unproquint(proquint(v).as_bytes()), Some(v));
        }
    }

    #[test]
    fn both_forms_roundtrip_to_the_same_secret() {
        for seed in 0..20 {
            let s = secret(seed);
            for form in [PhraseForm::English, PhraseForm::Proquint] {
                let words = s.phrase(form);
                assert_eq!(
                    words.len(),
                    match form {
                        PhraseForm::English => ENGLISH_WORDS,
                        PhraseForm::Proquint => PROQUINT_WORDS,
                    }
                );
                let p = RecoverySecret::from_phrase(&words.join(" ")).unwrap();
                assert_eq!(p.secret, s);
                assert_eq!(p.form, form);
                assert!(p.corrected.is_none());
            }
        }
    }

    #[test]
    fn separators_and_case_are_ignored() {
        let s = secret(1);
        let words = s.phrase(PhraseForm::Proquint);
        let text = words.join("-").to_uppercase();
        assert_eq!(RecoverySecret::from_phrase(&text).unwrap().secret, s);
        let english = s.phrase(PhraseForm::English).join("  ,  ");
        assert_eq!(RecoverySecret::from_phrase(&english).unwrap().secret, s);
    }

    #[test]
    fn a_single_proquint_typo_is_corrected_or_refused() {
        let mut corrected = 0;
        for seed in 0..50 {
            let s = secret(seed);
            let words = s.phrase(PhraseForm::Proquint);
            let mut text = words.join("-").into_bytes();
            // Change the first consonant of the third word to another consonant.
            let pos = 12;
            text[pos] = if text[pos] == b'b' { b'd' } else { b'b' };
            let text = String::from_utf8(text).unwrap();
            match RecoverySecret::from_phrase(&text) {
                Ok(p) => {
                    assert_eq!(p.secret, s, "a correction must restore the original");
                    assert_eq!(p.corrected.unwrap().as_str(), words.join("-"));
                    corrected += 1;
                }
                Err(e) => assert_eq!(e, BackupError::AmbiguousTypo),
            }
        }
        assert!(corrected >= 45, "{corrected}");
    }

    #[test]
    fn invalid_phrases_are_refused() {
        let s = secret(2);
        let mut english = s.phrase(PhraseForm::English);
        english.swap(0, 1);
        assert!(RecoverySecret::from_phrase(&english.join(" ")).is_err());
        assert!(RecoverySecret::from_phrase("").is_err());
        assert!(RecoverySecret::from_phrase("hello world").is_err());
        // Wrong length words, non-letters.
        assert!(RecoverySecret::from_phrase(&"lusa ".repeat(9)).is_err());
        assert!(RecoverySecret::from_phrase(&"lus4b ".repeat(9)).is_err());
    }

    #[test]
    fn seal_and_open_bind_the_header() {
        // Small parameters keep the test fast; the ranges are checked separately.
        let params = KdfParams {
            m_kib: 19_456,
            t: 1,
            p: 1,
        };
        let s = secret(3);
        let (salt, nonce) = ([1u8; 16], [2u8; 24]);
        let ct = s.seal(params, &salt, &nonce, b"header", b"state").unwrap();
        assert_eq!(
            s.open(params, &salt, &nonce, b"header", &ct)
                .unwrap()
                .as_slice(),
            b"state"
        );
        assert_eq!(
            s.open(params, &salt, &nonce, b"other", &ct),
            Err(BackupError::WrongPhraseOrCorrupt)
        );
        assert_eq!(
            secret(4).open(params, &salt, &nonce, b"header", &ct),
            Err(BackupError::WrongPhraseOrCorrupt)
        );
    }

    #[test]
    fn out_of_range_kdf_parameters_are_refused_before_work() {
        let s = secret(5);
        for params in [
            KdfParams {
                m_kib: 4_194_304,
                t: 3,
                p: 1,
            },
            KdfParams {
                m_kib: 65_536,
                t: 1000,
                p: 1,
            },
            KdfParams {
                m_kib: 8,
                t: 1,
                p: 1,
            },
        ] {
            assert_eq!(
                s.open(params, &[0; 16], &[0; 24], b"", &[0; 32]),
                Err(BackupError::KdfParams)
            );
        }
    }
}
