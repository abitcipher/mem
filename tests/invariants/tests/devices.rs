//! Several devices for one identity, end to end through a simulated hub: B adds a
//! device B2 in person; contact A learns B's list and writes to both; B2 writes to A with its
//! certified device key; contacts B pairs later reach B2 by own-device sync; retiring B2 stops
//! it and is noticed everywhere.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::collections::BTreeMap;

use mem_core::model::{ContactId, DeliveryStatus};
use mem_core::{ClaId, Crossing, DeviceJoin, HubOrigin, Input, Output, PrivacyGrade};
use mem_crypto::PurgeToken;
use mem_crypto::purge::commit;
use mem_invariants::{Device, MemStore, NOW, pair};
use mem_proto::Bundle;
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

/// A hub as `spec/hub.cddl` describes it: by tag, fetch by prefix, delete only by purge token.
#[derive(Default)]
struct Hub {
    stored: BTreeMap<([u8; 16], [u8; 16]), Vec<u8>>,
}

impl Hub {
    fn put(&mut self, bytes: &[u8]) {
        let b = Bundle::decode(bytes).unwrap();
        self.stored.insert(
            (*b.dst_tag().as_bytes(), *b.purge_commitment()),
            bytes.to_vec(),
        );
    }

    fn fetch(&self, prefixes: &[[u8; 2]]) -> Vec<(Vec<u8>, HubOrigin)> {
        self.stored
            .iter()
            .filter(|((tag, _), _)| prefixes.iter().any(|p| tag[..2] == p[..]))
            .map(|(_, b)| (b.clone(), HubOrigin::OwnOperator))
            .collect()
    }

    fn purge(&mut self, tokens: &[[u8; 16]]) {
        for t in tokens {
            let c = *commit(&PurgeToken::from_bytes(*t)).as_bytes();
            self.stored.retain(|(_, commitment), _| *commitment != c);
        }
    }
}

/// Serves a device's outputs against the hub until quiet, as the runtime does.
fn drive(dev: &mut Device, hub: &mut Hub, mut pending: Vec<Output>) -> Vec<Output> {
    let mut all = Vec::new();
    for _ in 0..64 {
        if pending.is_empty() {
            break;
        }
        let mut next = Vec::new();
        for o in &pending {
            match o {
                Output::Transmit {
                    op,
                    cla: ClaId::Hub,
                    bytes,
                    crossing,
                    ..
                } => {
                    assert_eq!(*crossing, Some(Crossing::OwnOperator), "no partners here");
                    hub.put(bytes);
                    next.extend(dev.handle(Input::TransmitResult {
                        op: *op,
                        result: Ok(()),
                    }));
                }
                Output::HubFetch { prefixes } => {
                    for chunk in hub.fetch(prefixes).chunks(16) {
                        next.extend(dev.handle(Input::HubBundles {
                            bundles: chunk.to_vec(),
                        }));
                    }
                    next.extend(dev.handle(Input::HubFetchDone));
                }
                Output::HubPurge { tokens } => hub.purge(tokens),
                _ => {}
            }
        }
        all.append(&mut pending);
        pending = next;
    }
    all
}

/// One round for a device: connect (fetch everything, upload what is due), after a tick.
fn sync(dev: &mut Device, hub: &mut Hub) -> Vec<Output> {
    let first = dev.handle(Input::HubDisconnected);
    let mut out = drive(dev, hub, first);
    let ticked = dev.tick();
    out.extend(drive(dev, hub, ticked));
    let connected = dev.handle(Input::HubConnected);
    out.extend(drive(dev, hub, connected));
    out
}

fn hub_on(dev: &mut Device) {
    dev.handle(Input::SetChannelEnabled {
        cla: ClaId::Hub,
        enabled: true,
    });
}

/// B approves a joining device in person; returns the new device.
fn add_device(b: &mut Device, seed: u8) -> Device {
    let mut rng = ChaCha20Rng::from_seed([seed; 32]);
    let join = DeviceJoin::new(&mut rng);
    let out = b.handle(Input::ApproveDevice {
        request: join.request(),
    });
    let setup = out
        .iter()
        .find_map(|o| match o {
            Output::DeviceSetupReady { setup } => Some(setup.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no setup: {out:?}"));
    let agent = join.finish(&setup, b.now).unwrap();
    Device {
        agent,
        rng,
        now: b.now,
        store: MemStore::default(),
    }
}

/// Rounds of everyone syncing with the hub.
fn rounds(devs: &mut [&mut Device], hub: &mut Hub, n: usize) -> Vec<Vec<Output>> {
    let mut outs: Vec<Vec<Output>> = devs.iter().map(|_| Vec::new()).collect();
    for _ in 0..n {
        for (i, d) in devs.iter_mut().enumerate() {
            d.advance(3_000);
            outs[i].extend(sync(d, hub));
        }
    }
    outs
}

/// Sends and serves whatever goes out at once (the runtime reports every transmission).
fn send(dev: &mut Device, hub: &mut Hub, to: ContactId, text: &str) {
    let out = dev.handle(Input::UserSend {
        contact: to,
        text: text.into(),
    });
    assert!(
        out.iter().any(|o| matches!(o, Output::Queued { .. })),
        "{out:?}"
    );
    drive(dev, hub, out);
}

fn texts(dev: &Device, contact: ContactId) -> Vec<String> {
    dev.agent
        .messages(contact)
        .iter()
        .map(|m| m.text.clone())
        .collect()
}

#[test]
fn a_new_device_joins_writes_and_is_written_to() {
    let (mut a, mut b) = (Device::new(201), Device::new(202));
    let (b_id, a_id) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    for d in [&mut a, &mut b] {
        hub_on(d);
    }
    let mut b2 = add_device(&mut b, 203);
    hub_on(&mut b2);

    // B2 starts as another device: no identity key, A already a contact, same identity.
    assert!(b.agent.is_main_device());
    assert!(!b2.agent.is_main_device());
    assert_eq!(b2.agent.fingerprint(), b.agent.fingerprint());
    assert_eq!(
        b2.agent.contacts().len(),
        1,
        "A, and never the self contact"
    );
    assert_eq!(b.agent.contacts().len(), 1);
    assert_eq!(b.agent.own_devices().len(), 2);
    assert!(b2.agent.own_devices()[1].this_device);
    // Only the main device pairs (Q2).
    assert_eq!(
        b2.handle(Input::ImportCard {
            cla: ClaId::Qr,
            bytes: Device::new(209).card(),
        }),
        vec![Output::Rejected(
            mem_core::model::RejectReason::MainDeviceOnly
        )]
    );

    // B shows B2 as waiting until B2 reports in with its own signed key update.
    assert!(!b.agent.own_devices()[1].joined);
    assert!(b2.agent.own_devices().iter().all(|d| d.joined));

    // B's next key update brings A the new list at once (no rotation wait).
    let mut hub = Hub::default();
    rounds(&mut [&mut a, &mut b, &mut b2], &mut hub, 3);
    assert_eq!(a.agent.contact_devices(b_id), 2, "A knows B2");
    assert!(b.agent.own_devices()[1].joined, "B2 reported in");

    // B2 writes to A, signed with its certified device key.
    send(&mut b2, &mut hub, a_id, "from the tablet");
    rounds(&mut [&mut a, &mut b, &mut b2], &mut hub, 3);
    assert!(texts(&a, b_id).contains(&"from the tablet".to_owned()));
    let got = a
        .agent
        .messages(b_id)
        .into_iter()
        .find(|m| m.text == "from the tablet")
        .unwrap();
    assert_eq!(got.channels[0].grade, PrivacyGrade::P2);
    // A's ack reaches B2 (the device that wrote), not B.
    rounds(&mut [&mut a, &mut b, &mut b2], &mut hub, 3);
    let sent = b2.agent.messages(a_id);
    assert_eq!(sent[0].status, DeliveryStatus::Delivered, "{sent:?}");

    // A writes to B: both devices get it; A's message is delivered at the first ack.
    send(&mut a, &mut hub, b_id, "to both");
    rounds(&mut [&mut a, &mut b, &mut b2], &mut hub, 3);
    assert!(texts(&b, a_id).contains(&"to both".to_owned()));
    assert!(texts(&b2, a_id).contains(&"to both".to_owned()));
    let mine = a
        .agent
        .messages(b_id)
        .into_iter()
        .find(|m| m.text == "to both")
        .unwrap();
    assert_eq!(mine.status, DeliveryStatus::Delivered);
    // One visible message, whatever the copies.
    assert_eq!(
        a.agent
            .messages(b_id)
            .iter()
            .filter(|m| m.text == "to both")
            .count(),
        1
    );

    // B pairs C later: own-device sync brings C to B2.
    let mut c = Device::new(204);
    pair(&mut b, &mut c, ClaId::Qr).unwrap();
    rounds(&mut [&mut a, &mut b, &mut b2], &mut hub, 3);
    assert_eq!(b2.agent.contacts().len(), 2, "C arrived by sync");
    // B removes C: B2 removes it too.
    let c_id = b
        .agent
        .contacts()
        .into_iter()
        .find(|x| x.id != b_id && x.id != a_id)
        .unwrap()
        .id;
    b.handle(Input::RemoveContact(c_id));
    rounds(&mut [&mut a, &mut b, &mut b2], &mut hub, 3);
    assert_eq!(b2.agent.contacts().len(), 1, "C left by sync");

    // B retires B2: B2 learns it, A stops writing to it.
    let tablet = b2
        .agent
        .own_devices()
        .into_iter()
        .find(|d| d.this_device)
        .unwrap()
        .id;
    b.handle(Input::RetireDevice { device: tablet });
    let outs = rounds(&mut [&mut a, &mut b, &mut b2], &mut hub, 4);
    let outs: Vec<Output> = outs.into_iter().flatten().collect();
    assert!(
        outs.iter().any(|o| matches!(o, Output::DeviceRetired)),
        "B2 was told"
    );
    assert_eq!(a.agent.contact_devices(b_id), 1, "A dropped B2");
}

/// A message signed by a device that is not in the newest list is refused, and
/// a device's request with a bad signature is not approved.
#[test]
fn certificates_must_match_the_list() {
    let mut b = Device::new(211);
    let mut rng = ChaCha20Rng::from_seed([212; 32]);
    let join = DeviceJoin::new(&mut rng);
    let mut request = join.request();
    let last = request.len() - 1;
    request[last] ^= 1;
    assert!(
        b.handle(Input::ApproveDevice { request })
            .contains(&Output::Rejected(
                mem_core::model::RejectReason::BadSignature
            ))
    );
    // At most four devices.
    for seed in 213..216 {
        add_device(&mut b, seed);
    }
    assert_eq!(b.agent.own_devices().len(), 4);
    let mut rng = ChaCha20Rng::from_seed([216; 32]);
    let join = DeviceJoin::new(&mut rng);
    assert!(
        b.handle(Input::ApproveDevice {
            request: join.request()
        })
        .contains(&Output::Rejected(
            mem_core::model::RejectReason::TooManyDevices
        ))
    );
    // A setup opens only on the device that asked for it.
    let mut rng = ChaCha20Rng::from_seed([217; 32]);
    let other = DeviceJoin::new(&mut rng);
    let mut b3 = Device::new(218);
    let mut rng = ChaCha20Rng::from_seed([219; 32]);
    let asker = DeviceJoin::new(&mut rng);
    let out = b3.handle(Input::ApproveDevice {
        request: asker.request(),
    });
    let setup = out
        .iter()
        .find_map(|o| match o {
            Output::DeviceSetupReady { setup } => Some(setup.clone()),
            _ => None,
        })
        .unwrap();
    assert!(other.finish(&setup, NOW).is_err());
    assert!(asker.finish(&setup, NOW).is_ok());
}

/// A device approved on the main device that has not opened its setup stays "waiting". If it is
/// removed and still reads the setup later (it was on screen), the farewell tells it it is out.
#[test]
fn a_device_removed_before_joining_learns_it_is_out() {
    let (mut a, mut b) = (Device::new(221), Device::new(222));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    for d in [&mut a, &mut b] {
        hub_on(d);
    }
    let mut rng = ChaCha20Rng::from_seed([223; 32]);
    let join = DeviceJoin::new(&mut rng);
    let out = b.handle(Input::ApproveDevice {
        request: join.request(),
    });
    let setup = out
        .iter()
        .find_map(|o| match o {
            Output::DeviceSetupReady { setup } => Some(setup.clone()),
            _ => None,
        })
        .unwrap();
    let mut hub = Hub::default();
    rounds(&mut [&mut a, &mut b], &mut hub, 3);
    let waiting = b.agent.own_devices()[1];
    assert!(!waiting.joined, "it has not reported in");
    assert_eq!(a.agent.contact_devices(b_id), 2, "A was told the list");

    b.handle(Input::RetireDevice { device: waiting.id });
    assert_eq!(b.agent.own_devices().len(), 1);
    rounds(&mut [&mut a, &mut b], &mut hub, 3);
    assert_eq!(a.agent.contact_devices(b_id), 1, "A dropped it");

    // The new device reads the setup after all: it hears it was removed.
    let mut late = Device {
        agent: join.finish(&setup, b.now).unwrap(),
        rng,
        now: b.now,
        store: MemStore::default(),
    };
    hub_on(&mut late);
    let outs = rounds(&mut [&mut a, &mut b, &mut late], &mut hub, 3);
    assert!(
        outs[2].iter().any(|o| matches!(o, Output::DeviceRetired)),
        "the late device was told"
    );
}
