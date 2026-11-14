//! MEM storage.
//!
//! [`CourierDb`] keeps the bundles this device carries for other people in an SQLCipher database.
//! The core decides everything (sans-IO) and emits [`StoreWrite`]s; the runtime applies
//! them here in one transaction per batch and loads the index back at start.
//!
//! The rest of the state stays in the encrypted snapshot (`mem-runtime`) until it moves to tables.
#![forbid(unsafe_code)]

use std::path::Path;

use mem_core::courier::{CourierClass, CourierKey, CourierMeta, StoreWrite, TombstoneRow};
use rusqlite::{Connection, OptionalExtension, params};
use zeroize::Zeroizing;

mod devhub;
pub use devhub::{HubBundle, HubPut};

/// Schema version written to `PRAGMA user_version`. Migrations are forward-only.
const SCHEMA_VERSION: i64 = 4;

/// Storage failures.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum StoreError {
    /// SQLite or SQLCipher error, including a wrong key ("file is not a database").
    #[error("database: {0}")]
    Db(#[from] rusqlite::Error),
    /// The database was written by a newer build.
    #[error("courier database schema {0} is newer than this build")]
    NewerSchema(i64),
    /// A stored row has an impossible value.
    #[error("courier database row is corrupt")]
    Corrupt,
}

/// The courier database.
pub struct CourierDb {
    conn: Connection,
}

impl core::fmt::Debug for CourierDb {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CourierDb").finish_non_exhaustive()
    }
}

impl CourierDb {
    /// Opens (or creates) the database at `path`, encrypted with the 32-byte raw `key`.
    ///
    /// # Errors
    ///
    /// [`StoreError::Db`] if the file cannot be opened or the key is wrong;
    /// [`StoreError::NewerSchema`] for a database from a newer build.
    ///
    /// # Security
    ///
    /// The key is passed as a raw SQLCipher key (`x'…'`), so no password KDF runs and no key text
    /// is stored. The hex string is zeroized after use. Rows are envelopes and ciphertext of other
    /// people's bundles; SQLCipher hides even the envelopes (tags, times) at rest.
    ///
    /// Accepted residue: SQLite frees its parsed copy of the `PRAGMA key`
    /// statement without wiping it, so part of the key's hex text can stay in freed heap memory
    /// while the database is open.
    pub fn open(path: &Path, key: &[u8; 32]) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        let mut hex = Zeroizing::new(String::with_capacity(70));
        hex.push_str("x'");
        for b in key {
            hex.push(char::from_digit(u32::from(b >> 4), 16).unwrap_or('0'));
            hex.push(char::from_digit(u32::from(b & 0xf), 16).unwrap_or('0'));
        }
        hex.push('\'');
        conn.pragma_update(None, "key", hex.as_str())?;
        // Fails here with "file is not a database" if the key is wrong.
        let version: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version > SCHEMA_VERSION {
            return Err(StoreError::NewerSchema(version));
        }
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        if version < 1 {
            conn.execute_batch(
                "BEGIN;
                 CREATE TABLE courier_store (
                     bundle_id BLOB NOT NULL,
                     purge_commitment BLOB NOT NULL,
                     dst_tag BLOB NOT NULL,
                     class INTEGER NOT NULL,
                     size INTEGER NOT NULL,
                     admitted_ms INTEGER NOT NULL,
                     age_ms INTEGER NOT NULL,
                     lifetime_ms INTEGER NOT NULL,
                     copies INTEGER NOT NULL,
                     hops INTEGER NOT NULL,
                     bytes BLOB NOT NULL,
                     PRIMARY KEY (bundle_id, purge_commitment)
                 ) WITHOUT ROWID;
                 -- Non-unique: one commitment may name several bundles.
                 CREATE INDEX courier_by_commitment ON courier_store (purge_commitment);
                 PRAGMA user_version = 1;
                 COMMIT;",
            )?;
        }
        if version < 2 {
            conn.execute_batch(
                "BEGIN;
                 -- One row per tombstoned commitment, with the token to gossip.
                 CREATE TABLE purge_tombstones (
                     commitment BLOB PRIMARY KEY,
                     token BLOB NOT NULL,
                     expires_ms INTEGER NOT NULL
                 ) WITHOUT ROWID;
                 PRAGMA user_version = 2;
                 COMMIT;",
            )?;
        }
        if version < 3 {
            conn.execute_batch(
                "BEGIN;
                 -- Arrival encounter, for fair eviction.
                 ALTER TABLE courier_store ADD COLUMN batch INTEGER NOT NULL DEFAULT 0;
                 PRAGMA user_version = 3;
                 COMMIT;",
            )?;
        }
        if version < 4 {
            conn.execute_batch(devhub::SCHEMA_V4)?;
        }
        Ok(Self { conn })
    }

    /// Every carried bundle's metadata, for [`mem_core::Input::StoreLoaded`].
    ///
    /// # Errors
    ///
    /// [`StoreError`] on a read failure or an impossible row.
    pub fn load_index(&self) -> Result<Vec<CourierMeta>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT bundle_id, purge_commitment, dst_tag, class, size, admitted_ms, age_ms,
                    lifetime_ms, copies, hops, batch FROM courier_store",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, Vec<u8>>(0)?,
                r.get::<_, Vec<u8>>(1)?,
                r.get::<_, Vec<u8>>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, i64>(4)?,
                r.get::<_, i64>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, i64>(7)?,
                r.get::<_, i64>(8)?,
                r.get::<_, i64>(9)?,
                r.get::<_, i64>(10)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, c, tag, class, size, admitted, age, lifetime, copies, hops, batch) = row?;
            out.push(CourierMeta {
                key: CourierKey {
                    bundle_id: id16(&id)?,
                    commitment: id16(&c)?,
                },
                dst_tag: id16(&tag)?,
                class: CourierClass::from_u8(small(class)?),
                size: u32::try_from(size).map_err(|_| StoreError::Corrupt)?,
                admitted_unix_ms: unsigned(admitted)?,
                age_at_admission_ms: unsigned(age)?,
                lifetime_ms: unsigned(lifetime)?,
                copies: small(copies)?,
                hops: small(hops)?,
                batch: unsigned(batch)?,
            });
        }
        Ok(out)
    }

    /// Every stored tombstone, for [`mem_core::Input::StoreLoaded`].
    ///
    /// # Errors
    ///
    /// [`StoreError`] on a read failure or an impossible row.
    pub fn load_tombstones(&self) -> Result<Vec<TombstoneRow>, StoreError> {
        let mut stmt = self
            .conn
            .prepare("SELECT commitment, token, expires_ms FROM purge_tombstones")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, Vec<u8>>(0)?,
                r.get::<_, Vec<u8>>(1)?,
                r.get::<_, i64>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (c, t, exp) = row?;
            out.push(TombstoneRow {
                commitment: id16(&c)?,
                token: id16(&t)?,
                expires_unix_ms: unsigned(exp)?,
            });
        }
        Ok(out)
    }

    /// One carried bundle's bytes, if still stored.
    ///
    /// # Errors
    ///
    /// [`StoreError::Db`] on a read failure.
    pub fn bundle(&self, key: &CourierKey) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT bytes FROM courier_store WHERE bundle_id = ?1 AND purge_commitment = ?2",
                params![key.bundle_id.as_slice(), key.commitment.as_slice()],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Applies `writes` in order, in one transaction: all or nothing.
    ///
    /// # Errors
    ///
    /// [`StoreError::Db`]; the database is then unchanged.
    pub fn apply(&mut self, writes: &[StoreWrite]) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        for w in writes {
            match w {
                StoreWrite::Put { meta, bytes } => {
                    tx.execute(
                        "INSERT OR REPLACE INTO courier_store (bundle_id, purge_commitment, dst_tag,
                             class, size, admitted_ms, age_ms, lifetime_ms, copies, hops, bytes,
                             batch)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                        params![
                            meta.key.bundle_id.as_slice(),
                            meta.key.commitment.as_slice(),
                            meta.dst_tag.as_slice(),
                            meta.class.as_u8(),
                            meta.size,
                            signed(meta.admitted_unix_ms),
                            signed(meta.age_at_admission_ms),
                            signed(meta.lifetime_ms),
                            meta.copies,
                            meta.hops,
                            bytes,
                            signed(meta.batch),
                        ],
                    )?;
                }
                StoreWrite::SetCopies { key, copies } => {
                    tx.execute(
                        "UPDATE courier_store SET copies = ?3
                         WHERE bundle_id = ?1 AND purge_commitment = ?2",
                        params![key.bundle_id.as_slice(), key.commitment.as_slice(), copies],
                    )?;
                }
                StoreWrite::TombstonePut(t) => {
                    tx.execute(
                        "INSERT OR REPLACE INTO purge_tombstones (commitment, token, expires_ms)
                         VALUES (?1, ?2, ?3)",
                        params![
                            t.commitment.as_slice(),
                            t.token.as_slice(),
                            signed(t.expires_unix_ms)
                        ],
                    )?;
                }
                StoreWrite::TombstoneDelete(commitments) => {
                    let mut stmt =
                        tx.prepare("DELETE FROM purge_tombstones WHERE commitment = ?1")?;
                    for c in commitments {
                        stmt.execute(params![c.as_slice()])?;
                    }
                }
                StoreWrite::Delete(keys) => {
                    let mut stmt = tx.prepare(
                        "DELETE FROM courier_store WHERE bundle_id = ?1 AND purge_commitment = ?2",
                    )?;
                    for k in keys {
                        stmt.execute(params![k.bundle_id.as_slice(), k.commitment.as_slice()])?;
                    }
                }
            }
        }
        tx.commit()?;
        Ok(())
    }
}

fn id16(v: &[u8]) -> Result<[u8; 16], StoreError> {
    v.try_into().map_err(|_| StoreError::Corrupt)
}

fn small(v: i64) -> Result<u8, StoreError> {
    u8::try_from(v).map_err(|_| StoreError::Corrupt)
}

fn unsigned(v: i64) -> Result<u64, StoreError> {
    u64::try_from(v).map_err(|_| StoreError::Corrupt)
}

/// SQLite integers are signed; our values (milliseconds, at most centuries) fit.
fn signed(v: u64) -> i64 {
    i64::try_from(v).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]

    use super::*;

    fn meta(n: u8) -> CourierMeta {
        CourierMeta {
            key: CourierKey {
                bundle_id: [n; 16],
                commitment: [n.wrapping_add(1); 16],
            },
            dst_tag: [9; 16],
            class: CourierClass::Other,
            size: 3,
            admitted_unix_ms: 1_000,
            age_at_admission_ms: 50,
            lifetime_ms: 86_400_000,
            copies: 4,
            hops: 3,
            batch: u64::from(n),
        }
    }

    #[test]
    fn rows_survive_reopen_and_need_the_key() {
        let dir = std::env::temp_dir().join(format!("mem-store-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("courier.db");
        let _ = std::fs::remove_file(&path);
        let key = [7u8; 32];
        {
            let mut db = CourierDb::open(&path, &key).unwrap();
            db.apply(&[
                StoreWrite::Put {
                    meta: meta(1),
                    bytes: vec![1, 2, 3],
                },
                StoreWrite::Put {
                    meta: meta(2),
                    bytes: vec![4, 5, 6],
                },
                StoreWrite::Delete(vec![meta(1).key]),
            ])
            .unwrap();
        }
        let db = CourierDb::open(&path, &key).unwrap();
        assert_eq!(db.load_index().unwrap(), vec![meta(2)]);
        assert_eq!(db.bundle(&meta(2).key).unwrap(), Some(vec![4, 5, 6]));
        assert_eq!(db.bundle(&meta(1).key).unwrap(), None);
        drop(db);
        // Encrypted at rest: the wrong key cannot open it, and the plain bytes are not in the file.
        assert!(CourierDb::open(&path, &[8u8; 32]).is_err());
        let raw = std::fs::read(&path).unwrap();
        assert!(!raw.windows(16).any(|w| w == [9u8; 16]));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
