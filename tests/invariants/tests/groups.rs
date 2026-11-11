//! Small groups over DTN by pairwise fan-out: the admin introduces members who never
//! met, they talk directly, the envelope stays minimal (no group id in any), contacts stay deny-by-default (a fellow member
//! is not a contact), and membership follows the admin's epochs.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use mem_core::model::{DeliveryStatus, GroupId, GroupStatus, MemberRole, RejectReason};
use mem_core::{ClaId, Input, Output};
use mem_invariants::{Device, LINK_PEER, encounter, pair, pump};

/// One encounter over Wi-Fi, run until quiet and then ended on both sides.
fn meet(x: &mut Device, y: &mut Device) -> (Vec<Output>, Vec<Output>) {
    let out = encounter(x, y, ClaId::Iroh);
    x.handle(Input::EncounterEnded { peer: LINK_PEER });
    y.handle(Input::EncounterEnded { peer: LINK_PEER });
    out
}

fn invitation(out: &[Output]) -> Option<GroupId> {
    out.iter().find_map(|o| match o {
        Output::GroupInvitation { group } => Some(*group),
        _ => None,
    })
}

fn group_messages(out: &[Output]) -> usize {
    out.iter()
        .filter(|o| matches!(o, Output::GroupMessage { .. }))
        .count()
}

fn envelopes(out: &[Output]) -> Vec<Vec<u8>> {
    out.iter()
        .filter_map(|o| match o {
            Output::Transmit { bytes, .. } => Some(bytes.clone()),
            _ => None,
        })
        .collect()
}

/// A with contacts B and C, who are not each other's contacts; a group of the three, joined.
fn three() -> (Device, Device, Device, GroupId) {
    let (mut a, mut b, mut c) = (Device::new(81), Device::new(82), Device::new(83));
    let (b_id, _) = pair(&mut a, &mut b, ClaId::Qr).unwrap();
    let (c_id, _) = pair(&mut a, &mut c, ClaId::Qr).unwrap();
    a.handle(Input::CreateGroup {
        name: "Hikers".into(),
        members: vec![b_id, c_id],
    });
    let group = a.agent.groups()[0].id;

    // B: invitation, then join; A answers with the next epoch.
    let (_, out_b) = meet(&mut a, &mut b);
    assert_eq!(invitation(&out_b), Some(group));
    assert_eq!(b.agent.group(group).unwrap().status, GroupStatus::Invited);
    b.handle(Input::AcceptGroupInvitation(group));
    meet(&mut a, &mut b);
    assert_eq!(b.agent.group(group).unwrap().status, GroupStatus::Member);

    // C the same; A's state for epoch 3 lists all three.
    let (_, out_c) = meet(&mut a, &mut c);
    assert_eq!(invitation(&out_c), Some(group));
    c.handle(Input::AcceptGroupInvitation(group));
    meet(&mut a, &mut c);
    let g = c.agent.group(group).unwrap();
    assert_eq!(g.status, GroupStatus::Member);
    assert_eq!(g.members.len(), 3);

    // B learns about C at its next encounter with A.
    meet(&mut a, &mut b);
    assert_eq!(b.agent.group(group).unwrap().members.len(), 3);
    (a, b, c, group)
}

#[test]
fn members_who_never_met_talk_directly() {
    let (_a, mut b, mut c, group) = three();
    // Group-only: not in each other's contact list.
    assert_eq!(b.agent.contacts().len(), 1);
    assert_eq!(c.agent.contacts().len(), 1);
    let c_in_b = b
        .agent
        .group(group)
        .unwrap()
        .members
        .iter()
        .find(|m| m.role == MemberRole::Member && !m.is_me)
        .unwrap()
        .clone();
    assert!(!c_in_b.is_contact);

    // B writes to the group; C gets it straight from B, though they never paired: both derived
    // the same pair secret from the cards in the admin's state.
    let out = b.handle(Input::SendGroupText {
        group,
        text: "trail at 9?".into(),
    });
    let (_, out_c) = pump(&mut b, &mut c, out, Vec::new());
    assert_eq!(group_messages(&out_c), 0, "nothing goes before they meet");
    let (_, out_c) = meet(&mut b, &mut c);
    assert_eq!(group_messages(&out_c), 1);
    let got = c.agent.group_messages(group);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].text, "trail at 9?");
    assert_eq!(got[0].group, Some(group));
    // Not a 1:1 message.
    assert!(
        c.agent
            .contacts()
            .iter()
            .all(|k| c.agent.messages(k.id).is_empty())
    );

    // A 1:1 message to a group-only member is refused at the sender.
    let b_in_c = c
        .agent
        .group(group)
        .unwrap()
        .members
        .iter()
        .find(|m| m.role == MemberRole::Member && !m.is_me)
        .unwrap()
        .contact;
    let out = c.handle(Input::UserSend {
        contact: b_in_c,
        text: "hi".into(),
    });
    assert!(out.contains(&Output::Rejected(RejectReason::NotAContact)));
}

#[test]
fn our_copies_show_as_one_message_with_k_of_n() {
    let (mut a, mut b, mut c, group) = three();
    let out = a.handle(Input::SendGroupText {
        group,
        text: "meet at the bridge".into(),
    });
    pump(&mut a, &mut b, out, Vec::new());
    let mine = a.agent.group_messages(group);
    assert_eq!(mine.len(), 1);
    assert_eq!(mine[0].recipients, Some((0, 2)));
    // B and C each confirm their copy.
    for other in [&mut b, &mut c] {
        encounter(&mut a, other, ClaId::Iroh);
        other.advance(3_000);
        let out = other.tick();
        pump(&mut a, other, Vec::new(), out);
        a.handle(Input::EncounterEnded { peer: LINK_PEER });
        other.handle(Input::EncounterEnded { peer: LINK_PEER });
    }
    let mine = a.agent.group_messages(group);
    assert_eq!(mine[0].recipients, Some((2, 2)));
    assert_eq!(mine[0].status, DeliveryStatus::Delivered);
}

/// Couriers and the hub see per-member bundles to unrelated tags; the group id never
/// appears outside the end-to-end payload.
#[test]
fn no_group_id_in_any_envelope() {
    let (mut a, mut b, _c, group) = three();
    let out = a.handle(Input::SendGroupText {
        group,
        text: "x".into(),
    });
    let (out_a, _) = pump(&mut a, &mut b, out, Vec::new());
    let (more, _) = meet(&mut a, &mut b);
    for env in envelopes(&out_a).iter().chain(&envelopes(&more)) {
        assert!(
            !env.windows(16).any(|w| w == group.as_bytes()),
            "group id visible to carriers"
        );
    }
}

#[test]
fn removal_follows_the_admins_epochs() {
    let (mut a, mut b, mut c, group) = three();
    let c_id = a
        .agent
        .group(group)
        .unwrap()
        .members
        .iter()
        .find(|m| m.fingerprint == c.agent.fingerprint())
        .map(|m| m.contact)
        .unwrap();
    // Only the admin may change membership.
    let a_in_b = b.agent.contacts()[0].id;
    assert!(
        b.handle(Input::RemoveFromGroup {
            group,
            member: a_in_b
        })
        .contains(&Output::Rejected(RejectReason::NotGroupAdmin))
    );
    a.handle(Input::RemoveFromGroup {
        group,
        member: c_id,
    });
    meet(&mut a, &mut c);
    assert_eq!(c.agent.group(group).unwrap().status, GroupStatus::Removed);
    // C keeps no group-only contacts, and cannot write to the group any more.
    assert_eq!(c.agent.group(group).unwrap().members.len(), 2);
    assert!(
        c.handle(Input::SendGroupText {
            group,
            text: "still here?".into()
        })
        .contains(&Output::Rejected(RejectReason::UnknownGroup))
    );
    meet(&mut a, &mut b);
    let g = b.agent.group(group).unwrap();
    assert_eq!(g.members.len(), 2);
    assert_eq!(g.epoch, 4);
}

#[test]
fn leaving_and_declining() {
    let (mut a, mut b, mut c, group) = three();
    b.handle(Input::LeaveGroup(group));
    meet(&mut a, &mut b);
    assert_eq!(b.agent.group(group).unwrap().status, GroupStatus::Left);
    assert_eq!(a.agent.group(group).unwrap().members.len(), 2);
    meet(&mut a, &mut c);
    assert_eq!(c.agent.group(group).unwrap().members.len(), 2);

    // A declined invitation disappears on both sides.
    let (mut admin, mut guest) = (Device::new(91), Device::new(92));
    let (guest_id, _) = pair(&mut admin, &mut guest, ClaId::Qr).unwrap();
    admin.handle(Input::CreateGroup {
        name: "Book club".into(),
        members: vec![guest_id],
    });
    let g2 = admin.agent.groups()[0].id;
    meet(&mut admin, &mut guest);
    guest.handle(Input::DeclineGroupInvitation(g2));
    meet(&mut admin, &mut guest);
    assert!(guest.agent.group(g2).is_none());
    assert!(
        admin
            .agent
            .group(g2)
            .unwrap()
            .members
            .iter()
            .all(|m| m.role != MemberRole::Invited)
    );
}

/// Invitations go only to full contacts, and groups survive a restart.
#[test]
fn invitations_need_contacts_and_groups_persist() {
    let (mut a, mut b, mut c, group) = three();
    // B cannot invite C: C is only a fellow member to B.
    let c_in_b = b
        .agent
        .group(group)
        .unwrap()
        .members
        .iter()
        .find(|m| m.role == MemberRole::Member && !m.is_me)
        .unwrap()
        .contact;
    assert!(
        b.handle(Input::CreateGroup {
            name: "side chat".into(),
            members: vec![c_in_b],
        })
        .contains(&Output::Rejected(RejectReason::UnknownContact))
    );
    b.restart().unwrap();
    let g = b.agent.group(group).unwrap();
    assert_eq!(g.members.len(), 3);
    assert_eq!(g.status, GroupStatus::Member);
    // The derived pair secret survived too: C still reaches B.
    let out = c.handle(Input::SendGroupText {
        group,
        text: "after restart".into(),
    });
    pump(&mut c, &mut b, out, Vec::new());
    let (_, out_b) = meet(&mut c, &mut b);
    assert_eq!(group_messages(&out_b), 1);
    let _ = &mut a;

    // Adding a fellow member as a contact is local; deny-by-default still needs the other side.
    let out = b.handle(Input::AddMemberAsContact(c_in_b));
    assert!(
        out.iter()
            .any(|o| matches!(o, Output::ContactPaired { .. }))
    );
    assert_eq!(b.agent.contacts().len(), 2);
}

/// Removing a contact (the user's choice) drops it from contacts and its 1:1 conversation, but a
/// shared group keeps working: it stays a fellow member (group-only).
#[test]
fn removing_a_contact_keeps_shared_groups_working() {
    let (mut a, mut b, _c, group) = three();
    let b_id = a
        .agent
        .contacts()
        .into_iter()
        .find(|c| c.fingerprint == b.agent.fingerprint())
        .unwrap()
        .id;
    a.send(b_id, "1:1 before removal").unwrap();
    a.handle(Input::RemoveContact(b_id));
    assert!(
        a.agent.contacts().iter().all(|c| c.id != b_id),
        "no longer a contact"
    );
    assert!(a.agent.messages(b_id).is_empty(), "1:1 conversation gone");
    // The group still reaches B.
    a.handle(Input::SendGroupText {
        group,
        text: "still in the group".into(),
    });
    let (_, out_b) = meet(&mut a, &mut b);
    assert_eq!(group_messages(&out_b), 1);
}

/// Offline: a 10-member group with no hub and no network. The writer meets only two
/// members; the others get their copies from members who carry them (couriers).
#[test]
fn a_ten_member_group_works_offline_through_couriers() {
    let mut a = Device::new(90);
    let mut m: Vec<Device> = (91..100).map(Device::new).collect();
    let ids = m
        .iter_mut()
        .map(|o| pair(&mut a, o, ClaId::Qr).unwrap().0)
        .collect();
    a.handle(Input::CreateGroup {
        name: "Ten".into(),
        members: ids,
    });
    let group = a.agent.groups()[0].id;
    for o in &mut m {
        meet(&mut a, o);
        o.handle(Input::AcceptGroupInvitation(group));
        meet(&mut a, o);
    }
    for o in &mut m {
        meet(&mut a, o);
        assert_eq!(o.agent.group(group).unwrap().members.len(), 10);
    }

    // m[0] writes, then meets m[1] and m[2] only; they pass copies on in later encounters.
    m[0].handle(Input::SendGroupText {
        group,
        text: "offline ten".into(),
    });
    let schedule: [(usize, usize); 9] = [
        (0, 1),
        (0, 2),
        (1, 3),
        (1, 4),
        (2, 5),
        (2, 6),
        (3, 7),
        (4, 8),
        (5, 9), // 9 = the admin
    ];
    for (x, y) in schedule {
        let (dx, dy) = two(&mut a, &mut m, x, y);
        meet(dx, dy);
    }
    for (i, d) in m.iter().enumerate().skip(1) {
        let n = d
            .agent
            .group_messages(group)
            .iter()
            .filter(|g| g.text == "offline ten")
            .count();
        assert_eq!(n, 1, "member {i}");
    }
    let n = a
        .agent
        .group_messages(group)
        .iter()
        .filter(|g| g.text == "offline ten")
        .count();
    assert_eq!(n, 1, "admin");
}

/// Two distinct devices out of the admin (index 9) and the members.
fn two<'d>(
    a: &'d mut Device,
    m: &'d mut [Device],
    x: usize,
    y: usize,
) -> (&'d mut Device, &'d mut Device) {
    assert!(x < y);
    if y == 9 {
        return (&mut m[x], a);
    }
    let (lo, hi) = m.split_at_mut(y);
    (&mut lo[x], &mut hi[0])
}
