//! Deterministic multi-node DTN simulator.
//!
//! Drives many `mem-core` agents with injected time and seeded randomness through encounters,
//! the way the runtime does (direct sends with proofs, `Forward` read from each node's store),
//! under a random-meeting mobility model and optional spamming couriers. Every routing change
//! must include simulation results.
//!
//! Scope of this model: encounters are complete and lossless (no rate or MTU model yet);
//! mobility is uniform random meetings or communities with a few travellers between them
//! ([`Mobility`]); the adversary floods every node it meets with junk bundles up to the
//! per-encounter allowance.

use std::collections::BTreeMap;

use mem_core::courier::{CourierKey, CourierMeta, StoreWrite, TombstoneRow};
use mem_core::model::{ContactId, DeliveryStatus, MsgId, PeerHandle};
use mem_core::{Agent, ClaId, EncounterMsg, Input, Output, StorageOp};
use mem_proto::encounter::{BundleEntry, ENCOUNTER_VERSION};
use mem_proto::{Bundle, BundleId, BundleKind, BundleParams, RendezvousTag};
use rand_chacha::ChaCha20Rng;
use rand_core::{Rng, SeedableRng};

/// One encounter's peer handle on both sides (encounters run one at a time).
const LINK: PeerHandle = PeerHandle(1);
/// Simulated wall clock start: 2026-10-15T00:00:00Z.
const START_MS: u64 = 1_792_022_400_000;

/// What the runtime would keep in its database.
#[derive(Default)]
struct Store {
    courier: BTreeMap<CourierKey, (CourierMeta, Vec<u8>)>,
    tombstones: BTreeMap<[u8; 16], TombstoneRow>,
    /// Real-message copies deleted to make room for another bundle (not purged).
    honest_evicted: usize,
}

impl Store {
    fn apply(&mut self, outputs: &[Output]) {
        for o in outputs {
            let Output::Persist(StorageOp::Store(writes)) = o else {
                continue;
            };
            for w in writes {
                match w {
                    // An eviction: deletions in the same write set as an admission.
                    StoreWrite::Delete(keys)
                        if writes.iter().any(|w| matches!(w, StoreWrite::Put { .. })) =>
                    {
                        for k in keys {
                            if k.bundle_id[0] != 0xFF && self.courier.contains_key(k) {
                                self.honest_evicted += 1;
                            }
                            self.courier.remove(k);
                        }
                    }
                    StoreWrite::Put { meta, bytes } => {
                        self.courier.insert(meta.key, (meta.clone(), bytes.clone()));
                    }
                    StoreWrite::Delete(keys) => {
                        for k in keys {
                            self.courier.remove(k);
                        }
                    }
                    StoreWrite::SetCopies { key, copies } => {
                        if let Some((m, _)) = self.courier.get_mut(key) {
                            m.copies = *copies;
                        }
                    }
                    StoreWrite::TombstonePut(t) => {
                        self.tombstones.insert(t.commitment, t.clone());
                    }
                    StoreWrite::TombstoneDelete(cs) => {
                        for c in cs {
                            self.tombstones.remove(c);
                        }
                    }
                }
            }
        }
    }
}

/// A simulated honest device.
struct Node {
    agent: Agent,
    rng: ChaCha20Rng,
    store: Store,
}

impl Node {
    fn handle(&mut self, now: u64, input: Input) -> Vec<Output> {
        let mut out = self
            .agent
            .handle(Input::Tick { wall_unix_ms: now }, &mut self.rng);
        out.extend(self.agent.handle(input, &mut self.rng));
        self.store.apply(&out);
        out
    }

    /// The bytes a peer receives for one output, as the runtime would send them.
    fn wire(&self, o: &Output) -> Option<EncounterMsg> {
        let msg = match o {
            Output::Encounter { msg, .. } => msg.clone(),
            Output::Transmit {
                peer: Some(_),
                bytes,
                proof,
                ..
            } => EncounterMsg::Bundles(vec![BundleEntry {
                bytes: bytes.clone(),
                proof: *proof,
            }]),
            Output::Forward {
                key,
                hops,
                copies,
                age_ms,
                ..
            } => {
                let (_, stored) = self.store.courier.get(key)?;
                let bytes = Bundle::relayed(stored, *hops, *copies, *age_ms).ok()?;
                EncounterMsg::Bundles(vec![BundleEntry::bare(bytes)])
            }
            _ => return None,
        };
        // Through the real codec, as on the wire.
        EncounterMsg::decode(&msg.encode().ok()?).ok()
    }
}

/// Who meets whom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mobility {
    /// Every two devices are equally likely to meet.
    Uniform,
    /// Device `i` belongs to community `i % groups`. A meeting is inside one community with
    /// probability `inside_pct` %; otherwise one of the `travellers` of a community (its
    /// highest-numbered devices) meets a device of another community. Senders and receivers are
    /// in different communities, so every message has to cross over: the case routing is for.
    Communities {
        /// Number of communities (at least 2).
        groups: usize,
        /// Share of meetings inside a community, percent.
        inside_pct: u8,
        /// Devices per community that also meet other communities.
        travellers: usize,
    },
}

/// Simulation settings.
#[derive(Debug, Clone, Copy)]
pub struct Config {
    /// Honest devices.
    pub nodes: usize,
    /// Sender–receiver pairs (contacts that never meet directly).
    pub pairs: usize,
    /// Messages each sender sends at the start.
    pub messages_per_pair: usize,
    /// Simulation steps; each step is one encounter between two random honest devices.
    pub steps: usize,
    /// Simulated minutes between steps.
    pub minutes_per_step: u64,
    /// Spamming devices; each meets a random honest device every `spam_every` steps.
    pub spammers: usize,
    /// Steps between spam encounters.
    pub spam_every: usize,
    /// Courier storage limit per honest device, bytes.
    pub courier_bytes: u64,
    /// Seed.
    pub seed: u64,
    /// Who meets whom.
    pub mobility: Mobility,
    /// Spray copies of each new message (`L`); the product default is 32.
    pub copies: u8,
    /// Junk bundles (16 KiB each) a spammer offers per encounter at most: 300 floods the
    /// per-encounter allowance; 15 (240 KiB) stays under the flood threshold.
    pub spam_bundles: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            nodes: 30,
            pairs: 5,
            messages_per_pair: 2,
            steps: 2_000,
            minutes_per_step: 5,
            spammers: 0,
            spam_every: 5,
            courier_bytes: mem_core::courier::DEFAULT_COURIER_BYTES,
            seed: 1,
            mobility: Mobility::Uniform,
            copies: 32,
            spam_bundles: 300,
        }
    }
}

/// What a run measured.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Report {
    /// Messages sent.
    pub sent: usize,
    /// Messages that reached their receiver.
    pub delivered: usize,
    /// Messages the sender saw confirmed by an ack.
    pub confirmed: usize,
    /// Median steps from send to delivery, for delivered messages.
    pub median_latency_steps: Option<usize>,
    /// Largest courier storage any honest device used at any step, bytes.
    pub max_courier_bytes: u64,
    /// Average courier storage per honest device at the end, bytes.
    pub avg_courier_bytes_end: u64,
    /// Junk bundles the spammers offered.
    pub junk_offered: usize,
    /// Largest number of devices holding a copy of one real message at once.
    pub max_holders: usize,
    /// Junk bytes held by honest devices at the end, in total.
    pub junk_bytes_end: u64,
    /// Real-message copies evicted to make room, over all honest devices.
    pub honest_evicted: usize,
    /// Junk bundles honest devices stored from other honest devices (spread by spraying, not
    /// handed over by a spammer), over the run.
    pub junk_relayed: usize,
}

/// Honest devices with the courier limit set, the sender–receiver pairs, and the messages sent.
type Setup = (
    Vec<Node>,
    Vec<(usize, usize, ContactId, ContactId)>,
    Vec<(usize, usize, ContactId, MsgId)>,
);

fn setup(cfg: &Config, now: u64) -> Setup {
    let mut nodes: Vec<Node> = (0..cfg.nodes)
        .map(|i| {
            let mut seed = [0u8; 32];
            seed[..8].copy_from_slice(&(cfg.seed ^ (i as u64 + 1) << 8).to_le_bytes());
            let mut rng = ChaCha20Rng::from_seed(seed);
            let agent = Agent::new(now, &mut rng);
            Node {
                agent,
                rng,
                store: Store::default(),
            }
        })
        .collect();
    for n in &mut nodes {
        n.agent.set_spray_copies_for_simulation(cfg.copies);
        n.handle(
            now,
            Input::SetCourierLimit {
                bytes: cfg.courier_bytes,
            },
        );
    }

    // Pairs: node 2k sends to node 2k+1; they pair in person once, then never meet again.
    let mut pairs: Vec<(usize, usize, ContactId, ContactId)> = Vec::new();
    for k in 0..cfg.pairs.min(cfg.nodes / 2) {
        let (s, r) = (2 * k, 2 * k + 1);
        let card_r = nodes[r].agent.own_card().unwrap_or_default();
        let card_s = nodes[s].agent.own_card().unwrap_or_default();
        let r_in_s = paired(&nodes[s].handle(now, import(card_r)));
        let s_in_r = paired(&nodes[r].handle(now, import(card_s)));
        if let (Some(a), Some(b)) = (r_in_s, s_in_r) {
            pairs.push((s, r, a, b));
        }
    }
    // (sender, receiver, sender's id at the receiver, message)
    let mut sent: Vec<(usize, usize, ContactId, MsgId)> = Vec::new();
    for &(s, r, r_id, s_id) in &pairs {
        for m in 0..cfg.messages_per_pair {
            let out = nodes[s].handle(
                now,
                Input::UserSend {
                    contact: r_id,
                    text: format!("m{m}"),
                },
            );
            if let Some(id) = out.iter().find_map(|o| match o {
                Output::Queued { message, .. } => Some(*message),
                _ => None,
            }) {
                sent.push((s, r, s_id, id));
            }
        }
    }

    (nodes, pairs, sent)
}

/// Runs one simulation.
///
/// # Panics
///
/// If a node's state cannot be built (a bug in the core, not in the scenario).
#[must_use]
pub fn run(cfg: Config) -> Report {
    let mut world = ChaCha20Rng::seed_from_u64(cfg.seed);
    let mut now = START_MS;
    let (mut nodes, pairs, sent) = setup(&cfg, now);
    let mut report = Report {
        sent: sent.len(),
        ..Report::default()
    };
    let mut delivered_at: BTreeMap<MsgId, usize> = BTreeMap::new();
    let pair_set: Vec<(usize, usize)> = pairs.iter().map(|p| (p.0, p.1)).collect();
    for step in 0..cfg.steps {
        now += cfg.minutes_per_step * 60_000;
        // A meeting between two honest devices that are not a sender–receiver pair.
        let (x, y) = meeting(&cfg, &mut world);
        let pair = pair_set
            .iter()
            .any(|&(s, r)| (s, r) == (x, y) || (s, r) == (y, x));
        if !pair {
            let junk_before: usize = [x, y].iter().map(|&i| junk_count(&nodes[i])).sum();
            let (a, b) = two(&mut nodes, x, y);
            meet(a, b, now);
            let junk_after: usize = [x, y].iter().map(|&i| junk_count(&nodes[i])).sum();
            report.junk_relayed += junk_after.saturating_sub(junk_before);
        }
        if cfg.spammers > 0 && step % cfg.spam_every.max(1) == 0 {
            for _ in 0..cfg.spammers {
                let victim = pick(&mut world, cfg.nodes);
                report.junk_offered += spam(&mut nodes[victim], now, &mut world, cfg.spam_bundles);
            }
        }
        for n in &nodes {
            report.max_courier_bytes = report.max_courier_bytes.max(n.agent.courier_status().0);
        }
        for &(_, r, s_id, id) in &sent {
            if !delivered_at.contains_key(&id)
                && nodes[r].agent.messages(s_id).iter().any(|m| m.id == id)
            {
                delivered_at.insert(id, step);
            }
        }
        // Copies of real bundles held at once (junk ids start with 0xFF).
        let mut holders: BTreeMap<[u8; 16], usize> = BTreeMap::new();
        for n in &nodes {
            for k in n.store.courier.keys().filter(|k| k.bundle_id[0] != 0xFF) {
                *holders.entry(k.bundle_id).or_default() += 1;
            }
        }
        report.max_holders = report
            .max_holders
            .max(holders.values().copied().max().unwrap_or(0));
    }
    report.delivered = delivered_at.len();
    report.confirmed = sent
        .iter()
        .filter(|(s, _, _, id)| {
            nodes[*s].agent.contacts().iter().any(|c| {
                nodes[*s]
                    .agent
                    .messages(c.id)
                    .iter()
                    .any(|m| m.id == *id && m.status == DeliveryStatus::Delivered)
            })
        })
        .count();
    let mut lat: Vec<usize> = delivered_at.values().copied().collect();
    lat.sort_unstable();
    report.median_latency_steps = lat.get(lat.len() / 2).copied();
    report.honest_evicted = nodes.iter().map(|n| n.store.honest_evicted).sum();
    report.junk_bytes_end = nodes
        .iter()
        .flat_map(|n| n.store.courier.iter())
        .filter(|(k, _)| k.bundle_id[0] == 0xFF)
        .map(|(_, (m, _))| u64::from(m.size))
        .sum();
    let total: u64 = nodes.iter().map(|n| n.agent.courier_status().0).sum();
    report.avg_courier_bytes_end = total / cfg.nodes.max(1) as u64;
    report
}

/// Junk bundles a device holds (junk ids start with 0xFF).
fn junk_count(n: &Node) -> usize {
    n.store
        .courier
        .keys()
        .filter(|k| k.bundle_id[0] == 0xFF)
        .count()
}

/// Two different devices that meet, following the mobility model.
fn meeting(cfg: &Config, world: &mut ChaCha20Rng) -> (usize, usize) {
    let n = cfg.nodes;
    let differ = |x: usize, y: usize| if x == y { (x, (y + 1) % n) } else { (x, y) };
    match cfg.mobility {
        Mobility::Uniform => differ(pick(world, n), pick(world, n)),
        Mobility::Communities {
            groups,
            inside_pct,
            travellers,
        } => {
            let groups = groups.clamp(2, n);
            // Members of community `c`: c, c + groups, c + 2·groups, …
            let members = |c: usize| (c..n).step_by(groups).collect::<Vec<_>>();
            let c = pick(world, groups);
            let own = members(c);
            if pick(world, 100) < usize::from(inside_pct) || own.len() < 2 {
                let x = own[pick(world, own.len())];
                let mut y = own[pick(world, own.len())];
                if x == y {
                    y = own[(own.iter().position(|&m| m == x).unwrap_or(0) + 1) % own.len()];
                }
                (x, y)
            } else {
                let t = travellers.clamp(1, own.len());
                let x = own[own.len() - 1 - pick(world, t)];
                let other = (c + 1 + pick(world, groups - 1)) % groups;
                let theirs = members(other);
                (x, theirs[pick(world, theirs.len())])
            }
        }
    }
}

/// A uniform-enough index below `n` (simulation only; modulo bias is irrelevant here).
fn pick(rng: &mut ChaCha20Rng, n: usize) -> usize {
    usize::try_from(rng.next_u64() % (n.max(1) as u64)).unwrap_or(0)
}

fn import(bytes: Vec<u8>) -> Input {
    Input::ImportCard {
        cla: ClaId::Qr,
        bytes,
    }
}

fn paired(out: &[Output]) -> Option<ContactId> {
    out.iter().find_map(|o| match o {
        Output::ContactPaired { contact, .. } => Some(*contact),
        _ => None,
    })
}

fn two(nodes: &mut [Node], x: usize, y: usize) -> (&mut Node, &mut Node) {
    if x < y {
        let (l, r) = nodes.split_at_mut(y);
        (&mut l[x], &mut r[0])
    } else {
        let (l, r) = nodes.split_at_mut(x);
        (&mut r[0], &mut l[y])
    }
}

/// Runs one complete encounter over the local network and parts the devices again.
fn meet(a: &mut Node, b: &mut Node, now: u64) {
    let started = Input::EncounterStarted {
        peer: LINK,
        cla: ClaId::Iroh,
    };
    let mut pa = a.handle(now, started.clone());
    let mut pb = b.handle(now, started);
    for _ in 0..64 {
        if pa.is_empty() && pb.is_empty() {
            break;
        }
        let to_b: Vec<EncounterMsg> = pa.iter().filter_map(|o| a.wire(o)).collect();
        let to_a: Vec<EncounterMsg> = pb.iter().filter_map(|o| b.wire(o)).collect();
        let a_results = results(&pa);
        let b_results = results(&pb);
        pa = Vec::new();
        pb = Vec::new();
        for i in a_results {
            pa.extend(a.handle(now, i));
        }
        for msg in to_a {
            pa.extend(a.handle(now, Input::EncounterMessage { peer: LINK, msg }));
        }
        for i in b_results {
            pb.extend(b.handle(now, i));
        }
        for msg in to_b {
            pb.extend(b.handle(now, Input::EncounterMessage { peer: LINK, msg }));
        }
    }
    a.handle(now, Input::EncounterEnded { peer: LINK });
    b.handle(now, Input::EncounterEnded { peer: LINK });
}

/// Transmit results the runtime reports back after a successful write.
fn results(outputs: &[Output]) -> Vec<Input> {
    outputs
        .iter()
        .filter_map(|o| match o {
            Output::Transmit {
                op, peer: Some(_), ..
            } => Some(Input::TransmitResult {
                op: *op,
                result: Ok(()),
            }),
            _ => None,
        })
        .collect()
}

/// A spammer meets `victim` and offers junk bundles (random tags, maximum copies and hops) until
/// the per-encounter allowance stops it. Returns how many it offered.
fn spam(victim: &mut Node, now: u64, world: &mut ChaCha20Rng, bundles: usize) -> usize {
    let peer = PeerHandle(99);
    victim.handle(
        now,
        Input::EncounterStarted {
            peer,
            cla: ClaId::Iroh,
        },
    );
    victim.handle(
        now,
        Input::EncounterMessage {
            peer,
            msg: EncounterMsg::Hello {
                version: ENCOUNTER_VERSION,
                ephemeral_id: [0xEE; 16],
            },
        },
    );
    let mut offered = 0;
    for _ in 0..bundles {
        let mut id = [0u8; 16];
        world.fill_bytes(&mut id);
        id[0] = 0xFF;
        let mut tag = [0u8; 16];
        world.fill_bytes(&mut tag);
        let mut commitment = [0u8; 16];
        world.fill_bytes(&mut commitment);
        let Ok(junk) = Bundle::new(BundleParams {
            bundle_id: BundleId::from_bytes(id),
            kind: BundleKind::Message,
            dst_tag: RendezvousTag::from_bytes(tag),
            lifetime_s: BundleKind::Message.max_lifetime_s(),
            age_ms: 0,
            hops_remaining: 5,
            copies: 64,
            pow: None,
            payload: vec![0xAB; 16 * 1024],
            purge_commitment: commitment,
        }) else {
            break;
        };
        offered += 1;
        let out = victim.handle(
            now,
            Input::EncounterMessage {
                peer,
                msg: EncounterMsg::Bundles(vec![BundleEntry::bare(junk.encode())]),
            },
        );
        if out.iter().any(|o| {
            matches!(
                o,
                Output::Rejected(mem_core::model::RejectReason::Quota)
                    | Output::EndEncounter { .. }
            )
        }) {
            break;
        }
    }
    victim.handle(now, Input::EncounterEnded { peer });
    offered
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Messages reach receivers they never meet, through couriers.
    /// Small enough for debug-mode CI; the full-size numbers come from `print_reports`.
    fn small() -> Config {
        Config {
            nodes: 12,
            pairs: 3,
            steps: 400,
            ..Config::default()
        }
    }

    #[test]
    fn delivers_through_couriers_without_direct_meetings() {
        let r = run(small());
        assert!(r.sent > 0);
        assert!(
            r.delivered * 10 >= r.sent * 8,
            "delivery ratio too low: {r:?}"
        );
        assert!(r.confirmed > 0, "acks travel back: {r:?}");
    }

    /// Storage stays bounded under a spam adversary, and honest traffic
    /// still gets through.
    #[test]
    fn storage_stays_bounded_under_spam() {
        let cfg = Config {
            spammers: 2,
            spam_every: 4,
            courier_bytes: 1024 * 1024,
            ..small()
        };
        let r = run(cfg);
        assert!(r.junk_offered > 500, "{r:?}");
        assert!(r.max_courier_bytes <= cfg.courier_bytes, "{r:?}");
        assert!(r.delivered > 0, "honest traffic still delivered: {r:?}");
    }

    /// Prints the courier-exchange numbers (run with `--ignored --nocapture`).
    #[test]
    #[ignore = "report, not a check"]
    #[allow(clippy::print_stdout, reason = "this test exists to print the report")]
    fn print_reports() {
        for (name, cfg) in [
            ("default", Config::default()),
            (
                "spam",
                Config {
                    spammers: 3,
                    spam_every: 2,
                    courier_bytes: 2 * 1024 * 1024,
                    ..Config::default()
                },
            ),
            (
                "sparse (60 nodes)",
                Config {
                    nodes: 60,
                    ..Config::default()
                },
            ),
        ] {
            println!("{name}: {:?}", run(cfg));
        }
        for seed in 2..=6 {
            let r = run(Config {
                spammers: 3,
                spam_every: 2,
                courier_bytes: 2 * 1024 * 1024,
                seed,
                ..Config::default()
            });
            println!(
                "spam seed {seed}: delivered {}/{}, confirmed {}",
                r.delivered, r.sent, r.confirmed
            );
        }
    }

    /// Community mobility: delivery and latency over seeds (run with `--ignored --nocapture`,
    /// release mode). Routing changes are compared against these numbers.
    #[test]
    #[ignore = "report, not a check"]
    #[allow(clippy::print_stdout, reason = "this test exists to print the report")]
    fn print_community_reports() {
        for (name, steps) in [
            ("communities, 3000 steps", 3_000),
            ("communities, 6000 steps", 6_000),
        ] {
            let (mut sent, mut delivered, mut confirmed, mut lat, mut holders) = (0, 0, 0, 0, 0);
            for seed in 1..=5 {
                let r = run(community(steps, seed));
                sent += r.sent;
                delivered += r.delivered;
                confirmed += r.confirmed;
                lat += r.median_latency_steps.unwrap_or(steps);
                holders += r.max_holders;
                println!("{name} seed {seed}: {r:?}");
            }
            println!(
                "{name}: delivered {delivered}/{sent}, confirmed {confirmed}, mean median latency {} steps, mean max holders {}",
                lat / 5,
                holders / 5
            );
        }
    }

    pub(crate) fn community(steps: usize, seed: u64) -> Config {
        Config {
            nodes: 60,
            pairs: 6,
            steps,
            seed,
            mobility: Mobility::Communities {
                groups: 4,
                inside_pct: 90,
                travellers: 2,
            },
            ..Config::default()
        }
    }

    /// The copy budget under community mobility.
    #[test]
    #[ignore = "report, not a check"]
    #[allow(clippy::print_stdout, reason = "this test exists to print the report")]
    fn print_copy_budgets() {
        for copies in [4, 8, 16, 32] {
            let (mut sent, mut delivered, mut confirmed, mut lat, mut bytes, mut holders) =
                (0, 0, 0, 0, 0, 0);
            for seed in 1..=5 {
                let r = run(Config {
                    copies,
                    ..community(3_000, seed)
                });
                sent += r.sent;
                delivered += r.delivered;
                confirmed += r.confirmed;
                lat += r.median_latency_steps.unwrap_or(3_000);
                bytes += r.max_courier_bytes;
                holders += r.max_holders;
            }
            println!(
                "L={copies}: delivered {delivered}/{sent}, confirmed {confirmed}, mean median latency {} steps, mean max holders {}, mean peak courier bytes {}",
                lat / 5,
                holders / 5,
                bytes / 5
            );
        }
    }

    /// Larger copy budgets under the spam adversary and uniform mobility.
    #[test]
    #[ignore = "report, not a check"]
    #[allow(clippy::print_stdout, reason = "this test exists to print the report")]
    fn print_copy_budgets_spam() {
        for copies in [8, 16, 32] {
            for (name, base) in [
                ("uniform 30", Config::default()),
                (
                    "spam",
                    Config {
                        spammers: 3,
                        spam_every: 2,
                        courier_bytes: 2 * 1024 * 1024,
                        ..Config::default()
                    },
                ),
                (
                    "communities + spam",
                    Config {
                        spammers: 3,
                        spam_every: 2,
                        courier_bytes: 2 * 1024 * 1024,
                        ..community(3_000, 1)
                    },
                ),
            ] {
                let (mut sent, mut delivered, mut confirmed, mut peak) = (0, 0, 0, 0);
                for seed in 1..=4 {
                    let r = run(Config {
                        seed,
                        copies,
                        ..base
                    });
                    sent += r.sent;
                    delivered += r.delivered;
                    confirmed += r.confirmed;
                    peak = peak.max(r.max_courier_bytes);
                }
                println!(
                    "L={copies} {name}: delivered {delivered}/{sent}, confirmed {confirmed}, peak courier bytes {peak}"
                );
            }
        }
    }

    /// Where junk goes under heavy spam: evicted honest copies, junk relayed between honest
    /// devices, junk held at the end.
    #[test]
    #[ignore = "report, not a check"]
    #[allow(clippy::print_stdout, reason = "this test exists to print the report")]
    fn print_junk_spread() {
        let r = run(Config {
            copies: 32,
            spammers: 3,
            spam_every: 2,
            courier_bytes: 2 * 1024 * 1024,
            ..community(3_000, 1)
        });
        println!(
            "delivered {}/{}, honest copies evicted {}, junk offered {}, junk relayed between honest devices {}, junk bytes held at end {} (of {} total capacity)",
            r.delivered,
            r.sent,
            r.honest_evicted,
            r.junk_offered,
            r.junk_relayed,
            r.junk_bytes_end,
            60 * 2 * 1024 * 1024
        );
    }

    /// Communities under heavy spam, by copy budget.
    #[test]
    #[ignore = "report, not a check"]
    #[allow(clippy::print_stdout, reason = "this test exists to print the report")]
    fn print_community_spam() {
        for copies in [8, 32] {
            let (mut sent, mut delivered, mut confirmed, mut evicted) = (0, 0, 0, 0);
            for seed in 1..=4 {
                let r = run(Config {
                    copies,
                    spammers: 3,
                    spam_every: 2,
                    courier_bytes: 2 * 1024 * 1024,
                    ..community(3_000, seed)
                });
                sent += r.sent;
                delivered += r.delivered;
                confirmed += r.confirmed;
                evicted += r.honest_evicted;
            }
            println!(
                "L={copies} communities + spam: delivered {delivered}/{sent}, confirmed {confirmed}, honest copies evicted {evicted}"
            );
        }
    }

    /// A spammer that stays under the flood threshold: many encounters of 240 KiB.
    #[test]
    #[ignore = "report, not a check"]
    #[allow(clippy::print_stdout, reason = "this test exists to print the report")]
    fn print_adaptive_spam() {
        for (name, spammers, every) in [
            ("3 spammers every 2 steps", 3, 2),
            ("6 spammers every step", 6, 1),
        ] {
            let (mut sent, mut delivered, mut confirmed, mut evicted) = (0, 0, 0, 0);
            for seed in 1..=4 {
                let r = run(Config {
                    spammers,
                    spam_every: every,
                    spam_bundles: 15,
                    courier_bytes: 2 * 1024 * 1024,
                    ..community(3_000, seed)
                });
                sent += r.sent;
                delivered += r.delivered;
                confirmed += r.confirmed;
                evicted += r.honest_evicted;
            }
            println!(
                "adaptive, {name}: delivered {delivered}/{sent}, confirmed {confirmed}, honest copies evicted {evicted}"
            );
        }
    }

    /// Runs are reproducible from the seed.
    #[test]
    fn deterministic() {
        let cfg = Config {
            steps: 150,
            ..small()
        };
        assert_eq!(run(cfg), run(cfg));
    }
}
