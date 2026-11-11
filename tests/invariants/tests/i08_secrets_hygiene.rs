//! Secrets hygiene: Debug output of secret-bearing types never contains key material.

#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::doc_markdown,
    clippy::format_collect,
    reason = "test code"
)]

use mem_core::ClaId;
use mem_crypto::{ContactSecret, IdentityKey, MailboxKey, PairingNonce};
use mem_invariants::{Device, pair};
use rand_chacha::ChaCha20Rng;
use rand_core::SeedableRng;

#[test]
fn debug_output_redacts_secrets() {
    let mut rng = ChaCha20Rng::from_seed([1; 32]);
    let id = IdentityKey::generate(&mut rng);
    let seed_hex: String = id.to_seed().iter().map(|b| format!("{b:02x}")).collect();
    let dbg = format!("{id:?}");
    assert!(!dbg.contains(&seed_hex[..8]));
    assert_eq!(
        format!("{:?}", MailboxKey::generate(&mut rng)),
        "MailboxKey(<secret>)"
    );
    let peer = IdentityKey::generate(&mut rng);
    let n = PairingNonce::generate(&mut rng);
    let s = ContactSecret::derive(&id, &n, &peer.public(), &n).unwrap();
    assert_eq!(format!("{s:?}"), "ContactSecret(<secret>)");

    let (mut a, mut b) = (Device::new(1), Device::new(2));
    pair(&mut a, &mut b, ClaId::Qr).unwrap();
    let agent_dbg = format!("{:?}", a.agent);
    assert!(agent_dbg.starts_with("Agent {"));
    assert!(!agent_dbg.contains("secret"));
}

/// The one exception to secrets hygiene: recovery phrase words reach FFI only through
/// `backup_phrase_show` (out) and `restore_with_phrase` (in). This checks the FFI source: any other
/// function touching the phrase fails the build's tests.
#[test]
fn only_two_ffi_functions_carry_the_recovery_phrase() {
    let src = include_str!("../../../crates/mem-ffi/src/api/mod.rs");
    let fn_of = |needle: &str| -> Vec<String> {
        let mut current = String::new();
        let mut found = Vec::new();
        for line in src.lines() {
            let t = line.trim_start();
            if let Some(rest) = t.strip_prefix("pub fn ").or_else(|| t.strip_prefix("fn ")) {
                current = rest.split('(').next().unwrap_or_default().to_string();
            }
            if line.contains(needle) && !t.starts_with("//") {
                found.push(current.clone());
            }
        }
        found
    };
    assert_eq!(fn_of("recovery_phrase()"), ["backup_phrase_show"]);
    assert_eq!(fn_of(".restore_backup("), ["restore_with_phrase"]);
    assert_eq!(fn_of("summary.corrected"), ["restore_with_phrase"]);
}
