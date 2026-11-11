//! Against a real mail server on this machine (`GreenMail`: SMTP 3025, IMAP 3143, users
//! ana@example.org and bo@example.net with password `pw`). Set `MEM_TEST_MAIL=127.0.0.1` to run;
//! skipped otherwise.

#![allow(clippy::unwrap_used, clippy::panic, reason = "test code")]

use std::time::Duration;

use cla_email::{EmailClient, EmailConfig, EmailEvent};
use zeroize::Zeroizing;

fn config(user: &str, host: &str) -> EmailConfig {
    EmailConfig {
        address: user.into(),
        imap_host: host.into(),
        imap_port: 3143,
        smtp_host: host.into(),
        smtp_port: 3025,
        username: user.into(),
        password: Zeroizing::new("pw".into()),
    }
}

async fn next(rx: &mut tokio::sync::mpsc::UnboundedReceiver<EmailEvent>) -> EmailEvent {
    tokio::time::timeout(Duration::from_secs(20), rx.recv())
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn a_bundle_travels_by_mail_and_lands_in_the_mem_folder() {
    let Ok(host) = std::env::var("MEM_TEST_MAIL") else {
        return;
    };
    let (ana, mut ea) = EmailClient::start(config("ana@example.org", &host));
    let (bo, mut eb) = EmailClient::start(config("bo@example.net", &host));
    assert_eq!(next(&mut ea).await, EmailEvent::Connected);
    assert_eq!(next(&mut eb).await, EmailEvent::Connected);

    let bundle = vec![0x5A; 5000];
    ana.send(1, "bo@example.net".into(), bundle.clone());
    assert_eq!(
        next(&mut ea).await,
        EmailEvent::SendResult { id: 1, ok: true }
    );
    bo.check_now();
    assert_eq!(next(&mut eb).await, EmailEvent::Bundle(bundle));

    // Moved out of the inbox: a second check imports nothing more.
    bo.check_now();
    assert!(
        tokio::time::timeout(Duration::from_secs(3), eb.recv())
            .await
            .is_err(),
        "imported twice"
    );
    ana.shutdown();
    bo.shutdown();
}
