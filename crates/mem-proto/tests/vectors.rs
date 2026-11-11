//! Published envelope test vectors (`spec/vectors/envelope/`).
//!
//! The `.cbor` files are normative: other implementations test against them, and CI validates
//! them against `spec/envelope.cddl`. To regenerate after an *intentional* wire change, run with `MEM_UPDATE_VECTORS=1`.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::path::PathBuf;

use mem_proto::{Bundle, BundleId, BundleKind, BundleParams, Pow, RendezvousTag};

fn vector_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/vectors/envelope")
}

fn id(seed: u8) -> [u8; 16] {
    core::array::from_fn(|i| seed.wrapping_add(u8::try_from(i).unwrap()))
}

fn vectors() -> Vec<(&'static str, Bundle)> {
    let base = BundleParams {
        bundle_id: BundleId::from_bytes(id(0x00)),
        kind: BundleKind::Message,
        dst_tag: RendezvousTag::from_bytes(id(0x80)),
        lifetime_s: 604_800,
        age_ms: 0,
        hops_remaining: 5,
        copies: 8,
        pow: None,
        payload: b"opaque-hpke-ciphertext".to_vec(),
        purge_commitment: id(0xc0),
    };
    let intro = BundleParams {
        kind: BundleKind::Intro,
        lifetime_s: 259_200,
        age_ms: 3_600_000,
        hops_remaining: 2,
        copies: 1,
        pow: Some(Pow::new([0xde, 0xad, 0xbe, 0xef, 0, 0, 0, 7], 4)),
        ..base.clone()
    };
    let ack = BundleParams {
        bundle_id: BundleId::from_bytes(id(0x40)),
        kind: BundleKind::Ack,
        lifetime_s: 1,
        age_ms: 999,
        hops_remaining: 0,
        copies: 64,
        payload: Vec::new(),
        ..base.clone()
    };
    vec![
        ("msg-basic", Bundle::new(base).unwrap()),
        ("intro-with-pow", Bundle::new(intro).unwrap()),
        ("ack-edge-values", Bundle::new(ack).unwrap()),
    ]
}

#[test]
fn encodings_match_published_vectors() {
    let update = std::env::var_os("MEM_UPDATE_VECTORS").is_some();
    for (name, bundle) in vectors() {
        let path = vector_dir().join(format!("{name}.cbor"));
        let encoded = bundle.encode();
        if update {
            std::fs::write(&path, &encoded).unwrap();
        }
        let published = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("{}: {e}; run with MEM_UPDATE_VECTORS=1", path.display()));
        assert_eq!(
            encoded, published,
            "{name}: encoding differs from published vector"
        );
        assert_eq!(
            Bundle::decode(&published).unwrap(),
            bundle,
            "{name}: decode mismatch"
        );
    }
}

/// Frame and encounter vectors, validated against `frame.cddl` and
/// `encounter.cddl` in CI.
#[test]
fn frame_and_encounter_vectors() {
    use mem_proto::encounter::{BundleEntry, EncounterMsg};
    use mem_proto::frame::{Frame, ObjectKind, TransferId};
    let update = std::env::var_os("MEM_UPDATE_VECTORS").is_some();
    let dir = vector_dir().join("..");
    let frame = Frame::new(
        TransferId([0xa5; 8]),
        ObjectKind::Bundle,
        1000,
        3,
        id(0x10).repeat(4),
    )
    .unwrap();
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("frame/repair-symbol", frame.encode()),
        (
            "intro/intro-content",
            mem_proto::inner::IntroContent {
                card: vec![0xa0; 8],
                recipient_nonce: id(0x70),
                note: Some("hello".into()),
            }
            .encode()
            .unwrap(),
        ),
        (
            "encounter/hello",
            EncounterMsg::Hello {
                version: 1,
                ephemeral_id: id(0x20),
            }
            .encode()
            .unwrap(),
        ),
        (
            "encounter/tags",
            EncounterMsg::Tags(vec![id(0x30), id(0x40)])
                .encode()
                .unwrap(),
        ),
        (
            "encounter/bundles",
            EncounterMsg::Bundles(vec![BundleEntry::bare(vectors()[0].1.encode())])
                .encode()
                .unwrap(),
        ),
        (
            "encounter/bundles-with-proof",
            EncounterMsg::Bundles(vec![BundleEntry {
                bytes: vectors()[0].1.encode(),
                proof: Some(id(0x50)),
            }])
            .encode()
            .unwrap(),
        ),
        (
            "encounter/summary",
            EncounterMsg::Summary {
                salt: id(0x60),
                held: vec![0x81; 64],
                wanted: vec![0x18; 32],
            }
            .encode()
            .unwrap(),
        ),
        (
            "encounter/offer",
            EncounterMsg::Offer(vec![id(0x80), id(0x81)])
                .encode()
                .unwrap(),
        ),
        (
            "encounter/want",
            EncounterMsg::Want(vec![id(0x80)]).encode().unwrap(),
        ),
        (
            "encounter/purge-tokens",
            EncounterMsg::PurgeTokens(vec![id(0x70), id(0x71)])
                .encode()
                .unwrap(),
        ),
        ("encounter/bye", EncounterMsg::Bye.encode().unwrap()),
    ];
    for (name, encoded) in cases {
        let path = dir.join(format!("{name}.cbor"));
        if update {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &encoded).unwrap();
        }
        let published = std::fs::read(&path)
            .unwrap_or_else(|e| panic!("{}: {e}; run with MEM_UPDATE_VECTORS=1", path.display()));
        assert_eq!(encoded, published, "{name}: differs from published vector");
    }
    assert_eq!(
        Frame::decode(&std::fs::read(dir.join("frame/repair-symbol.cbor")).unwrap()).unwrap(),
        frame
    );
}
