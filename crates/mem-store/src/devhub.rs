//! Bundles this device keeps while acting as a hub for its owner's contacts.
//!
//! Each row remembers which contact uploaded it, so one contact's share of the space can be
//! enforced; the hub never learns who a bundle is for beyond its tag. Rows leave on a purge
//! token or at expiry, and purged commitments are refused until their lifetime ends.

use rusqlite::{OptionalExtension, params};

use crate::{CourierDb, StoreError, signed};

pub(crate) const SCHEMA_V4: &str = "BEGIN;
     -- Bundles held while this device acts as a hub; `seq` orders live delivery.
     CREATE TABLE device_hub (
         seq INTEGER PRIMARY KEY AUTOINCREMENT,
         bundle_id BLOB NOT NULL,
         purge_commitment BLOB NOT NULL,
         dst_tag BLOB NOT NULL,
         contact BLOB NOT NULL,
         size INTEGER NOT NULL,
         expires_ms INTEGER NOT NULL,
         bytes BLOB NOT NULL,
         UNIQUE (bundle_id, purge_commitment)
     );
     CREATE INDEX device_hub_by_commitment ON device_hub (purge_commitment);
     CREATE INDEX device_hub_by_contact ON device_hub (contact);
     CREATE TABLE device_hub_tombstones (
         commitment BLOB PRIMARY KEY,
         expires_ms INTEGER NOT NULL
     ) WITHOUT ROWID;
     PRAGMA user_version = 4;
     COMMIT;";

/// A bundle offered to the hub, already decoded and checked by the caller.
#[derive(Clone, PartialEq, Eq)]
pub struct HubBundle<'a> {
    /// Envelope field 1.
    pub bundle_id: [u8; 16],
    /// Envelope field 10.
    pub commitment: [u8; 16],
    /// Envelope field 3.
    pub dst_tag: [u8; 16],
    /// Who uploaded it (contact id).
    pub contact: [u8; 32],
    /// When it expires, Unix ms.
    pub expires_ms: u64,
    /// The encoded bundle.
    pub bytes: &'a [u8],
}

impl core::fmt::Debug for HubBundle<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HubBundle")
            .field("size", &self.bytes.len())
            .finish_non_exhaustive()
    }
}

/// What became of an upload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HubPut {
    /// Stored, with its sequence number.
    Stored(i64),
    /// Already held.
    Duplicate,
    /// Its commitment was purged.
    Purged,
    /// No room: the total or the contact's share is used up.
    Quota,
}

impl CourierDb {
    /// Stores an upload within `total` bytes overall and `per_contact` for its uploader, after
    /// dropping what expired before `now_ms`.
    ///
    /// # Errors
    ///
    /// [`StoreError::Db`].
    pub fn hub_put(
        &mut self,
        b: &HubBundle<'_>,
        now_ms: u64,
        total: u64,
        per_contact: u64,
    ) -> Result<HubPut, StoreError> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "DELETE FROM device_hub WHERE expires_ms <= ?1",
            params![signed(now_ms)],
        )?;
        tx.execute(
            "DELETE FROM device_hub_tombstones WHERE expires_ms <= ?1",
            params![signed(now_ms)],
        )?;
        let purged: Option<i64> = tx
            .query_row(
                "SELECT 1 FROM device_hub_tombstones WHERE commitment = ?1",
                params![b.commitment.as_slice()],
                |r| r.get(0),
            )
            .optional()?;
        if purged.is_some() {
            tx.commit()?;
            return Ok(HubPut::Purged);
        }
        let held: Option<i64> = tx
            .query_row(
                "SELECT seq FROM device_hub WHERE bundle_id = ?1 AND purge_commitment = ?2",
                params![b.bundle_id.as_slice(), b.commitment.as_slice()],
                |r| r.get(0),
            )
            .optional()?;
        if held.is_some() {
            tx.commit()?;
            return Ok(HubPut::Duplicate);
        }
        let size = b.bytes.len() as u64;
        let used: i64 = tx.query_row("SELECT COALESCE(SUM(size), 0) FROM device_hub", [], |r| {
            r.get(0)
        })?;
        let theirs: i64 = tx.query_row(
            "SELECT COALESCE(SUM(size), 0) FROM device_hub WHERE contact = ?1",
            params![b.contact.as_slice()],
            |r| r.get(0),
        )?;
        let fits = |used: i64, limit: u64| u64::try_from(used).unwrap_or(u64::MAX) + size <= limit;
        if !fits(used, total) || !fits(theirs, per_contact) {
            tx.commit()?;
            return Ok(HubPut::Quota);
        }
        tx.execute(
            "INSERT INTO device_hub (bundle_id, purge_commitment, dst_tag, contact, size,
                 expires_ms, bytes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                b.bundle_id.as_slice(),
                b.commitment.as_slice(),
                b.dst_tag.as_slice(),
                b.contact.as_slice(),
                signed(size),
                signed(b.expires_ms),
                b.bytes,
            ],
        )?;
        let seq = tx.last_insert_rowid();
        tx.commit()?;
        Ok(HubPut::Stored(seq))
    }

    /// Unexpired bundles after `after_seq` whose tag starts with one of `prefixes`, oldest first,
    /// with their sequence numbers.
    ///
    /// # Errors
    ///
    /// [`StoreError`].
    pub fn hub_fetch(
        &self,
        prefixes: &[[u8; 2]],
        after_seq: i64,
        now_ms: u64,
    ) -> Result<Vec<(i64, Vec<u8>)>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT seq, dst_tag FROM device_hub WHERE seq > ?1 AND expires_ms > ?2 ORDER BY seq",
        )?;
        let rows = stmt.query_map(params![after_seq, signed(now_ms)], |r| {
            Ok((r.get::<_, i64>(0)?, r.get::<_, Vec<u8>>(1)?))
        })?;
        let mut wanted = Vec::new();
        for row in rows {
            let (seq, tag) = row?;
            if tag.len() < 2 {
                return Err(StoreError::Corrupt);
            }
            if prefixes.iter().any(|p| p[..] == tag[..2]) {
                wanted.push(seq);
            }
        }
        let mut out = Vec::with_capacity(wanted.len());
        for seq in wanted {
            let bytes: Option<Vec<u8>> = self
                .conn
                .query_row(
                    "SELECT bytes FROM device_hub WHERE seq = ?1",
                    params![seq],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(bytes) = bytes {
                out.push((seq, bytes));
            }
        }
        Ok(out)
    }

    /// Deletes every bundle with one of `commitments` and refuses them until `expires_ms`.
    ///
    /// # Errors
    ///
    /// [`StoreError::Db`]; nothing changes then.
    pub fn hub_purge(
        &mut self,
        commitments: &[[u8; 16]],
        expires_ms: u64,
    ) -> Result<(), StoreError> {
        let tx = self.conn.transaction()?;
        for c in commitments {
            tx.execute(
                "DELETE FROM device_hub WHERE purge_commitment = ?1",
                params![c.as_slice()],
            )?;
            tx.execute(
                "INSERT OR REPLACE INTO device_hub_tombstones (commitment, expires_ms)
                 VALUES (?1, ?2)",
                params![c.as_slice(), signed(expires_ms)],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Bytes held for others in all, and how many bundles.
    ///
    /// # Errors
    ///
    /// [`StoreError::Db`].
    pub fn hub_usage(&self, now_ms: u64) -> Result<(u64, u64), StoreError> {
        let (bytes, n): (i64, i64) = self.conn.query_row(
            "SELECT COALESCE(SUM(size), 0), COUNT(*) FROM device_hub WHERE expires_ms > ?1",
            params![signed(now_ms)],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok((
            u64::try_from(bytes).unwrap_or(0),
            u64::try_from(n).unwrap_or(0),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> (CourierDb, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "mem-devhub-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        (CourierDb::open(&dir.join("c.db"), &[3; 32]).unwrap(), dir)
    }

    fn bundle(id: u8, contact: u8, tag: [u8; 2], bytes: &[u8]) -> HubBundle<'_> {
        let mut dst_tag = [0; 16];
        dst_tag[..2].copy_from_slice(&tag);
        HubBundle {
            bundle_id: [id; 16],
            commitment: [id; 16],
            dst_tag,
            contact: [contact; 32],
            expires_ms: 10_000,
            bytes,
        }
    }

    #[test]
    fn quotas_fetch_by_prefix_purge_and_expiry() {
        let (mut d, dir) = db();
        let body = [7u8; 400];
        assert_eq!(
            d.hub_put(&bundle(1, 1, [1, 1], &body), 0, 1000, 600)
                .unwrap(),
            HubPut::Stored(1)
        );
        assert_eq!(
            d.hub_put(&bundle(1, 1, [1, 1], &body), 0, 1000, 600)
                .unwrap(),
            HubPut::Duplicate
        );
        // Contact 1's share is used up; contact 2 still fits the total.
        assert_eq!(
            d.hub_put(&bundle(2, 1, [2, 2], &body), 0, 1000, 600)
                .unwrap(),
            HubPut::Quota
        );
        assert!(matches!(
            d.hub_put(&bundle(3, 2, [2, 2], &body), 0, 1000, 600)
                .unwrap(),
            HubPut::Stored(_)
        ));
        // The total is used up.
        assert_eq!(
            d.hub_put(&bundle(4, 3, [3, 3], &body), 0, 1000, 600)
                .unwrap(),
            HubPut::Quota
        );
        let got = d.hub_fetch(&[[2, 2], [9, 9]], 0, 0).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].1, body);
        assert_eq!(d.hub_fetch(&[[2, 2]], got[0].0, 0).unwrap().len(), 0);
        assert_eq!(d.hub_usage(0).unwrap(), (800, 2));
        // Purged: gone, and refused when uploaded again.
        d.hub_purge(&[[1; 16]], 10_000).unwrap();
        assert_eq!(d.hub_fetch(&[[1, 1]], 0, 0).unwrap().len(), 0);
        assert_eq!(
            d.hub_put(&bundle(1, 1, [1, 1], &body), 0, 1000, 600)
                .unwrap(),
            HubPut::Purged
        );
        // Expired rows are not served and are dropped at the next upload.
        assert_eq!(d.hub_fetch(&[[2, 2]], 0, 10_000).unwrap().len(), 0);
        assert!(matches!(
            d.hub_put(
                &HubBundle {
                    expires_ms: 20_000,
                    ..bundle(5, 1, [5, 5], &body)
                },
                10_001,
                1000,
                600
            )
            .unwrap(),
            HubPut::Stored(_)
        ));
        assert_eq!(d.hub_usage(10_001).unwrap(), (400, 1));
        drop(d);
        let _ = std::fs::remove_dir_all(dir);
    }
}
