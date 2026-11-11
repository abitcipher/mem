//! Email bridge client.
//!
//! - **Sending:** one SMTP submission per bundle. The mail has subject `MEM`, a short plain-text
//!   body, the header `X-MEM: 1`, and the sealed bundle as the attachment `message.mem`. The
//!   result is reported as [`EmailEvent::SendResult`].
//! - **Receiving:** an IMAP session polls the inbox every 60 s, and at once on
//!   [`EmailClient::check_now`], for unseen mails with `X-MEM: 1`. It checks each size before
//!   downloading, parses only the `message.mem` attachment, reports the bytes as
//!   [`EmailEvent::Bundle`], then moves the mail to a `MEM` folder (never deleted).
//! - **TLS:** always, through rustls and the Mozilla roots. IMAP uses implicit TLS. SMTP uses
//!   implicit TLS on port 465 and STARTTLS (required) otherwise. Plaintext only to this machine
//!   (`localhost`, `127.0.0.1`), for tests and development.
//! - The app password stays in a `Zeroizing<String>` and is never logged.

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use lettre::message::header::{ContentType, HeaderName, HeaderValue};
use lettre::message::{Attachment, Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use zeroize::Zeroizing;

/// How often the inbox is checked while connected.
const POLL_EVERY: Duration = Duration::from_secs(60);
/// First retry after a failure; doubles up to [`MAX_BACKOFF`].
const MIN_BACKOFF: Duration = Duration::from_secs(5);
const MAX_BACKOFF: Duration = Duration::from_secs(300);
/// Largest mail downloaded: a 64 KiB bundle, base64 (+33 %), headers and body.
const MAX_MAIL_LEN: u32 = 128 * 1024;
/// Largest bundle accepted from an attachment.
const MAX_BUNDLE_LEN: usize = 65_536;
/// The folder imported mails move to.
pub const MEM_FOLDER: &str = "MEM";
/// Marks MEM mails.
pub const MEM_HEADER: &str = "X-MEM";
const BODY: &str = "This is an encrypted MEM message. Open it with MEM.\r\n";

/// The account.
#[derive(Clone)]
pub struct EmailConfig {
    /// Sender address.
    pub address: String,
    /// IMAP host.
    pub imap_host: String,
    /// IMAP port.
    pub imap_port: u16,
    /// SMTP host.
    pub smtp_host: String,
    /// SMTP port.
    pub smtp_port: u16,
    /// Login.
    pub username: String,
    /// App password.
    pub password: Zeroizing<String>,
}

impl std::fmt::Debug for EmailConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EmailConfig")
    }
}

/// What the mail client reports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmailEvent {
    /// Logged in to IMAP.
    Connected,
    /// The IMAP session ended; retrying.
    Disconnected,
    /// A bundle from a MEM mail (untrusted).
    Bundle(Vec<u8>),
    /// A send finished.
    SendResult {
        /// The id given to [`EmailClient::send`].
        id: u64,
        /// Submitted to the SMTP server.
        ok: bool,
    },
}

enum Command {
    Send {
        id: u64,
        to: String,
        bundle: Vec<u8>,
    },
    CheckNow,
    Shutdown,
}

/// Handle to the mail client. Cheap to clone.
#[derive(Clone, Debug)]
pub struct EmailClient {
    tx: mpsc::UnboundedSender<Command>,
}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Command")
    }
}

/// Plaintext is allowed only to this machine.
fn loopback(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1")
}

impl EmailClient {
    /// Starts the client in the current Tokio runtime.
    ///
    /// # Panics
    ///
    /// Outside a Tokio runtime.
    #[must_use]
    pub fn start(config: EmailConfig) -> (Self, mpsc::UnboundedReceiver<EmailEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let (events, events_rx) = mpsc::unbounded_channel();
        let (check_tx, check_rx) = mpsc::unbounded_channel();
        tokio::spawn(commands(config.clone(), rx, events.clone(), check_tx));
        tokio::spawn(receive(config, events, check_rx));
        (Self { tx }, events_rx)
    }

    /// Sends a bundle to `to`; an [`EmailEvent::SendResult`] with `id` follows.
    pub fn send(&self, id: u64, to: String, bundle: Vec<u8>) {
        let _ = self.tx.send(Command::Send { id, to, bundle });
    }

    /// Checks the inbox now (e.g. after a push wake-up).
    pub fn check_now(&self) {
        let _ = self.tx.send(Command::CheckNow);
    }

    /// Stops the client.
    pub fn shutdown(&self) {
        let _ = self.tx.send(Command::Shutdown);
    }
}

async fn commands(
    config: EmailConfig,
    mut rx: mpsc::UnboundedReceiver<Command>,
    events: mpsc::UnboundedSender<EmailEvent>,
    check: mpsc::UnboundedSender<bool>,
) {
    while let Some(cmd) = rx.recv().await {
        match cmd {
            Command::Send { id, to, bundle } => {
                let ok = submit(&config, &to, bundle).await;
                if !ok {
                    tracing::debug!("mail submission failed");
                }
                let _ = events.send(EmailEvent::SendResult { id, ok });
            }
            Command::CheckNow => {
                let _ = check.send(true);
            }
            Command::Shutdown => {
                let _ = check.send(false);
                return;
            }
        }
    }
    let _ = check.send(false);
}

/// Builds the MEM mail for `bundle`.
fn mail(from: &str, to: &str, bundle: Vec<u8>) -> Option<Message> {
    let from: Mailbox = from.parse().ok()?;
    let to: Mailbox = to.parse().ok()?;
    Message::builder()
        .from(from)
        .to(to)
        .subject("MEM")
        .raw_header(HeaderValue::new(
            HeaderName::new_from_ascii_str(MEM_HEADER),
            "1".to_owned(),
        ))
        .multipart(
            MultiPart::mixed()
                .singlepart(SinglePart::plain(BODY.to_owned()))
                .singlepart(
                    Attachment::new("message.mem".to_owned())
                        .body(bundle, ContentType::parse("application/octet-stream").ok()?),
                ),
        )
        .ok()
}

async fn submit(config: &EmailConfig, to: &str, bundle: Vec<u8>) -> bool {
    let Some(message) = mail(&config.address, to, bundle) else {
        return false;
    };
    let creds = Credentials::new(config.username.clone(), config.password.to_string());
    let transport = if loopback(&config.smtp_host) {
        AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(config.smtp_host.as_str())
            .port(config.smtp_port)
            .build()
    } else if config.smtp_port == 465 {
        match AsyncSmtpTransport::<Tokio1Executor>::relay(&config.smtp_host) {
            Ok(b) => b.port(465).credentials(creds).build(),
            Err(_) => return false,
        }
    } else {
        match AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.smtp_host) {
            Ok(b) => b.port(config.smtp_port).credentials(creds).build(),
            Err(_) => return false,
        }
    };
    transport.send(message).await.is_ok()
}

/// The receiving loop: connect, poll, reconnect with backoff, until shut down.
async fn receive(
    config: EmailConfig,
    events: mpsc::UnboundedSender<EmailEvent>,
    mut check: mpsc::UnboundedReceiver<bool>,
) {
    let mut backoff = MIN_BACKOFF;
    loop {
        let tcp = TcpStream::connect((config.imap_host.as_str(), config.imap_port)).await;
        let result = match tcp {
            Ok(tcp) if loopback(&config.imap_host) => {
                session(tcp, &config, &events, &mut check).await
            }
            Ok(tcp) => match tls(&config.imap_host, tcp).await {
                Some(t) => session(t, &config, &events, &mut check).await,
                None => Ok(()),
            },
            Err(_) => Ok(()),
        };
        match result {
            Err(Stop) => return,
            Ok(()) => {
                let _ = events.send(EmailEvent::Disconnected);
            }
        }
        tokio::select! {
            () = tokio::time::sleep(backoff) => {}
            more = check.recv() => if more != Some(true) { return },
        }
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
}

async fn tls(host: &str, tcp: TcpStream) -> Option<tokio_rustls::client::TlsStream<TcpStream>> {
    let roots = tokio_rustls::rustls::RootCertStore::from_iter(
        webpki_roots::TLS_SERVER_ROOTS.iter().cloned(),
    );
    let config = tokio_rustls::rustls::ClientConfig::builder_with_provider(Arc::new(
        tokio_rustls::rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .ok()?
    .with_root_certificates(roots)
    .with_no_client_auth();
    let name = tokio_rustls::rustls::pki_types::ServerName::try_from(host.to_owned()).ok()?;
    tokio_rustls::TlsConnector::from(Arc::new(config))
        .connect(name, tcp)
        .await
        .ok()
}

/// The client asked to stop.
struct Stop;

async fn session<S>(
    stream: S,
    config: &EmailConfig,
    events: &mpsc::UnboundedSender<EmailEvent>,
    check: &mut mpsc::UnboundedReceiver<bool>,
) -> Result<(), Stop>
where
    S: AsyncRead + AsyncWrite + Unpin + Send + std::fmt::Debug,
{
    let mut client = async_imap::Client::new(stream);
    if !matches!(client.read_response().await, Ok(Some(_))) {
        return Ok(());
    }
    let Ok(mut imap) = client
        .login(config.username.as_str(), config.password.as_str())
        .await
    else {
        tracing::debug!("mail login failed");
        return Ok(());
    };
    // Exists already on later runs: the error is expected then.
    let _ = imap.create(MEM_FOLDER).await;
    if events.send(EmailEvent::Connected).is_err() {
        return Err(Stop);
    }
    loop {
        if imap.select("INBOX").await.is_err() || !import(&mut imap, events).await {
            return Ok(());
        }
        tokio::select! {
            () = tokio::time::sleep(POLL_EVERY) => {}
            more = check.recv() => if more != Some(true) {
                let _ = imap.logout().await;
                return Err(Stop);
            },
        }
        if imap.noop().await.is_err() {
            return Ok(());
        }
    }
}

/// Imports every unseen MEM mail in the selected inbox and moves it to [`MEM_FOLDER`].
/// `false` if the session failed.
async fn import<S>(
    imap: &mut async_imap::Session<S>,
    events: &mpsc::UnboundedSender<EmailEvent>,
) -> bool
where
    S: AsyncRead + AsyncWrite + Unpin + Send + std::fmt::Debug,
{
    let Ok(uids) = imap
        .uid_search(format!("UNSEEN HEADER {MEM_HEADER} 1"))
        .await
    else {
        return false;
    };
    for uid in uids {
        let set = uid.to_string();
        // Size first: never download more than a MEM mail can be.
        let size = {
            let Ok(mut s) = imap.uid_fetch(&set, "RFC822.SIZE").await else {
                return false;
            };
            let mut size = None;
            while let Some(Ok(f)) = s.next().await {
                size = f.size;
            }
            size
        };
        if size.is_some_and(|n| n <= MAX_MAIL_LEN) {
            let raw = {
                let Ok(mut s) = imap.uid_fetch(&set, "BODY.PEEK[]").await else {
                    return false;
                };
                let mut raw = None;
                while let Some(Ok(f)) = s.next().await {
                    if let Some(b) = f.body() {
                        raw = Some(b.to_vec());
                    }
                }
                raw
            };
            if let Some(bundle) = raw.as_deref().and_then(bundle_of)
                && events.send(EmailEvent::Bundle(bundle)).is_err()
            {
                return false;
            }
        }
        // Imported, oversized or broken: out of the inbox, never deleted.
        if imap.uid_mv(&set, MEM_FOLDER).await.is_err() {
            let copied = imap.uid_copy(&set, MEM_FOLDER).await.is_ok();
            if copied {
                if let Ok(mut s) = imap.uid_store(&set, "+FLAGS (\\Deleted)").await {
                    while s.next().await.is_some() {}
                }
                if let Ok(s) = imap.expunge().await {
                    let mut s = Box::pin(s);
                    while s.next().await.is_some() {}
                }
            }
        }
    }
    true
}

/// The bundle in a MEM mail: the `message.mem` attachment, within the size limit.
#[must_use]
pub fn bundle_of(raw: &[u8]) -> Option<Vec<u8>> {
    use mail_parser::MimeHeaders;
    let message = mail_parser::MessageParser::default().parse(raw)?;
    message.header_raw(MEM_HEADER)?;
    message
        .attachments()
        .find(|a| a.attachment_name() == Some("message.mem"))
        .map(|a| a.contents().to_vec())
        .filter(|b| !b.is_empty() && b.len() <= MAX_BUNDLE_LEN)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, reason = "test code")]

    use super::*;

    #[test]
    fn a_mem_mail_roundtrips_its_attachment() {
        let bundle = vec![0xA5; 3000];
        let m = mail("ana@example.org", "bo@example.net", bundle.clone()).unwrap();
        let raw = m.formatted();
        assert_eq!(bundle_of(&raw), Some(bundle));
        let text = String::from_utf8_lossy(&raw);
        assert!(text.contains("X-MEM: 1"));
        assert!(text.contains("Subject: MEM"));
    }

    #[test]
    fn other_mail_is_ignored() {
        let raw = b"From: a@b.c\r\nTo: d@e.f\r\nSubject: hi\r\n\r\nhello\r\n";
        assert_eq!(bundle_of(raw), None);
        // An X-MEM mail without the attachment.
        let raw = b"From: a@b.c\r\nTo: d@e.f\r\nX-MEM: 1\r\nSubject: MEM\r\n\r\nhello\r\n";
        assert_eq!(bundle_of(raw), None);
    }
}
