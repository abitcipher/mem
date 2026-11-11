//! Encrypted snapshot storage.
//!
//! Layout in the profile directory:
//! - `state.key.wrapped`: the 32-byte state key wrapped by a platform [`KeyGuard`] (Android
//!   Keystore), when the platform has one.
//! - `state.key`: the state key itself, file mode 0600 on Unix; only where no guard exists
//!   (desktop, tests) or while the guard fails. A guard that works takes it over and the file
//!   is removed.
//! - `state.bin`: `nonce || XChaCha20-Poly1305(snapshot)`, replaced atomically.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use mem_crypto::storage::{open_state, seal_state};
use rand_core::CryptoRng;
use zeroize::Zeroizing;

const KEY_FILE: &str = "state.key";
const WRAPPED_FILE: &str = "state.key.wrapped";
const WRAPPED_TMP: &str = "state.key.wrapped.tmp";

/// A platform key store that wraps the state key with a key that never leaves it:
/// Android Keystore today. The wrapped form is opaque to MEM.
///
/// # Security
///
/// The unwrapped key exists only in this process; implementations must not log or keep it.
pub trait KeyGuard: Send + Sync + std::fmt::Debug {
    /// Wraps `key`.
    ///
    /// # Errors
    ///
    /// [`KeyGuardError`] if the platform store fails.
    fn wrap(&self, key: &[u8; 32]) -> Result<Vec<u8>, KeyGuardError>;
    /// Unwraps what [`KeyGuard::wrap`] returned.
    ///
    /// # Errors
    ///
    /// [`KeyGuardError`] if the platform store fails or the blob is not ours.
    fn unwrap(&self, wrapped: &[u8]) -> Result<Zeroizing<[u8; 32]>, KeyGuardError>;
}

/// The platform key store failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("platform key store failed")]
pub struct KeyGuardError;
const STATE_FILE: &str = "state.bin";
const TMP_FILE: &str = "state.bin.tmp";

/// Storage failures.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum StoreError {
    /// Filesystem error.
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    /// The key file has the wrong size.
    #[error("storage key is corrupt")]
    BadKey,
    /// The state key is wrapped by the platform key store, which cannot unwrap it now.
    #[error("the platform key store cannot unlock the storage key")]
    KeyUnavailable,
    /// The state could not be decrypted or decoded.
    #[error("stored state is corrupt or was written with another key")]
    Corrupt,
    /// Encryption failed.
    #[error("could not encrypt state")]
    Seal,
}

pub(crate) struct Store {
    dir: PathBuf,
    key: Zeroizing<[u8; 32]>,
}

impl Store {
    /// Opens the store, creating the state key on first run.
    ///
    /// With a `guard`: a wrapped key is unwrapped (failure is
    /// [`StoreError::KeyUnavailable`], never a new identity); a plain `state.key` from before is
    /// wrapped and then removed; a new key is stored wrapped only. If the guard cannot wrap, the
    /// key stays in `state.key` as before and wrapping is tried again at the next open, so a
    /// broken platform store never locks the user out.
    pub(crate) fn open(
        dir: &Path,
        rng: &mut impl CryptoRng,
        guard: Option<&dyn KeyGuard>,
    ) -> Result<Self, StoreError> {
        fs::create_dir_all(dir)?;
        let key_path = dir.join(KEY_FILE);
        let wrapped_path = dir.join(WRAPPED_FILE);
        let key = if wrapped_path.exists() {
            let guard = guard.ok_or(StoreError::KeyUnavailable)?;
            let key = guard
                .unwrap(&fs::read(&wrapped_path)?)
                .map_err(|_| StoreError::KeyUnavailable)?;
            // An upgrade interrupted after the wrapped key was written leaves the plain one.
            if key_path.exists() {
                if read_plain(&key_path)? != key {
                    return Err(StoreError::BadKey);
                }
                remove_plain(&key_path)?;
            }
            key
        } else if key_path.exists() {
            let key = read_plain(&key_path)?;
            if let Some(guard) = guard
                && store_wrapped(dir, guard, &key)?
            {
                remove_plain(&key_path)?;
            }
            key
        } else {
            let mut key = Zeroizing::new([0u8; 32]);
            rng.fill_bytes(key.as_mut());
            let wrapped = match guard {
                Some(guard) => store_wrapped(dir, guard, &key)?,
                None => false,
            };
            if !wrapped {
                write_private(&key_path, key.as_ref())?;
            }
            key
        };
        Ok(Self {
            dir: dir.to_path_buf(),
            key,
        })
    }

    /// Whether the state key is held by the platform key store.
    #[cfg(test)]
    pub(crate) fn key_guarded(&self) -> bool {
        self.dir.join(WRAPPED_FILE).exists() && !self.dir.join(KEY_FILE).exists()
    }

    /// The courier database key, derived from the state key.
    pub(crate) fn courier_key(&self) -> Zeroizing<[u8; 32]> {
        mem_crypto::storage::courier_db_key(&self.key)
    }

    /// The profile directory.
    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }

    pub(crate) fn load(&self) -> Result<Option<Zeroizing<Vec<u8>>>, StoreError> {
        let path = self.dir.join(STATE_FILE);
        if !path.exists() {
            return Ok(None);
        }
        let sealed = fs::read(path)?;
        open_state(&self.key, &sealed)
            .map(Some)
            .map_err(|_| StoreError::Corrupt)
    }

    /// Keeps the current encrypted state as `replaced-<unix_ms>.bin` before a restore replaces
    /// it. It stays encrypted with `state.key`; nothing is deleted.
    pub(crate) fn keep_copy(&self, unix_ms: u64) -> Result<Option<std::path::PathBuf>, StoreError> {
        let path = self.dir.join(STATE_FILE);
        if !path.exists() {
            return Ok(None);
        }
        let kept = self.dir.join(format!("replaced-{unix_ms}.bin"));
        write_private(&kept, &fs::read(path)?)?;
        Ok(Some(kept))
    }

    pub(crate) fn save(&self, snapshot: &[u8], rng: &mut impl CryptoRng) -> Result<(), StoreError> {
        let sealed = seal_state(&self.key, snapshot, rng).map_err(|_| StoreError::Seal)?;
        let tmp = self.dir.join(TMP_FILE);
        write_private(&tmp, &sealed)?;
        fs::rename(&tmp, self.dir.join(STATE_FILE))?;
        // Make the rename durable.
        if let Ok(d) = File::open(&self.dir) {
            let _ = d.sync_all();
        }
        Ok(())
    }
}

fn read_plain(path: &Path) -> Result<Zeroizing<[u8; 32]>, StoreError> {
    let bytes = Zeroizing::new(fs::read(path)?);
    let key: [u8; 32] = bytes
        .as_slice()
        .try_into()
        .map_err(|_| StoreError::BadKey)?;
    Ok(Zeroizing::new(key))
}

/// Wraps `key` and stores it atomically, after checking that it unwraps to the same key.
/// `Ok(false)` when the guard failed (nothing written).
fn store_wrapped(dir: &Path, guard: &dyn KeyGuard, key: &[u8; 32]) -> Result<bool, StoreError> {
    let Ok(wrapped) = guard.wrap(key) else {
        return Ok(false);
    };
    match guard.unwrap(&wrapped) {
        Ok(back) if *back == *key => {}
        _ => return Ok(false),
    }
    let tmp = dir.join(WRAPPED_TMP);
    write_private(&tmp, &wrapped)?;
    fs::rename(&tmp, dir.join(WRAPPED_FILE))?;
    if let Ok(d) = File::open(dir) {
        let _ = d.sync_all();
    }
    Ok(true)
}

/// Overwrites the plain key file with zeros, then removes it (the overwrite is best effort on
/// flash storage; the Keystore wrap is the protection).
fn remove_plain(path: &Path) -> Result<(), StoreError> {
    write_private(path, &[0u8; 32])?;
    fs::remove_file(path)?;
    Ok(())
}

/// Writes `data` to `path`, readable only by the owner on Unix, and fsyncs it.
fn write_private(path: &Path, data: &[u8]) -> Result<(), StoreError> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut f = options.open(path)?;
    f.write_all(data)?;
    f.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]

    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// A stand-in for the Keystore: XOR with a secret pad, with a marker byte.
    #[derive(Debug)]
    struct FakeGuard {
        pad: u8,
        broken: AtomicBool,
    }

    impl FakeGuard {
        fn new(pad: u8) -> Self {
            Self {
                pad,
                broken: AtomicBool::new(false),
            }
        }
    }

    impl KeyGuard for FakeGuard {
        fn wrap(&self, key: &[u8; 32]) -> Result<Vec<u8>, KeyGuardError> {
            if self.broken.load(Ordering::Relaxed) {
                return Err(KeyGuardError);
            }
            let mut v = vec![0xAA];
            v.extend(key.iter().map(|b| b ^ self.pad));
            Ok(v)
        }
        fn unwrap(&self, wrapped: &[u8]) -> Result<Zeroizing<[u8; 32]>, KeyGuardError> {
            if self.broken.load(Ordering::Relaxed) || wrapped.len() != 33 || wrapped[0] != 0xAA {
                return Err(KeyGuardError);
            }
            let mut k = Zeroizing::new([0u8; 32]);
            for (o, b) in k.iter_mut().zip(&wrapped[1..]) {
                *o = b ^ self.pad;
            }
            Ok(k)
        }
    }

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("mem-store-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d
    }

    fn rng() -> rand_chacha::ChaCha20Rng {
        rand_core::SeedableRng::seed_from_u64(7)
    }

    #[test]
    fn a_new_profile_keeps_only_the_wrapped_key() {
        let d = dir("new");
        let g = FakeGuard::new(0x5c);
        let s = Store::open(&d, &mut rng(), Some(&g)).unwrap();
        s.save(b"state", &mut rng()).unwrap();
        assert!(!d.join(KEY_FILE).exists());
        assert!(s.key_guarded());
        let again = Store::open(&d, &mut rng(), Some(&g)).unwrap();
        assert_eq!(again.load().unwrap().unwrap().as_slice(), b"state");
    }

    #[test]
    fn an_old_plain_key_is_taken_over_and_removed() {
        let d = dir("upgrade");
        let old = Store::open(&d, &mut rng(), None).unwrap();
        old.save(b"kept", &mut rng()).unwrap();
        assert!(d.join(KEY_FILE).exists());
        let g = FakeGuard::new(0x33);
        let s = Store::open(&d, &mut rng(), Some(&g)).unwrap();
        assert!(s.key_guarded());
        assert!(!d.join(KEY_FILE).exists());
        assert_eq!(s.load().unwrap().unwrap().as_slice(), b"kept");
        assert_eq!(
            *s.courier_key(),
            *old.courier_key(),
            "courier db still opens"
        );
    }

    #[test]
    fn a_failing_guard_never_locks_the_user_out() {
        let d = dir("broken");
        let g = FakeGuard::new(0x11);
        g.broken.store(true, Ordering::Relaxed);
        let s = Store::open(&d, &mut rng(), Some(&g)).unwrap();
        s.save(b"x", &mut rng()).unwrap();
        assert!(!s.key_guarded(), "plain key, as before");
        // The store works again: the next open takes the key over.
        g.broken.store(false, Ordering::Relaxed);
        let s = Store::open(&d, &mut rng(), Some(&g)).unwrap();
        assert!(s.key_guarded());
        assert_eq!(s.load().unwrap().unwrap().as_slice(), b"x");
    }

    #[test]
    fn a_wrapped_key_without_its_guard_is_an_error_not_a_new_identity() {
        let d = dir("locked");
        let g = FakeGuard::new(0x42);
        Store::open(&d, &mut rng(), Some(&g))
            .unwrap()
            .save(b"mine", &mut rng())
            .unwrap();
        assert!(matches!(
            Store::open(&d, &mut rng(), None),
            Err(StoreError::KeyUnavailable)
        ));
        g.broken.store(true, Ordering::Relaxed);
        assert!(matches!(
            Store::open(&d, &mut rng(), Some(&g)),
            Err(StoreError::KeyUnavailable)
        ));
        assert!(!d.join(KEY_FILE).exists(), "no new key was made");
    }

    #[test]
    fn an_interrupted_upgrade_finishes() {
        let d = dir("interrupted");
        let old = Store::open(&d, &mut rng(), None).unwrap();
        old.save(b"y", &mut rng()).unwrap();
        let g = FakeGuard::new(0x77);
        // The wrapped key was written but the plain one not yet removed.
        let key = read_plain(&d.join(KEY_FILE)).unwrap();
        assert!(store_wrapped(&d, &g, &key).unwrap());
        let s = Store::open(&d, &mut rng(), Some(&g)).unwrap();
        assert!(s.key_guarded());
        assert_eq!(s.load().unwrap().unwrap().as_slice(), b"y");
    }
}
