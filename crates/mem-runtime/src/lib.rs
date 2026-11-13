//! MEM runtime.
//!
//! Drives the sans-IO `mem-core` agent: owns the wall clock and OS randomness, applies storage
//! operations, turns channel text (armor, QR) into agent inputs, and runs network channels.
//! This is the only crate allowed to touch the OS RNG.
//!
//! - [`Runtime`]: synchronous agent + encrypted store, for user-driven channels and tests.
//! - [`Node`]: a [`Runtime`] shared with a Tokio runtime that runs `cla-iroh` (local network
//!   encounters) and a clock, routes agent outputs to peers and publishes [`NodeEvent`]s.
#![forbid(unsafe_code)]

mod node;
mod store;

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use mem_core::{Agent, ClaId, Input, Output, StorageOp};
use mem_proto::armor::{Armored, armor_bundle, armor_card, dearmor};
use rand_chacha::ChaCha20Rng;
use rand_core::{SeedableRng, TryRng};

pub use node::{BleCommand, Node, NodeConfig, NodeEvent, OPERATOR_KEYS};
pub use store::{KeyGuard, KeyGuardError, StoreError};

/// A decrypted attachment, ready to show or hand to the system.
pub struct OpenedAttachment {
    /// File name (chosen by the sender).
    pub name: String,
    /// Media type (stated by the sender).
    pub media_type: String,
    /// The file; wiped from memory when dropped.
    pub data: zeroize::Zeroizing<Vec<u8>>,
}

impl core::fmt::Debug for OpenedAttachment {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("OpenedAttachment")
            .field("media_type", &self.media_type)
            .field("len", &self.data.len())
            .finish_non_exhaustive()
    }
}

/// Runtime failures.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum RuntimeError {
    /// Local storage failed.
    #[error("storage: {0}")]
    Store(#[from] StoreError),
    /// The OS random number generator failed.
    #[error("system randomness unavailable")]
    Rng,
    /// The async runtime could not start.
    #[error("async runtime unavailable")]
    Async,
    /// A backup could not be made or restored.
    #[error("backup: {0}")]
    Backup(#[from] mem_core::BackupFailure),
    /// The courier database failed.
    #[error("courier store: {0}")]
    Courier(#[from] mem_store::StoreError),
    /// A device setup could not be used.
    #[error("device setup: {0:?}")]
    Join(mem_core::JoinError),
    /// This profile already has contacts: joining another identity would replace them.
    #[error("not empty")]
    NotEmpty,
    /// An attestation could not start.
    #[error("attestation: {0}")]
    Attest(mem_core::attest::AttestError),
}

/// The result of a restore, for the UI.
#[derive(Debug)]
pub struct RestoreSummary {
    /// When the backup was made, Unix ms.
    pub created_unix_ms: u64,
    /// Whether message history was included.
    pub history: bool,
    /// The corrected phrase if one mistyped letter was fixed (the user should check it).
    pub corrected: Option<zeroize::Zeroizing<String>>,
    /// Whether an existing identity was replaced (its state was kept, encrypted, next to it).
    pub replaced: bool,
}

/// What a piece of user-supplied text turned out to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Incoming {
    /// A message bundle; the outputs say whether it was delivered.
    Bundle(Vec<Output>),
    /// A pairing card; the outputs say whether the contact was added.
    Card(Vec<Output>),
    /// Not MEM data.
    NotMem,
}

/// The running MEM node for one local profile.
pub struct Runtime {
    agent: Agent,
    store: store::Store,
    /// Bundles carried for other people, next to the snapshot in the profile directory.
    courier: mem_store::CourierDb,
}

/// The courier database file in the profile directory.
const COURIER_DB: &str = "courier.db";

impl std::fmt::Debug for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runtime").finish_non_exhaustive()
    }
}

impl Runtime {
    /// Opens the profile in `dir`, creating a new identity on first run.
    ///
    /// # Errors
    ///
    /// [`RuntimeError`] if storage cannot be read or written, or the OS RNG fails.
    pub fn open(dir: impl AsRef<Path>) -> Result<Self, RuntimeError> {
        Self::open_guarded(dir, None)
    }

    /// Opens the profile with the state key held by a platform key store.
    ///
    /// # Errors
    ///
    /// [`RuntimeError`] if storage cannot be read or written, the OS RNG fails, or the key store
    /// cannot unlock a key it holds ([`StoreError::KeyUnavailable`]).
    pub fn open_guarded(
        dir: impl AsRef<Path>,
        guard: Option<&dyn KeyGuard>,
    ) -> Result<Self, RuntimeError> {
        let dir: PathBuf = dir.as_ref().to_path_buf();
        let mut rng = fresh_rng()?;
        let store = store::Store::open(&dir, &mut rng, guard)?;
        let now = now_unix_ms();
        let agent = if let Some(snapshot) = store.load()? {
            Agent::restore(&snapshot, now).map_err(|_| StoreError::Corrupt)?
        } else {
            let agent = Agent::new(now, &mut rng);
            store.save(&agent.snapshot(), &mut rng)?;
            agent
        };
        let courier =
            mem_store::CourierDb::open(&store.dir().join(COURIER_DB), &store.courier_key())?;
        let mut rt = Self {
            agent,
            store,
            courier,
        };
        rt.load_courier_index(&mut rng)?;
        Ok(rt)
    }

    /// Hands the carried-bundle index and the tombstones to the agent (after open and after a
    /// restore).
    fn load_courier_index(&mut self, rng: &mut ChaCha20Rng) -> Result<(), RuntimeError> {
        let courier = self.courier.load_index()?;
        let tombstones = self.courier.load_tombstones()?;
        let out = self.agent.handle(
            Input::StoreLoaded {
                courier,
                tombstones,
            },
            rng,
        );
        self.persist(&out, rng)
    }

    /// Applies the agent's persistence requests: attachment blobs first (the snapshot that
    /// names them must never come before them), then courier changes in one transaction, then
    /// the snapshot. The courier database is the source of truth for its index, so a crash
    /// between those two only means the index is reloaded from it at the next start.
    fn persist(&mut self, outputs: &[Output], rng: &mut ChaCha20Rng) -> Result<(), RuntimeError> {
        for o in outputs {
            if let Output::Persist(StorageOp::PutBlob { address, sealed }) = o {
                self.store.put_blob(address, sealed)?;
            }
        }
        let writes: Vec<mem_core::courier::StoreWrite> = outputs
            .iter()
            .filter_map(|o| match o {
                Output::Persist(StorageOp::Store(w)) => Some(w.iter().cloned()),
                _ => None,
            })
            .flatten()
            .collect();
        if !writes.is_empty() {
            self.courier.apply(&writes)?;
        }
        if outputs
            .iter()
            .any(|o| matches!(o, Output::Persist(StorageOp::SaveSnapshot)))
        {
            self.store.save(&self.agent.snapshot(), rng)?;
        }
        Ok(())
    }

    /// A carried bundle re-encoded for the next holder, or `None` if it is no
    /// longer stored or does not decode.
    #[must_use]
    pub fn relayed(
        &self,
        key: &mem_core::courier::CourierKey,
        hops: u8,
        copies: u8,
        age_ms: u64,
    ) -> Option<Vec<u8>> {
        let stored = self.courier.bundle(key).ok().flatten()?;
        mem_proto::Bundle::relayed(&stored, hops, copies, age_ms).ok()
    }

    /// Attachment `index` of `message`, decrypted: its name, media type and bytes. `None` if
    /// there is no such attachment or its blob is not on this device yet.
    ///
    /// # Errors
    ///
    /// [`RuntimeError`] if the blob cannot be read, or [`StoreError::Corrupt`] if it does not
    /// open with the message's key (altered on disk).
    pub fn open_attachment(
        &self,
        message: mem_core::model::MsgId,
        index: usize,
    ) -> Result<Option<OpenedAttachment>, RuntimeError> {
        let Some(secret) = self.agent.attachment(message, index) else {
            return Ok(None);
        };
        let Some(sealed) = self.store.blob(&secret.address)? else {
            return Ok(None);
        };
        let data = mem_crypto::attachment::open(&secret.key, &sealed, secret.size)
            .map_err(|_| StoreError::Corrupt)?;
        Ok(Some(OpenedAttachment {
            name: secret.name,
            media_type: secret.media_type,
            data,
        }))
    }

    /// Read access for queries.
    #[must_use]
    pub const fn agent(&self) -> &Agent {
        &self.agent
    }

    /// Ticks the clock, handles `input`, and applies persistence.
    ///
    /// # Errors
    ///
    /// [`RuntimeError`] if persistence or the RNG fails. The agent's in-memory state has already
    /// changed in that case; callers should surface the error and retry or restart.
    pub fn handle(&mut self, input: Input) -> Result<Vec<Output>, RuntimeError> {
        let mut rng = fresh_rng()?;
        let mut outputs = self.agent.handle(
            Input::Tick {
                wall_unix_ms: now_unix_ms(),
            },
            &mut rng,
        );
        outputs.extend(self.agent.handle(input, &mut rng));
        self.persist(&outputs, &mut rng)?;
        Ok(outputs)
    }

    /// Advances the clock alone (epoch rollover re-advertises tags to peers).
    ///
    /// # Errors
    ///
    /// See [`Runtime::handle`].
    pub fn tick(&mut self) -> Result<Vec<Output>, RuntimeError> {
        let mut rng = fresh_rng()?;
        let outputs = self.agent.handle(
            Input::Tick {
                wall_unix_ms: now_unix_ms(),
            },
            &mut rng,
        );
        // Timers change state too (expiry, acks handed over): persist like any input.
        self.persist(&outputs, &mut rng)?;
        Ok(outputs)
    }

    /// Seals the state as an encrypted backup file. Slow (Argon2id, about a second):
    /// call it off the UI thread.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::Backup`] if backups are off; [`RuntimeError::Rng`].
    pub fn make_backup(&self, history: bool) -> Result<Vec<u8>, RuntimeError> {
        let mut rng = fresh_rng()?;
        Ok(self
            .agent
            .make_backup(history, env!("CARGO_PKG_VERSION"), &mut rng)?)
    }

    /// Replaces this profile's state with a backup opened by `phrase`. The current state is kept,
    /// encrypted, as `replaced-<time>.bin` in the profile directory; nothing is deleted.
    /// `data` is a backup file or its `membackup1:` text.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::Backup`] for a wrong phrase or a damaged file; storage errors.
    pub fn restore_backup(
        &mut self,
        data: &[u8],
        phrase: &str,
    ) -> Result<RestoreSummary, RuntimeError> {
        let file = decode_backup_text(data).unwrap_or_else(|| data.to_vec());
        let now = now_unix_ms();
        let restored = Agent::restore_backup(&file, phrase, now)?;
        let mut rng = fresh_rng()?;
        let replaced = self.store.keep_copy(now)?.is_some() && !self.agent.contacts().is_empty();
        self.agent = restored.agent;
        self.store.save(&self.agent.snapshot(), &mut rng)?;
        // Bundles carried for others are not part of a backup; keep carrying what is on disk.
        self.load_courier_index(&mut rng)?;
        Ok(RestoreSummary {
            created_unix_ms: restored.created_unix_ms,
            history: restored.history,
            corrected: restored.corrected,
            replaced,
        })
    }

    /// Starts an in-person attestation with fresh randomness.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::Attest`] (not the main device, or a bad request); [`RuntimeError::Rng`].
    pub fn start_attestation(
        &self,
        request_text: &str,
    ) -> Result<mem_core::attest::AttestSession, RuntimeError> {
        self.agent
            .start_attestation(request_text, &mut fresh_rng()?)
            .map_err(RuntimeError::Attest)
    }

    /// A new device's join request, for a profile with no contacts yet.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::NotEmpty`] if this profile has contacts; [`RuntimeError::Rng`].
    pub fn device_join(&self) -> Result<mem_core::DeviceJoin, RuntimeError> {
        if !self.agent.contacts().is_empty() {
            return Err(RuntimeError::NotEmpty);
        }
        Ok(mem_core::DeviceJoin::new(&mut fresh_rng()?))
    }

    /// Becomes another device of the identity whose main device showed `setup`.
    /// The fresh profile's state is kept, encrypted, next to it; nothing is deleted.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::NotEmpty`] if this profile has contacts; [`RuntimeError::Join`] if the
    /// setup is not for this request; storage errors.
    pub fn join_device(
        &mut self,
        join: &mem_core::DeviceJoin,
        setup: &[u8],
    ) -> Result<(), RuntimeError> {
        if !self.agent.contacts().is_empty() {
            return Err(RuntimeError::NotEmpty);
        }
        let now = now_unix_ms();
        let agent = join.finish(setup, now).map_err(RuntimeError::Join)?;
        let mut rng = fresh_rng()?;
        self.store.keep_copy(now)?;
        self.agent = agent;
        self.store.save(&self.agent.snapshot(), &mut rng)?;
        self.load_courier_index(&mut rng)?;
        Ok(())
    }

    /// After our main device retired this device: a fresh identity replaces
    /// the state, which is overwritten, not kept (the user's own main device removed it).
    ///
    /// # Errors
    ///
    /// Storage errors; [`RuntimeError::Rng`].
    pub fn forget_this_device(&mut self) -> Result<(), RuntimeError> {
        let mut rng = fresh_rng()?;
        self.agent = Agent::new(now_unix_ms(), &mut rng);
        self.store.save(&self.agent.snapshot(), &mut rng)?;
        Ok(())
    }

    /// A backup as `membackup1:` + base64url text, for the share sheet.
    #[must_use]
    pub fn backup_text(file: &[u8]) -> String {
        format!(
            "{}{}",
            mem_proto::backup::ARMOR_PREFIX,
            data_encoding::BASE64URL_NOPAD.encode(file)
        )
    }

    /// Feeds text pasted or scanned by the user, received over `cla`.
    ///
    /// # Errors
    ///
    /// See [`Runtime::handle`].
    pub fn receive_text(&mut self, text: &str, cla: ClaId) -> Result<Incoming, RuntimeError> {
        match text_input(text, cla) {
            Some(TextInput::Bundle(input)) => Ok(Incoming::Bundle(self.handle(input)?)),
            Some(TextInput::Card(input)) => Ok(Incoming::Card(self.handle(input)?)),
            None => Ok(Incoming::NotMem),
        }
    }

    /// Our pairing card as armored text.
    #[must_use]
    pub fn own_card_text(&self) -> String {
        armor_card(&self.agent.own_card().unwrap_or_default())
    }

    /// Armors bundle bytes from an [`Output::Transmit`] for text channels and QR codes.
    #[must_use]
    pub fn armor(bytes: &[u8]) -> String {
        armor_bundle(bytes)
    }
}

/// Agent input for a piece of text, if it holds MEM data.
pub(crate) enum TextInput {
    Bundle(Input),
    Card(Input),
}

pub(crate) fn text_input(text: &str, cla: ClaId) -> Option<TextInput> {
    match dearmor(text) {
        Ok(Armored::Bundle(bytes)) => Some(TextInput::Bundle(Input::Received { cla, bytes })),
        Ok(Armored::Card(bytes)) => Some(TextInput::Card(Input::ImportCard { cla, bytes })),
        // Adding a device is its own, explicit step, never a side effect of a
        // pasted or scanned text.
        Ok(Armored::DeviceRequest(_)) | Err(_) => None,
    }
}

/// A CSPRNG freshly seeded from the OS for each operation.
fn fresh_rng() -> Result<ChaCha20Rng, RuntimeError> {
    let mut seed = zeroize::Zeroizing::new([0u8; 32]);
    getrandom::SysRng
        .try_fill_bytes(seed.as_mut())
        .map_err(|_| RuntimeError::Rng)?;
    Ok(ChaCha20Rng::from_seed(*seed))
}

/// Wall clock in Unix milliseconds (only read here, never in the core).
fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Decodes `membackup1:` text (surrounding whitespace allowed); `None` if `data` is not that.
fn decode_backup_text(data: &[u8]) -> Option<Vec<u8>> {
    let text = std::str::from_utf8(data).ok()?.trim();
    let body = text.strip_prefix(mem_proto::backup::ARMOR_PREFIX)?;
    data_encoding::BASE64URL_NOPAD.decode(body.as_bytes()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mem_core::model::RejectReason;

    fn tmp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("mem-runtime-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// A carried bundle goes into the encrypted courier database and comes back after a
    /// restart; the snapshot never contains it.
    #[test]
    fn carried_bundles_persist_in_the_courier_database() {
        use mem_core::EncounterMsg;
        use mem_core::model::PeerHandle;
        use mem_proto::{Bundle, BundleId, BundleKind, BundleParams, RendezvousTag};

        let dir = tmp("courier");
        let mut c = Runtime::open(&dir).unwrap();
        let bundle = Bundle::new(BundleParams {
            bundle_id: BundleId::from_bytes([1; 16]),
            kind: BundleKind::Message,
            dst_tag: RendezvousTag::from_bytes([2; 16]),
            lifetime_s: 3600,
            age_ms: 0,
            hops_remaining: 3,
            copies: 4,
            pow: None,
            payload: vec![0xCD; 300],
            purge_commitment: [3; 16],
        })
        .unwrap()
        .encode();
        let peer = PeerHandle(1);
        c.handle(Input::EncounterStarted {
            peer,
            cla: ClaId::Iroh,
        })
        .unwrap();
        c.handle(Input::EncounterMessage {
            peer,
            msg: EncounterMsg::Hello {
                version: mem_proto::encounter::ENCOUNTER_VERSION,
                ephemeral_id: [9; 16],
            },
        })
        .unwrap();
        c.handle(Input::EncounterMessage {
            peer,
            msg: EncounterMsg::Bundles(vec![mem_proto::encounter::BundleEntry::bare(
                bundle.clone(),
            )]),
        })
        .unwrap();
        assert_eq!(c.agent().courier_status().2, 1);
        assert!(
            !c.agent()
                .snapshot()
                .windows(300)
                .any(|w| w == &bundle[bundle.len() - 300..])
        );
        drop(c);

        let c = Runtime::open(&dir).unwrap();
        assert_eq!(c.agent().courier_status().2, 1);
        let key = c.agent().courier_entries()[0].key;
        assert_eq!(c.courier.bundle(&key).unwrap(), Some(bundle));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn two_profiles_pair_and_message_via_text_and_survive_restart() {
        let (da, db) = (tmp("a"), tmp("b"));
        let mut a = Runtime::open(&da).unwrap();
        let mut b = Runtime::open(&db).unwrap();
        let fp_a = a.agent().fingerprint();

        assert!(matches!(
            b.receive_text(&a.own_card_text(), ClaId::Qr).unwrap(),
            Incoming::Card(_)
        ));
        assert!(matches!(
            a.receive_text(&b.own_card_text(), ClaId::Qr).unwrap(),
            Incoming::Card(_)
        ));
        let b_id = a.agent().contacts()[0].id;

        let out = a
            .handle(Input::UserSend {
                contact: b_id,
                text: "over text".into(),
            })
            .unwrap();
        let Some(Output::Queued { message, .. }) =
            out.into_iter().find(|o| matches!(o, Output::Queued { .. }))
        else {
            panic!("not queued")
        };
        let out = a
            .handle(Input::RequestTransmit {
                message,
                cla: ClaId::Qr,
            })
            .unwrap();
        let Some(Output::Transmit { bytes, .. }) = out
            .into_iter()
            .find(|o| matches!(o, Output::Transmit { .. }))
        else {
            panic!("no transmit")
        };
        let text = format!("look: {} !", Runtime::armor(&bytes));

        drop(a);
        drop(b);
        let a = Runtime::open(&da).unwrap();
        let mut b = Runtime::open(&db).unwrap();
        assert_eq!(a.agent().fingerprint(), fp_a);

        let Incoming::Bundle(out) = b.receive_text(&text, ClaId::Share).unwrap() else {
            panic!()
        };
        assert!(
            out.iter().any(|o| matches!(o, Output::Deliver { .. })),
            "{out:?}"
        );
        let Incoming::Bundle(out) = b.receive_text(&text, ClaId::Share).unwrap() else {
            panic!()
        };
        // Refused by its tombstone; the only side effect is a saved re-ack.
        assert!(
            out.contains(&Output::Rejected(RejectReason::Purged))
                && !out.iter().any(|o| matches!(o, Output::Deliver { .. })),
            "{out:?}"
        );
        assert_eq!(
            b.receive_text("just chatting", ClaId::Share).unwrap(),
            Incoming::NotMem
        );

        let _ = std::fs::remove_dir_all(da);
        let _ = std::fs::remove_dir_all(db);
    }
}
