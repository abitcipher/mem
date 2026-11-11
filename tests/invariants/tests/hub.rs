//! The hub channel, against a simulated hub that follows the protocol's rules: it
//! stores envelopes by tag, answers fetches by 2-byte prefix, never deletes on fetch, and deletes
//! only when shown a purge token whose commitment matches.
//!
//! Partner operators: with `partner_round_trip` set, a bundle uploaded as "may
//! cross" is fetched back marked as from another operator, as if a partner's hub carried it.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::collections::BTreeMap;

use mem_core::model::DeliveryStatus;
use mem_core::{ClaId, Crossing, HubOrigin, Input, Output, PrivacyGrade};
use mem_crypto::IdentityKey;
use mem_crypto::PurgeToken;
use mem_crypto::purge::commit;
use mem_invariants::{Device, NOW, pair};
use mem_proto::Bundle;
use mem_proto::hublist::{HUB_LIST_DOMAIN, HubEntry, HubList, PartnerEntry};

/// An envelope and whether it was uploaded as "may cross".
type Held = (Vec<u8>, bool);

/// A hub as `spec/hub.cddl` describes it.
#[derive(Default)]
struct Hub {
    /// (tag, commitment) -> (envelope, uploaded as "may cross").
    stored: BTreeMap<([u8; 16], [u8; 16]), Held>,
    /// Every prefix list a client ever sent: all a hub learns from fetching.
    seen_fetches: Vec<Vec<[u8; 2]>>,
    /// Every crossing flag uploads came with.
    crossings: Vec<Crossing>,
    /// Deliver "may cross" uploads as from another operator.
    partner_round_trip: bool,
}

impl Hub {
    fn put(&mut self, bytes: &[u8], crossing: Option<Crossing>) -> bool {
        let Ok(b) = Bundle::decode(bytes) else {
            return false;
        };
        // Every hub upload says whether it may cross (the runtime's default is "no").
        let Some(crossing) = crossing else {
            panic!("a hub upload carries its crossing");
        };
        self.crossings.push(crossing);
        self.stored.insert(
            (*b.dst_tag().as_bytes(), *b.purge_commitment()),
            (bytes.to_vec(), crossing == Crossing::Partners),
        );
        true
    }

    fn fetch(&mut self, prefixes: &[[u8; 2]]) -> Vec<(Vec<u8>, HubOrigin)> {
        self.seen_fetches.push(prefixes.to_vec());
        let round_trip = self.partner_round_trip;
        self.stored
            .iter()
            .filter(|((tag, _), _)| prefixes.iter().any(|p| tag[..2] == p[..]))
            .map(|(_, (b, cross))| {
                let origin = if round_trip && *cross {
                    HubOrigin::OtherOperator
                } else {
                    HubOrigin::OwnOperator
                };
                (b.clone(), origin)
            })
            .collect()
    }

    fn purge(&mut self, tokens: &[[u8; 16]]) {
        for t in tokens {
            let c = *commit(&PurgeToken::from_bytes(*t)).as_bytes();
            self.stored.retain(|(_, commitment), _| *commitment != c);
        }
    }
}

/// Runs one device against the hub until quiet: uploads are answered (as `put_ok`), fetches
/// served batch by batch, purges applied. Returns every output.
fn sync(dev: &mut Device, hub: &mut Hub, connect: bool) -> Vec<Output> {
    let first = if connect {
        dev.handle(Input::HubConnected)
    } else {
        dev.tick()
    };
    drive(dev, hub, first)
}

/// Serves `pending` (outputs of any input) against the hub until quiet, as the runtime does.
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
                    let ok = hub.put(bytes, *crossing);
                    next.extend(dev.handle(Input::TransmitResult {
                        op: *op,
                        result: if ok {
                            Ok(())
                        } else {
                            Err(mem_core::ClaError::Failed)
                        },
                    }));
                }
                Output::HubFetch { prefixes } => {
                    let bundles = hub.fetch(prefixes);
                    for chunk in bundles.chunks(16) {
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

fn hub_on(dev: &mut Device) {
    dev.handle(Input::SetChannelEnabled {
        cla: ClaId::Hub,
        enabled: true,
    });
}

fn uploads(out: &[Output]) -> usize {
    out.iter()
        .filter(|o| {
            matches!(
                o,
                Output::Transmit {
                    cla: ClaId::Hub,
                    ..
                }
            )
        })
        .count()
}

#[test]
fn the_hub_is_off_until_turned_on() {
    let mut a = Device::new(101);
    assert!(!a.agent.channel_enabled(ClaId::Hub));
    assert_eq!(a.handle(Input::HubConnected), Vec::new());
}

/// Two phones that never meet and never share a network exchange a message and its
/// ack through the hub, which ends up holding nothing.
#[test]
fn a_message_and_its_ack_travel_through_the_hub() {
    let (mut a, mut b) = (Device::new(102), Device::new(103));
    let (b_id, a_id) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    hub_on(&mut a);
    hub_on(&mut b);
    let mut hub = Hub::default();

    let msg = a.send(b_id, "via the hub").unwrap();
    let out = sync(&mut a, &mut hub, true);
    assert_eq!(uploads(&out), 1);
    let sent = &a.agent.messages(b_id)[0];
    assert_eq!(sent.status, DeliveryStatus::Sent);
    assert_eq!(sent.channels[0].cla, ClaId::Hub);
    assert_eq!(sent.channels[0].grade, PrivacyGrade::P2);

    // B collects it; provenance says hub, P2.
    let out = sync(&mut b, &mut hub, true);
    assert!(out.iter().any(|o| matches!(o, Output::Deliver { .. })));
    let got = &b.agent.messages(a_id)[0];
    assert_eq!(got.text, "via the hub");
    assert_eq!(got.channels[0].cla, ClaId::Hub);
    assert_eq!(got.channels[0].grade, PrivacyGrade::P2);
    // B's purge token deleted A's bundle from the hub; fetching alone never did.
    assert!(
        hub.stored.is_empty(),
        "the recipient purges what it collected"
    );

    // B's standalone ack goes by hub after the ack delay; A collects it.
    b.advance(3_000);
    sync(&mut b, &mut hub, false);
    sync(&mut a, &mut hub, true);
    assert_eq!(
        a.agent.messages(b_id)[0].status,
        DeliveryStatus::Delivered,
        "{msg:?}"
    );
    // Both sides released their tokens: nothing is left on the hub.
    sync(&mut a, &mut hub, false);
    assert!(hub.stored.is_empty(), "{} left", hub.stored.len());
}

/// The hub learns 2-byte prefixes padded with decoys, never tags.
#[test]
fn fetching_reveals_only_padded_prefixes() {
    let (mut a, mut b) = (Device::new(104), Device::new(105));
    pair(&mut a, &mut b, ClaId::Qr).unwrap();
    hub_on(&mut b);
    let mut hub = Hub::default();
    sync(&mut b, &mut hub, true);
    assert_ne!(hub.seen_fetches.len(), 0);
    for f in &hub.seen_fetches {
        assert!(f.len() >= 8 && f.len() <= 64 && f.len().is_power_of_two());
        assert!(f.windows(2).all(|w| w[0] < w[1]), "sorted, no duplicates");
    }
}

/// Fetching never deletes: a stranger who learned a tag (couriers see tags in encounters) can
/// read ciphertext sizes but cannot take anyone's mail away.
#[test]
fn fetching_never_deletes() {
    let (mut a, mut b) = (Device::new(106), Device::new(107));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    hub_on(&mut a);
    let mut hub = Hub::default();
    a.send(b_id, "keep me").unwrap();
    sync(&mut a, &mut hub, true);
    let all: Vec<[u8; 2]> = (0..64u8).map(|i| [i, 0]).collect();
    for _ in 0..3 {
        hub.fetch(&all);
        let every: Vec<[u8; 2]> = hub.stored.keys().map(|(t, _)| [t[0], t[1]]).collect();
        hub.fetch(&every);
    }
    assert_eq!(hub.stored.len(), 1);
}

/// A contact whose floor is stricter than the hub's P2 gets nothing by hub without consent,
/// and an uploaded bundle is not uploaded again on reconnect.
#[test]
fn uploads_respect_the_floor_and_happen_once() {
    let (mut a, mut b) = (Device::new(108), Device::new(109));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    hub_on(&mut a);
    let mut hub = Hub::default();
    a.handle(Input::SetFloor {
        contact: b_id,
        floor: PrivacyGrade::P1,
    });
    a.send(b_id, "not by hub").unwrap();
    assert_eq!(uploads(&sync(&mut a, &mut hub, true)), 0);
    assert!(hub.stored.is_empty());

    a.handle(Input::SetFloor {
        contact: b_id,
        floor: PrivacyGrade::P3,
    });
    assert_eq!(uploads(&sync(&mut a, &mut hub, false)), 1);
    a.handle(Input::HubDisconnected);
    assert_eq!(uploads(&sync(&mut a, &mut hub, true)), 0);
    a.restart().unwrap();
    assert_eq!(
        uploads(&sync(&mut a, &mut hub, true)),
        0,
        "remembered across a restart"
    );
}

/// Push wake-ups: the endpoint is registered with exactly the live set's padded
/// prefixes (never a full tag), renewed with every fetch plan, and withdrawn when it changes.
#[test]
fn wake_registration_uses_the_padded_live_prefixes() {
    let (mut a, mut b) = (Device::new(110), Device::new(111));
    pair(&mut a, &mut b, ClaId::Qr).unwrap();
    hub_on(&mut b);
    let bad = b.handle(Input::SetPushEndpoint(Some(
        "http://ntfy.example/up".into(),
    )));
    assert!(bad.contains(&Output::Rejected(
        mem_core::model::RejectReason::InvalidPushEndpoint
    )));
    b.handle(Input::SetPushEndpoint(Some(
        "https://ntfy.example/upA".into(),
    )));
    let mut hub = Hub::default();
    let out = sync(&mut b, &mut hub, true);
    let regs: Vec<&Vec<[u8; 2]>> = out
        .iter()
        .filter_map(|o| match o {
            Output::HubWakeRegister { endpoint, prefixes }
                if endpoint == "https://ntfy.example/upA" =>
            {
                Some(prefixes)
            }
            _ => None,
        })
        .collect();
    assert_eq!(regs.len(), 1);
    assert_eq!(
        Some(regs[0]),
        hub.seen_fetches.last(),
        "same set as the live fetch"
    );
    assert!(regs[0].len() >= 8 && regs[0].len().is_power_of_two());

    // A new endpoint withdraws the old one and registers itself.
    let out = b.handle(Input::SetPushEndpoint(Some(
        "https://ntfy.example/upB".into(),
    )));
    assert!(out.contains(&Output::HubWakeUnregister {
        endpoint: "https://ntfy.example/upA".into()
    }));
    let out = sync_outputs(&mut b, &mut hub, out);
    assert!(out.iter().any(|o| matches!(
        o,
        Output::HubWakeRegister { endpoint, .. } if endpoint == "https://ntfy.example/upB"
    )));
    b.restart().unwrap();
    assert_eq!(b.agent.push_endpoint(), Some("https://ntfy.example/upB"));
}

/// Runs outputs a device already produced through the hub, like `sync`.
fn sync_outputs(dev: &mut Device, hub: &mut Hub, first: Vec<Output>) -> Vec<Output> {
    let mut all = Vec::new();
    let mut pending = first;
    for _ in 0..64 {
        if pending.is_empty() {
            break;
        }
        let mut next = Vec::new();
        for o in &pending {
            if let Output::HubFetch { prefixes } = o {
                for chunk in hub.fetch(prefixes).chunks(16) {
                    next.extend(dev.handle(Input::HubBundles {
                        bundles: chunk.to_vec(),
                    }));
                }
                next.extend(dev.handle(Input::HubFetchDone));
            }
        }
        all.append(&mut pending);
        pending = next;
    }
    all
}

/// A contact added while connected is fetched for (and woken for) at the next tick, not an hour
/// later.
#[test]
fn a_new_contact_is_fetched_for_at_once() {
    let (mut a, mut b) = (Device::new(112), Device::new(113));
    hub_on(&mut b);
    b.handle(Input::SetPushEndpoint(Some(
        "https://ntfy.example/up".into(),
    )));
    let mut hub = Hub::default();
    let out = sync(&mut b, &mut hub, true);
    assert!(
        !out.iter()
            .any(|o| matches!(o, Output::HubWakeRegister { .. })),
        "no contacts yet"
    );
    pair(&mut a, &mut b, ClaId::Qr).unwrap();
    b.advance(1_000);
    let out = sync(&mut b, &mut hub, false);
    assert!(out.iter().any(|o| matches!(o, Output::HubFetch { .. })));
    assert!(
        out.iter()
            .any(|o| matches!(o, Output::HubWakeRegister { .. }))
    );
}

/// Online: a 10-member group formed and used only through the hub. The members
/// never meet each other; each message reaches all nine others and is confirmed by all.
#[test]
fn a_ten_member_group_works_through_the_hub() {
    use mem_core::model::GroupStatus;
    let mut a = Device::new(120);
    let mut others: Vec<Device> = (121..130).map(Device::new).collect();
    let ids = others
        .iter_mut()
        .map(|o| pair(&mut a, o, ClaId::Qr).unwrap().0)
        .collect();
    hub_on(&mut a);
    for o in &mut others {
        hub_on(o);
    }
    let mut hub = Hub::default();
    a.handle(Input::CreateGroup {
        name: "Ten".into(),
        members: ids,
    });
    let group = a.agent.groups()[0].id;
    sync(&mut a, &mut hub, true);
    for o in &mut others {
        sync(o, &mut hub, true);
        assert_eq!(o.agent.group(group).unwrap().status, GroupStatus::Invited);
        let out = o.handle(Input::AcceptGroupInvitation(group));
        drive(o, &mut hub, out);
    }
    // The admin collects the nine joins and sends the state listing everyone.
    sync(&mut a, &mut hub, true);
    for o in &mut others {
        sync(o, &mut hub, true);
        let g = o.agent.group(group).unwrap();
        assert_eq!(g.status, GroupStatus::Member);
        assert_eq!(g.members.len(), 10);
    }

    // A member (not the admin) writes; everyone else reads it from the hub.
    let out = others[4].handle(Input::SendGroupText {
        group,
        text: "ten of us".into(),
    });
    drive(&mut others[4], &mut hub, out);
    let mut readers: Vec<&mut Device> = others
        .iter_mut()
        .enumerate()
        .filter(|(i, _)| *i != 4)
        .map(|(_, d)| d)
        .collect();
    readers.push(&mut a);
    for r in &mut readers {
        sync(r, &mut hub, true);
        let got = r.agent.group_messages(group);
        assert_eq!(got.iter().filter(|m| m.text == "ten of us").count(), 1);
    }
    // Their acks come back through the hub: delivered to 9 of 9.
    for r in &mut readers {
        r.advance(3_000);
        sync(r, &mut hub, false);
    }
    sync(&mut others[4], &mut hub, true);
    let mine = others[4].agent.group_messages(group);
    let sent = mine.iter().find(|m| m.text == "ten of us").unwrap();
    assert_eq!(sent.recipients, Some((9, 9)));
    assert_eq!(sent.status, DeliveryStatus::Delivered);
}

// --- Partner operators.

fn operator() -> IdentityKey {
    IdentityKey::from_seed(&[7; 32])
}

/// A signed list of one own hub, with or without one partner operator's hub.
fn list(with_partner: bool) -> Vec<u8> {
    let mut l = HubList {
        // A newer list replaces an older one only with a higher serial.
        serial: 1 + u64::from(with_partner),
        issued_unix_ms: NOW,
        expires_unix_ms: NOW + 30 * 86_400_000,
        hubs: vec![HubEntry {
            url: "wss://hub.example/v1".into(),
            key: *IdentityKey::from_seed(&[1; 32]).public().as_bytes(),
            region: None,
        }],
        signature: [0; 64],
        partners: if with_partner {
            vec![PartnerEntry {
                url: "wss://hub.partner.example/v1".into(),
                key: *IdentityKey::from_seed(&[2; 32]).public().as_bytes(),
                operator: "Partner".into(),
            }]
        } else {
            Vec::new()
        },
    };
    l.signature = operator().sign(HUB_LIST_DOMAIN, &[&l.signed_bytes().unwrap()]);
    l.encode().unwrap()
}

fn trust_list(dev: &mut Device, with_partner: bool) {
    dev.handle(Input::SetOperatorKeys(vec![
        *operator().public().as_bytes(),
    ]));
    dev.handle(Input::HubListReceived {
        bytes: list(with_partner),
    });
}

/// Sends while connected and serves the upload; returns the crossing it carried.
fn send_now(
    dev: &mut Device,
    hub: &mut Hub,
    to: mem_core::model::ContactId,
    text: &str,
) -> Crossing {
    let before = hub.crossings.len();
    let out = dev.handle(Input::UserSend {
        contact: to,
        text: text.into(),
    });
    drive(dev, hub, out);
    assert_eq!(hub.crossings.len(), before + 1, "one upload for {text:?}");
    *hub.crossings.last().unwrap()
}

/// The grade the sender recorded for its last hub upload.
fn last_sent_grade(dev: &Device, contact: mem_core::model::ContactId) -> PrivacyGrade {
    let m = dev.agent.messages(contact);
    let used = &m.last().unwrap().channels;
    assert_eq!(used.last().unwrap().cla, ClaId::Hub);
    used.last().unwrap().grade
}

/// An upload may cross only with partners listed, the switch off and P3 within the floor; a
/// crossing send is recorded as P3, one that stays with the operator as P2.
#[test]
fn an_upload_crosses_only_when_partners_switch_and_floor_allow_it() {
    let (mut a, mut b) = (Device::new(120), Device::new(121));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    hub_on(&mut a);
    let mut hub = Hub::default();

    // No list (today): never crosses.
    a.send(b_id, "no list").unwrap();
    sync(&mut a, &mut hub, true);
    assert_eq!(hub.crossings, vec![Crossing::OwnOperator]);
    assert_eq!(last_sent_grade(&a, b_id), PrivacyGrade::P2);

    // A list without partners: still not.
    trust_list(&mut a, false);
    assert_eq!(
        send_now(&mut a, &mut hub, b_id, "no partners"),
        Crossing::OwnOperator
    );

    // Partners listed and the contact's floor P3 (the default): it may cross, recorded P3.
    trust_list(&mut a, true);
    assert_eq!(a.agent.hub_partners().len(), 1);
    assert_eq!(
        send_now(&mut a, &mut hub, b_id, "may cross"),
        Crossing::Partners
    );
    assert_eq!(last_sent_grade(&a, b_id), PrivacyGrade::P3);

    // "Only my operator's hubs": stays, P2.
    a.handle(Input::SetOwnOperatorOnly { enabled: true });
    assert!(a.agent.own_operator_only());
    assert_eq!(
        send_now(&mut a, &mut hub, b_id, "own only"),
        Crossing::OwnOperator
    );
    assert_eq!(last_sent_grade(&a, b_id), PrivacyGrade::P2);

    // A stricter contact (floor P2) is never crossed without asking.
    a.handle(Input::SetOwnOperatorOnly { enabled: false });
    a.handle(Input::SetFloor {
        contact: b_id,
        floor: PrivacyGrade::P2,
    });
    assert_eq!(
        send_now(&mut a, &mut hub, b_id, "strict contact"),
        Crossing::OwnOperator
    );
    assert_eq!(last_sent_grade(&a, b_id), PrivacyGrade::P2);
}

/// A bundle the hub marks as from another operator is recorded as P3 on arrival; its
/// sender recorded P3 too.
#[test]
fn a_bundle_from_another_operator_arrives_as_p3() {
    let (mut a, mut b) = (Device::new(122), Device::new(123));
    let (b_id, a_id) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    hub_on(&mut a);
    hub_on(&mut b);
    trust_list(&mut a, true);
    let mut hub = Hub {
        partner_round_trip: true,
        ..Hub::default()
    };
    a.send(b_id, "crossed").unwrap();
    sync(&mut a, &mut hub, true);
    assert_eq!(last_sent_grade(&a, b_id), PrivacyGrade::P3);
    let out = sync(&mut b, &mut hub, true);
    assert!(out.iter().any(|o| matches!(o, Output::Deliver { .. })));
    let got = &b.agent.messages(a_id)[0];
    assert_eq!(got.text, "crossed");
    assert_eq!(got.channels[0].cla, ClaId::Hub);
    assert_eq!(got.channels[0].grade, PrivacyGrade::P3);
}

/// The switch survives a restart (it is a core setting in the snapshot).
#[test]
fn own_operator_only_is_persisted() {
    let mut a = Device::new(124);
    a.handle(Input::SetOwnOperatorOnly { enabled: true });
    a.restart().unwrap();
    assert!(a.agent.own_operator_only());
}
