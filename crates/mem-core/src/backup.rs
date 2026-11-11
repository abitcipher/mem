//! Backup and restore: sealing the state snapshot under the recovery
//! secret, and opening it again on a new device. Pure computation; the runtime does the I/O.

use alloc::string::String;
use alloc::vec::Vec;

use mem_crypto::backup::{BackupError, KdfParams, ParsedPhrase, PhraseForm, RecoverySecret};
use mem_proto::backup::{
    BackupContents, BackupFile, BackupHeader, KdfFields, MAX_APP_VERSION_LEN, pad, unpad,
};
use rand_core::CryptoRng;
use zeroize::{Zeroize, Zeroizing};

use crate::agent::Agent;
use crate::snapshot::SnapshotError;

/// Why a backup could not be made or restored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum BackupFailure {
    /// Backups are not turned on: there is no recovery secret yet.
    #[error("backups are not turned on")]
    NotEnabled,
    /// Not a MEM backup file.
    #[error("not a MEM backup")]
    NotABackup,
    /// Phrase or decryption failure.
    #[error("{0}")]
    Crypto(#[from] BackupError),
    /// The decrypted state is unusable (for example from a newer build).
    #[error("{0}")]
    Snapshot(#[from] SnapshotError),
}

/// A successful restore.
#[derive(Debug)]
pub struct Restored {
    /// The restored agent: same identity, contacts and keys.
    pub agent: Agent,
    /// When the backup was made.
    pub created_unix_ms: u64,
    /// Whether it included message history.
    pub history: bool,
    /// `Some` if a mistyped proquint letter was corrected, for the user to confirm.
    pub corrected: Option<Zeroizing<String>>,
}

impl Agent {
    /// Whether backups are on (a recovery secret exists).
    #[must_use]
    pub const fn backups_enabled(&self) -> bool {
        self.recovery.is_some()
    }

    /// The form the recovery phrase is shown in.
    #[must_use]
    pub const fn phrase_form(&self) -> PhraseForm {
        self.recovery_form
    }

    /// The phrase to show: a new one waiting for confirmation if there is one, otherwise the
    /// current recovery phrase, if backups are on.
    ///
    /// # Security
    ///
    /// The words are the secret: only the capture-protected phrase screen may receive them.
    #[must_use]
    pub fn recovery_phrase(&self) -> Option<Zeroizing<Vec<String>>> {
        match &self.pending_recovery {
            Some((secret, form)) => Some(secret.phrase(*form)),
            None => self.recovery.as_ref().map(|r| r.phrase(self.recovery_form)),
        }
    }

    /// Whether a new phrase is waiting for the user to confirm it.
    #[must_use]
    pub const fn recovery_pending(&self) -> bool {
        self.pending_recovery.is_some()
    }

    /// Draws a new secret to show. It takes effect only on [`Agent::confirm_pending_recovery`],
    /// so a phrase the user never confirmed can never lock a backup.
    pub(crate) fn new_pending_recovery(&mut self, form: PhraseForm, rng: &mut impl CryptoRng) {
        self.pending_recovery = Some((RecoverySecret::generate(rng), form));
    }

    /// Makes the pending secret the recovery secret: backups are on, or use the new phrase from
    /// now on. Older backup files keep opening with the old phrase only. False if none is pending.
    pub(crate) fn confirm_pending_recovery(&mut self) -> bool {
        let Some((secret, form)) = self.pending_recovery.take() else {
            return false;
        };
        self.recovery = Some(secret);
        self.recovery_form = form;
        true
    }

    /// Changes only how the phrase is written; the secret, and so every backup, stays the same.
    pub(crate) fn set_phrase_form(&mut self, form: PhraseForm) {
        self.recovery_form = form;
    }

    /// Seals the current state as a backup file.
    ///
    /// # Errors
    ///
    /// [`BackupFailure::NotEnabled`] without a recovery secret.
    pub fn make_backup(
        &self,
        history: bool,
        app_version: &str,
        rng: &mut impl CryptoRng,
    ) -> Result<Vec<u8>, BackupFailure> {
        self.make_backup_with(history, app_version, KdfParams::DEFAULT, rng)
    }

    /// As [`Agent::make_backup`] with explicit KDF parameters (tests use small ones).
    ///
    /// # Errors
    ///
    /// As [`Agent::make_backup`], and [`BackupError::KdfParams`] for out-of-range parameters.
    pub fn make_backup_with(
        &self,
        history: bool,
        app_version: &str,
        params: KdfParams,
        rng: &mut impl CryptoRng,
    ) -> Result<Vec<u8>, BackupFailure> {
        let secret = self.recovery.as_ref().ok_or(BackupFailure::NotEnabled)?;
        let mut salt = [0u8; 16];
        rng.fill_bytes(&mut salt);
        let mut nonce = [0u8; 24];
        rng.fill_bytes(&mut nonce);
        let header = BackupHeader {
            kdf: KdfFields {
                m_kib: params.m_kib,
                t: params.t,
                p: params.p,
            },
            salt,
            nonce,
        };
        let version: String = app_version.chars().take(MAX_APP_VERSION_LEN).collect();
        let mut contents = BackupContents {
            created_unix_ms: self.now_unix_ms,
            app_version: if version.is_empty() {
                String::from("unknown")
            } else {
                version
            },
            snapshot: self.snapshot_for_backup(history).to_vec(),
            history,
        };
        let plaintext = Zeroizing::new(pad(&Zeroizing::new(contents.encode())));
        contents.snapshot.zeroize();
        let ciphertext = secret.seal(params, &salt, &nonce, &header.aad(), &plaintext)?;
        Ok(BackupFile { header, ciphertext }.encode())
    }

    /// Opens a backup file with a phrase in either form and restores the agent from it.
    ///
    /// # Errors
    ///
    /// [`BackupFailure`]: not a backup, wrong phrase, out-of-range parameters (refused before any
    /// key derivation), or an unusable snapshot.
    pub fn restore_backup(
        file: &[u8],
        phrase: &str,
        now_unix_ms: u64,
    ) -> Result<Restored, BackupFailure> {
        let file = BackupFile::decode(file).map_err(|_| BackupFailure::NotABackup)?;
        let ParsedPhrase {
            secret, corrected, ..
        } = RecoverySecret::from_phrase(phrase)?;
        let h = file.header;
        let params = KdfParams {
            m_kib: h.kdf.m_kib,
            t: h.kdf.t,
            p: h.kdf.p,
        };
        let padded = secret.open(params, &h.salt, &h.nonce, &h.aad(), &file.ciphertext)?;
        let plain = unpad(&padded).map_err(|_| BackupError::WrongPhraseOrCorrupt)?;
        let mut contents =
            BackupContents::decode(plain).map_err(|_| BackupError::WrongPhraseOrCorrupt)?;
        let agent = Agent::restore(&contents.snapshot, now_unix_ms);
        contents.snapshot.zeroize();
        let agent = agent?;
        Ok(Restored {
            agent,
            created_unix_ms: contents.created_unix_ms,
            history: contents.history,
            corrected,
        })
    }
}
