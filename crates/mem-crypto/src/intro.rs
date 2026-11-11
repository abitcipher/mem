//! Contact requests without meeting: intro tags, the intro key id and proof of work.

use zeroize::Zeroizing;

use crate::identity::IdentityPublic;
use crate::mailbox::{MailboxKeyId, MailboxPublic};

/// Milliseconds in an intro week.
pub const WEEK_MS: u64 = 7 * 24 * 60 * 60 * 1000;

/// Required proof-of-work difficulty in protocol v1, in leading zero bits.
///
/// About 16 Argon2id runs on average: roughly 2 s on a mid-range phone, longer on old ones.
pub const POW_DIFFICULTY: u8 = 4;

/// Argon2id memory for proof of work, KiB (16 MiB).
pub const POW_M_KIB: u32 = 16 * 1024;

const POW_SALT: &[u8; 16] = b"MEM/v1/pow-salt!";

/// The intro week of a wall-clock time.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    reason = "u32 weeks last for 82 million years"
)]
pub const fn week_for_unix_ms(unix_ms: u64) -> u32 {
    (unix_ms / WEEK_MS) as u32
}

/// `intro_tag = BLAKE3("MEM/v1/intro" || identity_pk || week_be32)[0..16]`.
///
/// # Security
///
/// Computable by anyone who has the card, so it must not be advertised unless the user opted in.
#[must_use]
pub fn intro_tag(recipient: &IdentityPublic, week: u32) -> [u8; 16] {
    let mut h = blake3::Hasher::new();
    h.update(b"MEM/v1/intro");
    h.update(recipient.as_bytes());
    h.update(&week.to_be_bytes());
    let mut out = [0u8; 16];
    out.copy_from_slice(&h.finalize().as_bytes()[..16]);
    out
}

/// The opaque key id of an intro payload, sealed to a mailbox key from a card before any contact
/// secret exists.
#[must_use]
pub fn intro_key_id(mailbox: &MailboxPublic) -> MailboxKeyId {
    let k = blake3::derive_key("MEM v1 2026-10 intro-key-id", mailbox.as_bytes());
    let mut out = [0u8; 8];
    out.copy_from_slice(&k[..8]);
    MailboxKeyId::from_bytes(out)
}

/// One proof-of-work evaluation: `Argon2id(bundle_id || dst_tag || week_be32 || nonce)`.
fn pow_hash(bundle_id: &[u8; 16], dst_tag: &[u8; 16], week: u32, nonce: [u8; 8]) -> [u8; 32] {
    let mut password = Zeroizing::new([0u8; 44]);
    password[..16].copy_from_slice(bundle_id);
    password[16..32].copy_from_slice(dst_tag);
    password[32..36].copy_from_slice(&week.to_be_bytes());
    password[36..].copy_from_slice(&nonce);
    let mut out = [0u8; 32];
    // The parameters are fixed and valid, so this cannot fail; if it ever did, the all-ones
    // output below is never a valid proof.
    let ok = argon2::Params::new(POW_M_KIB, 1, 1, Some(32))
        .ok()
        .map(|p| argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, p))
        .is_some_and(|a| {
            a.hash_password_into(&password[..], POW_SALT, &mut out)
                .is_ok()
        });
    if !ok {
        out = [0xff; 32];
    }
    out
}

fn leading_zero_bits(h: &[u8; 32]) -> u32 {
    let mut n = 0;
    for b in h {
        if *b == 0 {
            n += 8;
        } else {
            return n + b.leading_zeros();
        }
    }
    n
}

/// Whether `nonce` proves `difficulty` for this bundle and week (one Argon2id run, ~0.1 s).
///
/// # Security
///
/// Costly by design: callers bound how many proofs they check per encounter.
#[must_use]
pub fn pow_valid(
    bundle_id: &[u8; 16],
    dst_tag: &[u8; 16],
    week: u32,
    nonce: &[u8; 8],
    difficulty: u8,
) -> bool {
    leading_zero_bits(&pow_hash(bundle_id, dst_tag, week, *nonce)) >= u32::from(difficulty)
}

/// Near a week boundary a verifier also tries the adjacent week (clock skew).
pub const WEEK_SLACK_MS: u64 = 6 * 60 * 60 * 1000;

/// The intro weeks a verifier tries for a bundle created at `created_unix_ms` (its local estimate
/// `now − age_ms`): that week, and the adjacent one within [`WEEK_SLACK_MS`] of a boundary.
/// Couriers and the hub use the same rule.
#[must_use]
pub fn pow_weeks(created_unix_ms: u64) -> ([u32; 2], usize) {
    let w = week_for_unix_ms(created_unix_ms);
    let into_week = created_unix_ms % WEEK_MS;
    if into_week < WEEK_SLACK_MS {
        ([w, w.saturating_sub(1)], 2)
    } else if WEEK_MS - into_week < WEEK_SLACK_MS {
        ([w, w.saturating_add(1)], 2)
    } else {
        ([w, w], 1)
    }
}

/// Searches nonces `start, start + 1, …` for at most `tries` Argon2id runs.
#[must_use]
pub fn pow_solve(
    bundle_id: &[u8; 16],
    dst_tag: &[u8; 16],
    week: u32,
    difficulty: u8,
    start: u64,
    tries: u32,
) -> Option<[u8; 8]> {
    (0..u64::from(tries))
        .map(|i| start.wrapping_add(i).to_be_bytes())
        .find(|n| pow_valid(bundle_id, dst_tag, week, n, difficulty))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::IdentityKey;
    use rand_chacha::ChaCha20Rng;
    use rand_core::SeedableRng;

    #[test]
    fn intro_tags_change_weekly_and_per_identity() {
        let mut rng = ChaCha20Rng::from_seed([1; 32]);
        let a = IdentityKey::generate(&mut rng).public();
        let b = IdentityKey::generate(&mut rng).public();
        assert_ne!(intro_tag(&a, 100), intro_tag(&a, 101));
        assert_ne!(intro_tag(&a, 100), intro_tag(&b, 100));
        assert_eq!(week_for_unix_ms(WEEK_MS * 3 + 1), 3);
    }

    #[test]
    fn proof_of_work_solves_and_binds_its_inputs() {
        let (id, tag) = ([1; 16], [2; 16]);
        let nonce = pow_solve(&id, &tag, 9, 2, 0, 64).unwrap();
        assert!(pow_valid(&id, &tag, 9, &nonce, 2));
        // Another bundle, tag or week needs its own work.
        let others = [([3; 16], tag, 9), (id, [4; 16], 9), (id, tag, 10)];
        assert!(
            others
                .iter()
                .any(|(i, t, w)| !pow_valid(i, t, *w, &nonce, 2))
        );
    }
}
