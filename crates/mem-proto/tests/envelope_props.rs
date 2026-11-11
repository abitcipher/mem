//! Property tests for the bundle envelope: roundtrip, deterministic
//! encoding, and canonical uniqueness of accepted input.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use mem_proto::envelope::{MAX_INTRO_COPIES, MAX_INTRO_LIFETIME_S, MAX_LIFETIME_S};
use mem_proto::{Bundle, BundleId, BundleKind, BundleParams, Pow, RendezvousTag};
use proptest::prelude::*;

fn kind() -> impl Strategy<Value = BundleKind> {
    prop_oneof![
        Just(BundleKind::Message),
        Just(BundleKind::Ack),
        Just(BundleKind::Intro),
        Just(BundleKind::Control),
        Just(BundleKind::KeyUpdate),
    ]
}

/// A kind together with a lifetime valid for it and an unexpired age.
fn kind_lifetime_age() -> impl Strategy<Value = (BundleKind, u32, u64)> {
    kind()
        .prop_flat_map(|k| {
            let max = if k == BundleKind::Intro {
                MAX_INTRO_LIFETIME_S
            } else {
                MAX_LIFETIME_S
            };
            (Just(k), 1..=max)
        })
        .prop_flat_map(|(k, lifetime_s)| {
            (Just(k), Just(lifetime_s), 0..u64::from(lifetime_s) * 1000)
        })
}

prop_compose! {
    fn valid_params()(
        (kind, lifetime_s, age_ms) in kind_lifetime_age(),
        id in any::<[u8; 16]>(),
        tag in any::<[u8; 16]>(),
        hops_remaining in 0u8..=5,
        copies in 1u8..=64,
        nonce in any::<[u8; 8]>(),
        difficulty in any::<u8>(),
        payload in proptest::collection::vec(any::<u8>(), 0..2048),
        purge_commitment in any::<[u8; 16]>(),
    ) -> BundleParams {
        BundleParams {
            bundle_id: BundleId::from_bytes(id),
            kind,
            dst_tag: RendezvousTag::from_bytes(tag),
            lifetime_s,
            age_ms,
            hops_remaining,
            // Intro bundles: a proof of work and at most four copies; others: no proof.
            copies: if kind == BundleKind::Intro { copies.min(MAX_INTRO_COPIES) } else { copies },
            pow: (kind == BundleKind::Intro).then(|| Pow::new(nonce, difficulty)),
            payload,
            purge_commitment,
        }
    }
}

/// If the decoder accepts `input`, it must be the unique canonical encoding of the result.
fn assert_canonical_if_accepted(input: &[u8]) {
    if let Ok(bundle) = Bundle::decode(input) {
        assert_eq!(bundle.encode(), input, "accepted a non-canonical encoding");
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn roundtrip(params in valid_params()) {
        let bundle = Bundle::new(params).unwrap();
        let encoded = bundle.encode();
        prop_assert_eq!(Bundle::decode(&encoded).unwrap(), bundle.clone());
        // Deterministic: encoding twice gives identical bytes.
        prop_assert_eq!(bundle.encode(), encoded);
    }

    #[test]
    fn arbitrary_bytes_never_panic(input in proptest::collection::vec(any::<u8>(), 0..512)) {
        assert_canonical_if_accepted(&input);
    }

    #[test]
    fn mutated_encodings_are_rejected_or_canonical(
        params in valid_params(),
        pos in any::<prop::sample::Index>(),
        byte in any::<u8>(),
        op in 0u8..3,
    ) {
        let mut input = Bundle::new(params).unwrap().encode();
        let i = pos.index(input.len());
        match op {
            0 => input[i] = byte,
            1 => input.insert(i, byte),
            _ => { input.remove(i); }
        }
        assert_canonical_if_accepted(&input);
    }
}
