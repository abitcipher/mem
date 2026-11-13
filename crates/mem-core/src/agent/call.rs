//! Calls: the core carries call setup ([`CallSignal`]) end to end and decides which media paths
//! a call may use. The media itself (WebRTC) runs in the app.
//!
//! Setup messages live [`CALL_LIFETIME_S`], go as one copy to the contact's main device over an
//! automatic channel within the floor, are never handed to couriers and are not acknowledged:
//! a late offer must not ring a phone. Before a description or candidate leaves, every ICE
//! candidate whose path the contact's floor does not allow is removed, so the app cannot
//! connect a call below the floor without consent.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::string::String;
use alloc::vec::Vec;

use mem_cla::ClaId;
use mem_proto::call::{CONTENT_TYPE_CALL, CallSignal};
use rand_core::CryptoRng;

use super::group::Hidden;
use super::{Agent, Output};
use crate::model::{ChannelUse, ContactId, RejectReason};

/// Lifetime of a call setup message.
pub const CALL_LIFETIME_S: u32 = 60;
/// Most offers taken from one contact per minute; more are dropped (a ringing flood).
pub const MAX_OFFERS_PER_MINUTE: usize = 5;

/// The media paths a call with a contact may use, from its floor and standing rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallPolicy {
    /// Face to face over Wi-Fi Direct (P0).
    pub wifi_direct: bool,
    /// Directly over the local network (P1): the contact learns our local address.
    pub local_network: bool,
    /// Through our own STUN/TURN servers next to the hub (P2).
    pub own_servers: bool,
}

/// Where a candidate would take the media.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Path {
    WifiDirect,
    LocalNetwork,
    Servers,
}

/// The path of one ICE candidate line (`candidate:… <address> <port> typ <type> …`, with or
/// without an `a=` prefix); `None` if it cannot be read, which drops it.
fn candidate_path(line: &str) -> Option<Path> {
    let line = line.trim().strip_prefix("a=").unwrap_or(line.trim());
    let fields: Vec<&str> = line
        .strip_prefix("candidate:")?
        .split_whitespace()
        .collect();
    let address = *fields.get(4)?;
    let typ = fields
        .iter()
        .position(|f| *f == "typ")
        .and_then(|i| fields.get(i + 1))?;
    match *typ {
        // Android's Wi-Fi Direct groups use 192.168.49.0/24.
        "host" if address.starts_with("192.168.49.") => Some(Path::WifiDirect),
        "host" => Some(Path::LocalNetwork),
        // Reflexive addresses come from a STUN server and relayed ones from TURN; both are ours
        // in this version (third-party servers need consent and are not offered).
        "srflx" | "prflx" | "relay" => Some(Path::Servers),
        _ => None,
    }
}

impl CallPolicy {
    fn allows(self, line: &str) -> bool {
        match candidate_path(line) {
            Some(Path::WifiDirect) => self.wifi_direct,
            Some(Path::LocalNetwork) => self.local_network,
            Some(Path::Servers) => self.own_servers,
            None => false,
        }
    }

    /// `signal` without the candidates this policy does not allow: candidate lines inside a
    /// description, and candidate messages (`None` when none is left).
    fn filter(self, signal: CallSignal) -> Option<CallSignal> {
        let keep_sdp = |sdp: String| -> String {
            sdp.split_inclusive('\n')
                .filter(|l| !l.starts_with("a=candidate:") || self.allows(l))
                .collect()
        };
        match signal {
            CallSignal::Offer { call_id, sdp } => Some(CallSignal::Offer {
                call_id,
                sdp: keep_sdp(sdp),
            }),
            CallSignal::Answer { call_id, sdp } => Some(CallSignal::Answer {
                call_id,
                sdp: keep_sdp(sdp),
            }),
            CallSignal::Candidates {
                call_id,
                candidates,
            } => {
                let candidates: Vec<String> =
                    candidates.into_iter().filter(|c| self.allows(c)).collect();
                (!candidates.is_empty()).then_some(CallSignal::Candidates {
                    call_id,
                    candidates,
                })
            }
            end @ CallSignal::End { .. } => Some(end),
        }
    }
}

/// Recent offers per contact, for [`MAX_OFFERS_PER_MINUTE`] (volatile).
#[derive(Default)]
pub(crate) struct OfferLimits(BTreeMap<ContactId, VecDeque<u64>>);

impl Agent {
    /// The media paths a call with `contact` may use; `None` for anyone who is not an
    /// approved contact.
    #[must_use]
    pub fn call_policy(&self, contact: ContactId) -> Option<CallPolicy> {
        let c = self.contacts.get(&contact)?;
        if c.group_only || c.requested || self.is_self(contact) {
            return None;
        }
        let ok = |cla: ClaId| {
            let grade = cla.grade();
            grade.within(c.floor) || self.matching_rule(contact, cla, grade).is_some()
        };
        Some(CallPolicy {
            wifi_direct: ok(ClaId::WifiDirect),
            local_network: ok(ClaId::Iroh),
            own_servers: ok(ClaId::Hub),
        })
    }

    /// Sends a call setup message to `contact` now, or refuses with
    /// [`RejectReason::Unreachable`] when no automatic channel within the floor is open.
    pub(super) fn send_call(
        &mut self,
        contact: ContactId,
        signal: CallSignal,
        rng: &mut impl CryptoRng,
    ) -> Vec<Output> {
        let Some(policy) = self.call_policy(contact) else {
            return alloc::vec![Output::Rejected(RejectReason::NotAContact)];
        };
        let Some(signal) = policy.filter(signal) else {
            // Every candidate was below the floor: nothing to send.
            return Vec::new();
        };
        let Ok(content) = signal.encode() else {
            return alloc::vec![Output::Rejected(RejectReason::Malformed)];
        };
        let how = Hidden {
            lifetime_s: CALL_LIFETIME_S,
            ack: false,
            copies: 1,
            device_copies: false,
        };
        let Some(id) = self.queue_hidden_as(contact, CONTENT_TYPE_CALL, content, how, rng) else {
            return alloc::vec![Output::Rejected(RejectReason::NotAContact)];
        };
        let out = self.flush_message(id);
        let sent = out
            .iter()
            .any(|o| matches!(o, Output::Transmit { message, .. } if *message == id));
        if sent {
            out
        } else {
            // A call cannot wait for a channel: drop it rather than ring later.
            self.outbox.remove(&id);
            alloc::vec![Output::Rejected(RejectReason::Unreachable)]
        }
    }

    /// A call setup message from `contact`, over `cla` at `grade`. Offers beyond
    /// [`MAX_OFFERS_PER_MINUTE`] are dropped.
    pub(super) fn received_call(
        &mut self,
        contact: ContactId,
        signal: CallSignal,
        cla: ClaId,
        grade: mem_cla::PrivacyGrade,
    ) -> Vec<Output> {
        let via = ChannelUse {
            cla,
            grade,
            at_unix_ms: self.now_unix_ms,
        };
        if self.call_policy(contact).is_none() {
            return alloc::vec![Output::Rejected(RejectReason::NotAContact)];
        }
        if matches!(signal, CallSignal::Offer { .. }) {
            let now = self.now_unix_ms;
            let recent = self.offer_limits.0.entry(contact).or_default();
            recent.retain(|t| now.saturating_sub(*t) < 60_000);
            if recent.len() >= MAX_OFFERS_PER_MINUTE {
                return alloc::vec![Output::Rejected(RejectReason::Quota)];
            }
            recent.push_back(now);
        }
        alloc::vec![Output::CallSignal {
            contact,
            signal,
            via,
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAN: &str = "candidate:1 1 udp 2122260223 10.0.0.5 41000 typ host generation 0";
    const DIRECT: &str = "candidate:2 1 udp 2122260223 192.168.49.1 41001 typ host";
    const RELAY: &str = "candidate:3 1 udp 41885439 144.31.216.27 50000 typ relay raddr 0.0.0.0";

    #[test]
    fn candidates_are_read_by_path() {
        assert_eq!(candidate_path(LAN), Some(Path::LocalNetwork));
        assert_eq!(
            candidate_path(&alloc::format!("a={DIRECT}")),
            Some(Path::WifiDirect)
        );
        assert_eq!(candidate_path(RELAY), Some(Path::Servers));
        assert_eq!(candidate_path("candidate:garbage"), None);
    }

    #[test]
    fn the_floor_removes_candidates_before_they_leave() {
        let face_to_face = CallPolicy {
            wifi_direct: true,
            local_network: false,
            own_servers: false,
        };
        let sdp = alloc::format!("v=0\r\na={LAN}\r\na={DIRECT}\r\na={RELAY}\r\nm=audio\r\n");
        let Some(CallSignal::Offer { sdp, .. }) = face_to_face.filter(CallSignal::Offer {
            call_id: [1; 16],
            sdp,
        }) else {
            unreachable!()
        };
        assert!(sdp.contains("192.168.49.1"));
        assert!(!sdp.contains("10.0.0.5") && !sdp.contains("144.31.216.27"));
        assert!(sdp.contains("m=audio"));
        let only_lan = CallSignal::Candidates {
            call_id: [1; 16],
            candidates: alloc::vec![LAN.into()],
        };
        assert_eq!(face_to_face.filter(only_lan), None);
    }

    mod agents {
        use super::*;
        use crate::agent::Input;
        use crate::model::PeerHandle;
        use mem_cla::PrivacyGrade;
        use rand_chacha::ChaCha20Rng;
        use rand_core::SeedableRng;

        const NOW: u64 = 1_792_022_400_000;

        fn agent(seed: u8) -> (Agent, ChaCha20Rng) {
            let mut rng = ChaCha20Rng::from_seed([seed; 32]);
            let mut a = Agent::new(NOW, &mut rng);
            let _ = a.handle(Input::Tick { wall_unix_ms: NOW }, &mut rng);
            (a, rng)
        }

        /// A and B paired and in one local-network encounter (B is handle 1 at A, A is 2 at B).
        fn connected() -> (Agent, ChaCha20Rng, Agent, ChaCha20Rng, ContactId) {
            let (mut a, mut ra) = agent(1);
            let (mut b, mut rb) = agent(2);
            let (card_a, card_b) = (a.own_card().unwrap(), b.own_card().unwrap());
            let _ = a.handle(
                Input::ImportCard {
                    cla: ClaId::Qr,
                    bytes: card_b,
                },
                &mut ra,
            );
            let _ = b.handle(
                Input::ImportCard {
                    cla: ClaId::Qr,
                    bytes: card_a,
                },
                &mut rb,
            );
            let b_id = a.contacts()[0].id;
            let mut queue = alloc::collections::VecDeque::from([
                (
                    true,
                    Input::EncounterStarted {
                        peer: PeerHandle(1),
                        cla: ClaId::Iroh,
                    },
                ),
                (
                    false,
                    Input::EncounterStarted {
                        peer: PeerHandle(2),
                        cla: ClaId::Iroh,
                    },
                ),
            ]);
            while let Some((to_a, input)) = queue.pop_front() {
                let out = if to_a {
                    a.handle(input, &mut ra)
                } else {
                    b.handle(input, &mut rb)
                };
                for o in out {
                    if let Output::Encounter { msg, .. } = o {
                        let peer = PeerHandle(if to_a { 2 } else { 1 });
                        queue.push_back((!to_a, Input::EncounterMessage { peer, msg }));
                    }
                }
            }
            (a, ra, b, rb, b_id)
        }

        /// Sends `signal` from A and hands what A transmits to B.
        fn call(
            a: &mut Agent,
            ra: &mut ChaCha20Rng,
            b: &mut Agent,
            rb: &mut ChaCha20Rng,
            b_id: ContactId,
            signal: CallSignal,
        ) -> Vec<Output> {
            let out = a.handle(
                Input::SendCallSignal {
                    contact: b_id,
                    signal,
                },
                ra,
            );
            let mut got = Vec::new();
            for o in out {
                match o {
                    Output::Transmit { bytes, .. } => got.extend(b.handle(
                        Input::Received {
                            cla: ClaId::Iroh,
                            bytes,
                        },
                        rb,
                    )),
                    r @ Output::Rejected(_) => got.push(r),
                    _ => {}
                }
            }
            got
        }

        fn offer(n: u8) -> CallSignal {
            CallSignal::Offer {
                call_id: [n; 16],
                sdp: alloc::format!("v=0\r\na={LAN}\r\nm=audio\r\n"),
            }
        }

        #[test]
        fn an_offer_reaches_the_contact_and_nobody_waits_for_a_channel() {
            let (mut a, mut ra) = agent(1);
            let (b, _) = agent(2);
            let _ = a.handle(
                Input::ImportCard {
                    cla: ClaId::Qr,
                    bytes: b.own_card().unwrap(),
                },
                &mut ra,
            );
            let b_id = a.contacts()[0].id;
            // No channel open: refused at once, nothing left to ring later.
            let out = a.handle(
                Input::SendCallSignal {
                    contact: b_id,
                    signal: offer(1),
                },
                &mut ra,
            );
            assert_eq!(
                out,
                alloc::vec![Output::Rejected(RejectReason::Unreachable)]
            );
            assert!(a.outbox.is_empty());

            let (mut a, mut ra, mut b, mut rb, b_id) = connected();
            let got = call(&mut a, &mut ra, &mut b, &mut rb, b_id, offer(1));
            let signal = got.iter().find_map(|o| match o {
                Output::CallSignal { signal, via, .. } => Some((signal.clone(), via.cla)),
                _ => None,
            });
            let Some((CallSignal::Offer { sdp, .. }, cla)) = signal else {
                panic!("no offer: {got:?}");
            };
            assert!(sdp.contains("10.0.0.5"));
            assert_eq!(cla, ClaId::Iroh);
        }

        #[test]
        fn the_callees_floor_strips_paths_and_floods_are_cut() {
            let (mut a, mut ra, mut b, mut rb, b_id) = connected();
            // A allows B face to face only: the local-network candidate does not leave.
            let _ = a.handle(
                Input::SetFloor {
                    contact: b_id,
                    floor: PrivacyGrade::P0,
                },
                &mut ra,
            );
            let policy = a.call_policy(b_id).unwrap();
            assert!(policy.wifi_direct && !policy.local_network && !policy.own_servers);
            let _ = a.handle(
                Input::SetFloor {
                    contact: b_id,
                    floor: PrivacyGrade::P2,
                },
                &mut ra,
            );

            for n in 0..MAX_OFFERS_PER_MINUTE {
                let got = call(
                    &mut a,
                    &mut ra,
                    &mut b,
                    &mut rb,
                    b_id,
                    offer(u8::try_from(n).unwrap()),
                );
                assert!(got.iter().any(|o| matches!(o, Output::CallSignal { .. })));
            }
            let got = call(&mut a, &mut ra, &mut b, &mut rb, b_id, offer(99));
            assert!(got.contains(&Output::Rejected(RejectReason::Quota)));
        }
    }
}
