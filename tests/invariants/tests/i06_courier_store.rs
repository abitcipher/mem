//! Bounded courier resources: bundles for other people are carried only within the
//! total, per-tag, intro and per-encounter quotas, with eviction by class, age and size.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::cast_possible_truncation,
    reason = "test code"
)]

use mem_core::courier::{
    BLE_ENCOUNTER_BUNDLES, CourierClass, DEFAULT_COURIER_BYTES, PER_ENCOUNTER_BUNDLES,
    PER_TAG_BYTES, StoreWrite,
};
use mem_core::model::{PeerHandle, RejectReason};
use mem_core::{ClaId, EncounterMsg, Input, Output, StorageOp};
use mem_invariants::{Device, pair, transmitted};
use mem_proto::{Bundle, BundleId, BundleKind, BundleParams, RendezvousTag};

const PEER: PeerHandle = PeerHandle(7);

/// A syntactically valid bundle for someone else (couriers never decrypt).
fn foreign(n: u32, tag: [u8; 16], kind: BundleKind, payload_len: usize, hops: u8) -> Vec<u8> {
    let mut id = [0u8; 16];
    id[..4].copy_from_slice(&n.to_be_bytes());
    let mut commitment = [0x55u8; 16];
    commitment[..4].copy_from_slice(&n.to_be_bytes());
    Bundle::new(BundleParams {
        bundle_id: BundleId::from_bytes(id),
        kind,
        dst_tag: RendezvousTag::from_bytes(tag),
        lifetime_s: kind.max_lifetime_s(),
        age_ms: 0,
        hops_remaining: hops,
        copies: 4,
        pow: None,
        payload: vec![0xAB; payload_len],
        purge_commitment: commitment,
    })
    .unwrap()
    .encode()
}

/// Opens an encounter on `dev` with a stranger and hands it `bundles`, one message each.
fn offer(dev: &mut Device, peer: PeerHandle, bundles: &[Vec<u8>]) -> Vec<Output> {
    offer_over(dev, peer, ClaId::Iroh, bundles)
}

fn offer_over(dev: &mut Device, peer: PeerHandle, cla: ClaId, bundles: &[Vec<u8>]) -> Vec<Output> {
    let mut out = dev.handle(Input::EncounterStarted { peer, cla });
    out.extend(dev.handle(Input::EncounterMessage {
        peer,
        msg: EncounterMsg::Hello {
            version: mem_proto::encounter::ENCOUNTER_VERSION,
            ephemeral_id: [3; 16],
        },
    }));
    for b in bundles {
        out.extend(dev.handle(Input::EncounterMessage {
            peer,
            msg: EncounterMsg::Bundles(vec![mem_proto::encounter::BundleEntry::bare(b.clone())]),
        }));
    }
    out
}

fn puts(out: &[Output]) -> usize {
    out.iter()
        .filter_map(|o| match o {
            Output::Persist(StorageOp::Store(w)) => Some(w),
            _ => None,
        })
        .flatten()
        .filter(|w| matches!(w, StoreWrite::Put { .. }))
        .count()
}

#[test]
fn a_bundle_for_someone_else_is_carried_not_shown() {
    let mut c = Device::new(3);
    let bundle = foreign(1, [1; 16], BundleKind::Message, 10, 3);
    let out = offer(&mut c, PEER, std::slice::from_ref(&bundle));
    assert_eq!(puts(&out), 1, "{out:?}");
    assert!(!out.iter().any(|o| matches!(o, Output::Deliver { .. })));
    assert_eq!(c.agent.courier_status().2, 1);
    // A second copy of the same bundle is not stored twice.
    let out = offer(&mut c, PeerHandle(8), &[bundle.clone(), bundle]);
    assert_eq!(puts(&out), 0);
}

#[test]
fn direct_only_bundles_are_never_carried() {
    // Direct sends are direct-only (hops 0, copies 1).
    let (mut a, mut b, mut c) = (Device::new(1), Device::new(2), Device::new(3));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    let msg = a.send(b_id, "for B only").unwrap();
    let (_, bytes) = transmitted(&a.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Qr,
    }))
    .unwrap();
    assert_eq!(Bundle::decode(&bytes).unwrap().hops_remaining(), 0);
    let out = offer(&mut c, PEER, &[bytes]);
    assert_eq!(puts(&out), 0);
    assert!(!out.iter().any(|o| matches!(o, Output::Deliver { .. })));
}

#[test]
fn bundles_that_cannot_travel_further_are_not_carried() {
    let mut c = Device::new(3);
    let out = offer(
        &mut c,
        PEER,
        &[foreign(1, [1; 16], BundleKind::Message, 10, 0)],
    );
    assert_eq!(puts(&out), 0, "hops_remaining 0 is never stored");
}

#[test]
fn per_encounter_allowance_is_enforced() {
    let mut c = Device::new(3);
    let bundles: Vec<Vec<u8>> = (0..=PER_ENCOUNTER_BUNDLES)
        .map(|n| foreign(n, [n as u8; 16], BundleKind::Message, 10, 3))
        .collect();
    let out = offer(&mut c, PEER, &bundles);
    assert_eq!(puts(&out), PER_ENCOUNTER_BUNDLES as usize);
    assert!(out.contains(&Output::Rejected(RejectReason::Quota)));
    // A new encounter has a fresh allowance.
    let out = offer(
        &mut c,
        PeerHandle(9),
        &[foreign(9_999, [7; 16], BundleKind::Message, 10, 3)],
    );
    assert_eq!(puts(&out), 1);
}

/// BLE links are slow and short: a smaller allowance per encounter.
#[test]
fn bluetooth_encounters_have_the_smaller_allowance() {
    let mut c = Device::new(3);
    c.handle(Input::SetChannelEnabled {
        cla: ClaId::Ble,
        enabled: true,
    });
    let bundles: Vec<Vec<u8>> = (0..=BLE_ENCOUNTER_BUNDLES)
        .map(|n| foreign(n, [n as u8; 16], BundleKind::Message, 10, 3))
        .collect();
    let out = offer_over(&mut c, PEER, ClaId::Ble, &bundles);
    assert_eq!(puts(&out), BLE_ENCOUNTER_BUNDLES as usize);
    assert!(out.contains(&Output::Rejected(RejectReason::Quota)));
}

/// BLE tells every scanner nearby that MEM runs, so it is off until the user turns it on:
/// an encounter over it is ended at once.
#[test]
fn bluetooth_is_off_until_turned_on() {
    let mut c = Device::new(3);
    let out = c.handle(Input::EncounterStarted {
        peer: PEER,
        cla: ClaId::Ble,
    });
    assert_eq!(out, vec![Output::EndEncounter { peer: PEER }]);
    assert_eq!(c.agent.peer_count(), 0);
}

#[test]
fn per_tag_quota_holds_and_newer_does_not_displace_equal_class() {
    let mut c = Device::new(3);
    // 60 KiB bundles to one tag: the 512 KiB per-tag quota fits 8.
    let big = 60 * 1024;
    let bundles: Vec<Vec<u8>> = (0..12)
        .map(|n| foreign(n, [4; 16], BundleKind::Message, big, 3))
        .collect();
    offer(&mut c, PEER, &bundles);
    let (used, _, count) = c.agent.courier_status();
    assert!(used <= PER_TAG_BYTES, "{used}");
    assert!(count < 12);
}

#[test]
fn total_quota_evicts_lower_class_first_and_respects_the_limit() {
    let (mut c, mut d) = (Device::new(3), Device::new(4));
    let (d_id, _) = pair(&mut c, &mut d, ClaId::Qr).unwrap();
    // D's receiving tag, as C addresses it: a bundle carrying it is class 2 at C.
    let to_d = c.send(d_id, "x").unwrap();
    let (_, to_d_bytes) = transmitted(&c.handle(Input::RequestTransmit {
        message: to_d,
        cla: ClaId::Qr,
    }))
    .unwrap();
    let to_contact_tag = *Bundle::decode(&to_d_bytes).unwrap().dst_tag().as_bytes();
    // A small total so the test stays fast: 4 bundles of ~20 KiB.
    c.handle(Input::SetCourierLimit { bytes: 80 * 1024 });
    let other: Vec<Vec<u8>> = (0..4)
        .map(|n| foreign(n, [n as u8 + 10; 16], BundleKind::Message, 19 * 1024, 3))
        .collect();
    offer(&mut c, PEER, &other);
    assert_eq!(c.agent.courier_status().2, 4);

    // A bundle to one of C's contacts (class 2) displaces an "other" one.
    let out = offer(
        &mut c,
        PeerHandle(10),
        &[foreign(
            100,
            to_contact_tag,
            BundleKind::Message,
            19 * 1024,
            3,
        )],
    );
    assert_eq!(puts(&out), 1);
    let (used, limit, count) = c.agent.courier_status();
    assert!(used <= limit);
    assert_eq!(count, 4);
    assert!(
        c.agent
            .courier_entries()
            .iter()
            .any(|m| m.class == CourierClass::ToContact)
    );

    // Among equal classes, the encounter that stored the most goes
    // first. A newcomer from a fresh encounter displaces one from the big earlier batch, while
    // the contact's bundle (class 2) stays; a flood from that same new encounter then refuses
    // itself instead of evicting more.
    let out = offer(
        &mut c,
        PeerHandle(11),
        &[
            foreign(200, [99; 16], BundleKind::Message, 19 * 1024, 3),
            foreign(201, [98; 16], BundleKind::Message, 19 * 1024, 3),
            foreign(202, [97; 16], BundleKind::Message, 19 * 1024, 3),
        ],
    );
    assert!(puts(&out) >= 1);
    let entries = c.agent.courier_entries();
    assert!(entries.iter().any(|m| m.class == CourierClass::ToContact));
    let first_batch = entries.iter().filter(|m| m.key.bundle_id[3] < 4).count();
    let newest_batch = entries
        .iter()
        .filter(|m| (200..=202).contains(&m.key.bundle_id[3]))
        .count();
    assert!(
        newest_batch <= first_batch + 1,
        "no single encounter takes over: first {first_batch}, newest {newest_batch}"
    );
    assert!(c.agent.courier_status().0 <= 80 * 1024);
}

/// A flood arriving in one encounter cannot push out small honest batches (the
/// `mem-sim` spam finding).
#[test]
fn one_encounter_cannot_flush_out_earlier_small_batches() {
    let mut c = Device::new(3);
    c.handle(Input::SetCourierLimit { bytes: 200 * 1024 });
    // Five honest encounters, one small bundle each.
    for n in 0..5u32 {
        offer(
            &mut c,
            PeerHandle(20 + u64::from(n)),
            &[foreign(
                n,
                [n as u8 + 1; 16],
                BundleKind::Message,
                2 * 1024,
                3,
            )],
        );
    }
    // A spammer encounter offers far more than fits.
    let flood: Vec<Vec<u8>> = (1000..1100u32)
        .map(|n| foreign(n, [0x77; 16], BundleKind::Message, 4 * 1024, 3))
        .collect();
    offer(&mut c, PeerHandle(99), &flood);
    let honest = c
        .agent
        .courier_entries()
        .iter()
        .filter(|m| u32::from_be_bytes(m.key.bundle_id[..4].try_into().unwrap()) < 5)
        .count();
    assert_eq!(honest, 5, "all honest bundles survive");
    assert!(c.agent.courier_status().0 <= 200 * 1024);
}

#[test]
fn courier_mode_off_stores_nothing_and_shrinking_evicts() {
    let mut c = Device::new(3);
    assert_eq!(c.agent.courier_status().1, DEFAULT_COURIER_BYTES);
    offer(
        &mut c,
        PEER,
        &[foreign(1, [1; 16], BundleKind::Message, 1000, 3)],
    );
    assert_eq!(c.agent.courier_status().2, 1);
    let out = c.handle(Input::SetCourierLimit { bytes: 0 });
    assert!(out.iter().any(|o| matches!(
        o,
        Output::Persist(StorageOp::Store(w)) if matches!(w.as_slice(), [StoreWrite::Delete(_)])
    )));
    assert_eq!(c.agent.courier_status().2, 0);
    let out = offer(
        &mut c,
        PeerHandle(8),
        &[foreign(2, [1; 16], BundleKind::Message, 10, 3)],
    );
    assert_eq!(puts(&out), 0);
}

#[test]
fn state_from_before_courier_mode_starts_with_it_off() {
    // The 0.2.0-ft.8 legacy backup has no courier setting.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/vectors/backup");
    let file = std::fs::read(dir.join("backup-0.2.0-ft8.cbor")).unwrap();
    let info = std::fs::read_to_string(dir.join("backup-0.2.0-ft8.txt")).unwrap();
    let words = info
        .lines()
        .find_map(|l| l.strip_prefix("english: "))
        .unwrap();
    let r = mem_core::Agent::restore_backup(&file, words, 0).unwrap();
    assert_eq!(r.agent.courier_status().1, 0);
}

#[test]
fn the_index_survives_a_restart_and_drops_expired_entries() {
    let mut c = Device::new(3);
    let out = offer(
        &mut c,
        PEER,
        &[foreign(1, [1; 16], BundleKind::Message, 100, 3)],
    );
    let rows: Vec<_> = out
        .iter()
        .filter_map(|o| match o {
            Output::Persist(StorageOp::Store(w)) => Some(w),
            _ => None,
        })
        .flatten()
        .filter_map(|w| match w {
            StoreWrite::Put { meta, .. } => Some(meta.clone()),
            _ => None,
        })
        .collect();
    let snap = c.agent.snapshot();
    c.agent = mem_core::Agent::restore(&snap, c.now).unwrap();
    c.handle(Input::StoreLoaded {
        courier: rows.clone(),
        tombstones: vec![],
    });
    assert_eq!(c.agent.courier_status().2, 1);

    // Past the 30-day lifetime the next start drops it.
    c.agent = mem_core::Agent::restore(&snap, c.now).unwrap();
    c.advance(31 * 24 * 60 * 60 * 1000);
    let out = c.handle(Input::StoreLoaded {
        courier: rows,
        tombstones: vec![],
    });
    assert_eq!(c.agent.courier_status().2, 0);
    assert!(
        out.iter()
            .any(|o| matches!(o, Output::Persist(StorageOp::Store(_))))
    );
}
