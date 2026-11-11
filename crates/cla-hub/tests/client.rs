//! The client against the in-process hub: greeting, upload, prefix fetch with live push, purge,
//! and reconnecting after the hub goes away.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::time::Duration;

use cla_hub::test_hub::TestHub;
use cla_hub::{HubClient, HubEvent};
use mem_crypto::purge::commit;
use mem_crypto::{PurgeToken, SenderPurgeKey};
use mem_proto::hub::{Crossing, HubOrigin};
use mem_proto::{Bundle, BundleId, BundleKind, BundleParams, RendezvousTag};
use tokio::sync::mpsc::UnboundedReceiver;

fn bundle(tag: [u8; 16], token: &PurgeToken) -> Vec<u8> {
    Bundle::new(BundleParams {
        bundle_id: BundleId::from_bytes([9; 16]),
        kind: BundleKind::Message,
        dst_tag: RendezvousTag::from_bytes(tag),
        lifetime_s: 3600,
        age_ms: 0,
        hops_remaining: 0,
        copies: 1,
        pow: None,
        payload: vec![0xAB; 200],
        purge_commitment: *commit(token).as_bytes(),
    })
    .unwrap()
    .encode()
}

async fn next(rx: &mut UnboundedReceiver<HubEvent>) -> HubEvent {
    tokio::time::timeout(Duration::from_secs(10), rx.recv())
        .await
        .unwrap()
        .unwrap()
}

fn prefixes(real: [u8; 2]) -> Vec<[u8; 2]> {
    let mut p: Vec<[u8; 2]> = (0..7u8).map(|i| [0xF0, i]).collect();
    p.push(real);
    p.sort_unstable();
    p
}

#[tokio::test]
async fn upload_fetch_live_and_purge() {
    let hub = TestHub::start().await;
    let (a, mut ea) = HubClient::start(hub.url());
    let (b, mut eb) = HubClient::start(hub.url());
    assert!(matches!(next(&mut ea).await, HubEvent::Connected(_)));
    assert!(matches!(next(&mut eb).await, HubEvent::Connected(_)));

    let key = SenderPurgeKey::from_bytes(&[5; 32]);
    let token = key.derive_token(&[9; 16]);
    let tag = [0x12, 0x34, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14];

    // B fetches first (nothing yet), then A uploads: B gets it live.
    b.fetch(prefixes([0x12, 0x34]));
    assert_eq!(next(&mut eb).await, HubEvent::FetchDone);
    a.put(1, bundle(tag, &token), Crossing::OwnOperator);
    assert_eq!(next(&mut ea).await, HubEvent::PutResult { id: 1, ok: true });
    assert!(matches!(
        next(&mut eb).await,
        HubEvent::Bundles(v) if v.len() == 1 && v[0].1 == HubOrigin::OwnOperator
    ));

    // Fetching again finds it still there (fetching never deletes) ...
    b.fetch(prefixes([0x12, 0x34]));
    assert!(matches!(next(&mut eb).await, HubEvent::Bundles(v) if v.len() == 1));
    assert_eq!(next(&mut eb).await, HubEvent::FetchDone);
    // ... the purge token removes it.
    b.purge(vec![*token.as_bytes()]);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(hub.is_empty());

    // An unrelated prefix sees nothing.
    a.fetch(prefixes([0x99, 0x99]));
    assert_eq!(next(&mut ea).await, HubEvent::FetchDone);
    a.shutdown();
    b.shutdown();
}

#[tokio::test]
async fn uploads_fail_fast_when_the_hub_is_unreachable() {
    // Nothing listens on this port.
    let (c, mut ev) = HubClient::start("ws://127.0.0.1:9/v1".into());
    c.put(42, vec![1, 2, 3], Crossing::OwnOperator);
    assert_eq!(
        next(&mut ev).await,
        HubEvent::PutResult { id: 42, ok: false }
    );
    c.shutdown();
}

/// The client sends the "may cross" flag and reports the hub's foreign mark (the test
/// hub delivers crossable uploads as if a partner operator had carried them).
#[tokio::test]
async fn crossing_flag_and_foreign_mark() {
    let hub = TestHub::start().await;
    let (a, mut ea) = HubClient::start(hub.url());
    assert!(matches!(next(&mut ea).await, HubEvent::Connected(_)));
    let key = SenderPurgeKey::from_bytes(&[6; 32]);
    let tag = [0x56, 0x78, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14];
    a.put(
        1,
        bundle(tag, &key.derive_token(&[1; 16])),
        Crossing::Partners,
    );
    assert_eq!(next(&mut ea).await, HubEvent::PutResult { id: 1, ok: true });
    a.fetch(prefixes([0x56, 0x78]));
    assert!(matches!(
        next(&mut ea).await,
        HubEvent::Bundles(v) if v.len() == 1 && v[0].1 == HubOrigin::OtherOperator
    ));
    assert_eq!(next(&mut ea).await, HubEvent::FetchDone);
    a.shutdown();
}
