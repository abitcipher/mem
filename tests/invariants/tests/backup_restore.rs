//! A backup restores the same identity, contacts and keys on a new device, which keeps
//! talking to existing contacts; the phrase works in either form; hostile files are refused
//! before any key derivation.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::doc_markdown,
    reason = "test code"
)]

use mem_core::model::{DeliveryStatus, RejectReason};
use mem_core::{Agent, BackupError, BackupFailure, ClaId, Input, KdfParams, Output, PhraseForm};
use mem_invariants::{Device, pair, transmitted};

/// Small Argon2 parameters keep tests fast; the defaults are exercised in mem-crypto.
const FAST: KdfParams = KdfParams {
    m_kib: 19_456,
    t: 1,
    p: 1,
};

fn qr_send(from: &mut Device, to: &mut Device, contact: mem_core::model::ContactId, text: &str) {
    let msg = from.send(contact, text).unwrap();
    let out = from.handle(Input::RequestTransmit {
        message: msg,
        cla: ClaId::Qr,
    });
    let (op, bytes) = transmitted(&out).unwrap();
    from.handle(Input::TransmitResult { op, result: Ok(()) });
    let out = to.handle(Input::Received {
        cla: ClaId::Qr,
        bytes,
    });
    assert!(
        out.iter().any(|o| matches!(o, Output::Deliver { .. })),
        "{out:?}"
    );
}

fn phrase(dev: &Device) -> String {
    dev.agent.recovery_phrase().unwrap().join(" ")
}

#[test]
fn restored_device_keeps_talking_to_contacts() {
    let (mut a, mut b) = (Device::new(91), Device::new(92));
    let (b_id, a_id) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    qr_send(&mut a, &mut b, b_id, "before");
    qr_send(&mut b, &mut a, a_id, "reply");
    // A new phrase changes nothing until it is confirmed; then it is saved.
    assert_eq!(
        a.handle(Input::NewRecoveryPhrase {
            form: PhraseForm::English
        }),
        vec![]
    );
    assert!(!a.agent.backups_enabled());
    assert!(
        a.handle(Input::ConfirmRecoveryPhrase)
            .contains(&Output::Persist(mem_core::StorageOp::SaveSnapshot))
    );
    assert!(a.agent.backups_enabled());
    let file = a
        .agent
        .make_backup_with(
            true,
            "test",
            FAST,
            &mut rand_chacha::ChaCha20Rng::from_seed([1; 32]),
        )
        .unwrap();
    let words = phrase(&a);

    // A new phone: restore, same fingerprint, contacts and history.
    let restored = Agent::restore_backup(&file, &words, a.now).unwrap();
    assert!(restored.history);
    assert!(restored.corrected.is_none());
    let mut a2 = Device::new(93);
    a2.agent = restored.agent;
    assert_eq!(a2.agent.fingerprint(), a.agent.fingerprint());
    assert_eq!(a2.agent.contacts().len(), 1);
    let texts: Vec<String> = a2
        .agent
        .messages(b_id)
        .into_iter()
        .map(|m| m.text)
        .collect();
    assert_eq!(texts, ["before", "reply"]);
    // The phrase is the same on the new phone.
    assert_eq!(phrase(&a2), words);

    // Both directions still work: B does not notice the move.
    qr_send(&mut a2, &mut b, b_id, "from the new phone");
    qr_send(&mut b, &mut a2, a_id, "welcome back");
    assert_eq!(b.agent.messages(a_id).len(), 4);
}

#[test]
fn without_history_keeps_contacts_and_unsent_messages() {
    let (mut a, mut b) = (Device::new(94), Device::new(95));
    let (b_id, a_id) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    qr_send(&mut a, &mut b, b_id, "delivered long ago");
    // B's reply confirms it (piggybacked ack), so it is history, not an unsent message.
    qr_send(&mut b, &mut a, a_id, "got it");
    assert_eq!(a.agent.messages(b_id)[0].status, DeliveryStatus::Delivered);
    let queued = a.send(b_id, "still queued").unwrap();
    a.handle(Input::NewRecoveryPhrase {
        form: PhraseForm::Proquint,
    });
    a.handle(Input::ConfirmRecoveryPhrase);
    let file = a
        .agent
        .make_backup_with(
            false,
            "test",
            FAST,
            &mut rand_chacha::ChaCha20Rng::from_seed([2; 32]),
        )
        .unwrap();
    let restored = Agent::restore_backup(&file, &phrase(&a), a.now).unwrap();
    assert!(!restored.history);
    let msgs = restored.agent.messages(b_id);
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].id, queued);
    assert_eq!(msgs[0].status, DeliveryStatus::Queued);
    assert_eq!(restored.agent.contacts().len(), 1);
    // Still sendable after the restore.
    assert_ne!(restored.agent.channel_options(queued).len(), 0);
}

#[test]
fn either_phrase_form_opens_the_backup_and_a_wrong_phrase_does_not() {
    let (mut a, mut b) = (Device::new(96), Device::new(97));
    pair(&mut a, &mut b, ClaId::Qr).unwrap();
    a.handle(Input::NewRecoveryPhrase {
        form: PhraseForm::English,
    });
    a.handle(Input::ConfirmRecoveryPhrase);
    let file = a
        .agent
        .make_backup_with(
            true,
            "test",
            FAST,
            &mut rand_chacha::ChaCha20Rng::from_seed([3; 32]),
        )
        .unwrap();
    // Shown in the other form: the secret, and so the backup, is unchanged.
    a.handle(Input::SetPhraseForm {
        form: PhraseForm::Proquint,
    });
    let proquints = phrase(&a);
    assert_eq!(proquints.split(' ').count(), 9);
    assert!(Agent::restore_backup(&file, &proquints, a.now).is_ok());

    // Another device's phrase does not open it.
    let mut c = Device::new(98);
    c.handle(Input::NewRecoveryPhrase {
        form: PhraseForm::English,
    });
    c.handle(Input::ConfirmRecoveryPhrase);
    assert!(matches!(
        Agent::restore_backup(&file, &phrase(&c), a.now),
        Err(BackupFailure::Crypto(BackupError::WrongPhraseOrCorrupt))
    ));
    // Garbage is not a backup.
    assert!(matches!(
        Agent::restore_backup(b"hello", &proquints, a.now),
        Err(BackupFailure::NotABackup)
    ));
    // No secret, no backup.
    assert!(matches!(
        b.agent.make_backup(
            true,
            "test",
            &mut rand_chacha::ChaCha20Rng::from_seed([4; 32])
        ),
        Err(BackupFailure::NotEnabled)
    ));
}

/// A file asking for 4 GiB of Argon2 memory is refused before any key derivation, so the
/// call returns at once.
#[test]
fn hostile_kdf_parameters_are_refused_before_work() {
    let mut a = Device::new(99);
    a.handle(Input::NewRecoveryPhrase {
        form: PhraseForm::English,
    });
    a.handle(Input::ConfirmRecoveryPhrase);
    let file = a
        .agent
        .make_backup_with(
            true,
            "test",
            FAST,
            &mut rand_chacha::ChaCha20Rng::from_seed([5; 32]),
        )
        .unwrap();
    let mut decoded = mem_proto::backup::BackupFile::decode(&file).unwrap();
    decoded.header.kdf.m_kib = 4 * 1024 * 1024;
    let hostile = decoded.encode();
    let started = std::time::Instant::now();
    assert!(matches!(
        Agent::restore_backup(&hostile, &phrase(&a), a.now),
        Err(BackupFailure::Crypto(BackupError::KdfParams))
    ));
    assert!(started.elapsed() < std::time::Duration::from_millis(500));
}

use rand_core::SeedableRng;

/// Published vector (`spec/vectors/backup/`): a fixed identity and secret, default KDF parameters.
/// Regenerate with `MEM_UPDATE_VECTORS=1`. The test restores it with both phrase forms.
#[test]
fn published_backup_vector_restores() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/vectors/backup");
    let mut a = Device::new(100);
    a.handle(Input::NewRecoveryPhrase {
        form: PhraseForm::Proquint,
    });
    a.handle(Input::ConfirmRecoveryPhrase);
    let file = a
        .agent
        .make_backup(
            true,
            "vector",
            &mut rand_chacha::ChaCha20Rng::from_seed([7; 32]),
        )
        .unwrap();
    let syllables = phrase(&a);
    a.handle(Input::SetPhraseForm {
        form: PhraseForm::English,
    });
    let english = phrase(&a);
    let info = format!(
        "fingerprint: {}\nsyllables: {syllables}\nenglish: {english}\n",
        a.agent.fingerprint()
    );
    if std::env::var_os("MEM_UPDATE_VECTORS").is_some() {
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("backup.cbor"), &file).unwrap();
        std::fs::write(dir.join("backup.txt"), &info).unwrap();
    }
    let published = std::fs::read(dir.join("backup.cbor")).unwrap();
    assert_eq!(published, file, "vector changed; regenerate deliberately");
    assert_eq!(
        std::fs::read_to_string(dir.join("backup.txt")).unwrap(),
        info
    );
    for words in [&syllables, &english] {
        let r = Agent::restore_backup(&published, words, a.now).unwrap();
        assert_eq!(r.agent.fingerprint(), a.agent.fingerprint());
    }
}

/// A backup written before the default-floor setting existed (builds up to 0.2.0-ft.8) still
/// restores, and new contacts then get the default floor P2.
#[test]
fn backup_from_before_the_default_floor_restores() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/vectors/backup");
    let file = std::fs::read(dir.join("backup-0.2.0-ft8.cbor")).unwrap();
    let info = std::fs::read_to_string(dir.join("backup-0.2.0-ft8.txt")).unwrap();
    let field = |name: &str| {
        info.lines()
            .find_map(|l| l.strip_prefix(name))
            .unwrap()
            .to_owned()
    };
    let r = Agent::restore_backup(&file, &field("english: "), 0).unwrap();
    assert_eq!(r.agent.fingerprint(), field("fingerprint: "));
    assert_eq!(r.agent.default_floor(), mem_core::PrivacyGrade::P2);
}

/// A backup written before key rotation and contact requests still
/// restores: one mailbox key becomes serial 1, nothing is requested, no requests are pending.
#[test]
fn backup_from_before_key_rotation_restores() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/vectors/backup");
    let file = std::fs::read(dir.join("backup-pre-rotation.cbor")).unwrap();
    let info = std::fs::read_to_string(dir.join("backup-pre-rotation.txt")).unwrap();
    let field = |name: &str| {
        info.lines()
            .find_map(|l| l.strip_prefix(name))
            .unwrap()
            .to_owned()
    };
    let r = Agent::restore_backup(&file, &field("english: "), 0).unwrap();
    assert_eq!(r.agent.fingerprint(), field("fingerprint: "));
    assert_eq!(r.agent.requests().len(), 0);
    assert!(!r.agent.accepts_requests_nearby());
}

/// A backup written before Bluetooth existed restores with Bluetooth off: an update
/// never turns on a channel that tells nearby scanners MEM runs.
#[test]
fn backup_from_before_bluetooth_keeps_it_off() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/vectors/backup");
    let file = std::fs::read(dir.join("backup-pre-ble.cbor")).unwrap();
    let info = std::fs::read_to_string(dir.join("backup-pre-ble.txt")).unwrap();
    let english = info
        .lines()
        .find_map(|l| l.strip_prefix("english: "))
        .unwrap();
    let r = Agent::restore_backup(&file, english, 0).unwrap();
    assert!(!r.agent.channel_enabled(mem_core::ClaId::Ble));
    assert!(r.agent.channel_enabled(mem_core::ClaId::Iroh));
}

/// A backup written before groups existed restores with no groups and every contact
/// a full one.
#[test]
fn backup_from_before_groups_restores() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/vectors/backup");
    let file = std::fs::read(dir.join("backup-pre-groups.cbor")).unwrap();
    let info = std::fs::read_to_string(dir.join("backup-pre-groups.txt")).unwrap();
    let english = info
        .lines()
        .find_map(|l| l.strip_prefix("english: "))
        .unwrap();
    let r = Agent::restore_backup(&file, english, 0).unwrap();
    assert_eq!(r.agent.groups(), Vec::new());
}

/// The phrase takes effect only after the user confirms it. Leaving the check, or
/// a restart in the middle, leaves backups off; replacing a phrase keeps the old one until then.
#[test]
fn unconfirmed_phrase_never_takes_effect() {
    let mut a = Device::new(110);
    a.handle(Input::NewRecoveryPhrase {
        form: PhraseForm::English,
    });
    assert!(a.agent.recovery_pending());
    assert!(!a.agent.backups_enabled());
    assert!(matches!(
        a.agent
            .make_backup(true, "x", &mut rand_chacha::ChaCha20Rng::from_seed([1; 32])),
        Err(BackupFailure::NotEnabled)
    ));
    // Restart mid-flow: the pending phrase is gone, nothing was stored.
    a.agent = Agent::restore(&a.agent.snapshot(), a.now).unwrap();
    assert!(!a.agent.recovery_pending());
    assert!(a.agent.recovery_phrase().is_none());
    assert_eq!(
        a.handle(Input::ConfirmRecoveryPhrase),
        vec![Output::Rejected(RejectReason::NoPendingPhrase)]
    );

    // Turned on properly, then a replacement is started and abandoned.
    a.handle(Input::NewRecoveryPhrase {
        form: PhraseForm::English,
    });
    a.handle(Input::ConfirmRecoveryPhrase);
    let old = phrase(&a);
    a.handle(Input::NewRecoveryPhrase {
        form: PhraseForm::Proquint,
    });
    assert_ne!(phrase(&a), old, "the new phrase is shown while pending");
    a.handle(Input::DiscardRecoveryPhrase);
    assert_eq!(phrase(&a), old);
    let file = a
        .agent
        .make_backup_with(
            true,
            "x",
            FAST,
            &mut rand_chacha::ChaCha20Rng::from_seed([2; 32]),
        )
        .unwrap();
    assert!(Agent::restore_backup(&file, &old, a.now).is_ok());
}

/// A backup written before the hub network restores with no hub list: hubs come only
/// from the user's own address until a signed list arrives.
#[test]
fn backup_from_before_the_hub_network_restores() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/vectors/backup");
    let file = std::fs::read(dir.join("backup-pre-hubnet.cbor")).unwrap();
    let info = std::fs::read_to_string(dir.join("backup-pre-hubnet.txt")).unwrap();
    let field = |name: &str| {
        info.lines()
            .find_map(|l| l.strip_prefix(name))
            .unwrap()
            .to_owned()
    };
    let r = Agent::restore_backup(&file, &field("english: "), 0).unwrap();
    assert_eq!(r.agent.fingerprint(), field("fingerprint: "));
    assert!(r.agent.hub_list().is_none());
    assert_eq!(r.agent.hub_candidates(), Vec::<String>::new());
}

/// A backup written before partner operators restores with "Only my operator's
/// hubs" off, the default.
#[test]
fn backup_from_before_partner_hubs_restores() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/vectors/backup");
    let file = std::fs::read(dir.join("backup-pre-federation.cbor")).unwrap();
    let info = std::fs::read_to_string(dir.join("backup-pre-federation.txt")).unwrap();
    let field = |name: &str| {
        info.lines()
            .find_map(|l| l.strip_prefix(name))
            .unwrap()
            .to_owned()
    };
    let r = Agent::restore_backup(&file, &field("english: "), 0).unwrap();
    assert_eq!(r.agent.fingerprint(), field("fingerprint: "));
    assert!(!r.agent.own_operator_only());
    assert_eq!(r.agent.hub_partners(), []);
}

/// A backup written before several devices restores with every contact on one
/// device and no device list.
#[test]
fn backup_from_before_devices_restores() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec/vectors/backup");
    let file = std::fs::read(dir.join("backup-pre-devices.cbor")).unwrap();
    let info = std::fs::read_to_string(dir.join("backup-pre-devices.txt")).unwrap();
    let field = |name: &str| {
        info.lines()
            .find_map(|l| l.strip_prefix(name))
            .unwrap()
            .to_owned()
    };
    let r = Agent::restore_backup(&file, &field("english: "), 0).unwrap();
    assert_eq!(r.agent.fingerprint(), field("fingerprint: "));
    for c in r.agent.contacts() {
        assert_eq!(r.agent.contact_devices(c.id), 1);
    }
}
