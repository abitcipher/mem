//! The email bridge, sans-IO.
//!
//! The user's own mail account carries sealed bundles: P4, so every send asks unless a standing
//! rule for that contact allows email. The runtime's `cla-email` sends what the core
//! emits as [`Output::EmailSend`] and feeds MEM mails back as `Input::Received { cla: Email }`.
//! Contacts' addresses are typed by the user and stay local.

use alloc::collections::BTreeSet;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

use mem_cla::ClaId;
use zeroize::Zeroizing;

use super::{Agent, Ask, Output, StorageOp};
use crate::model::{ContactId, MsgId, RejectReason};

/// Longest email address (RFC 5321).
const MAX_ADDRESS_LEN: usize = 254;
/// Longest host name.
const MAX_HOST_LEN: usize = 253;
/// Longest user name or password we accept.
const MAX_SECRET_LEN: usize = 512;

/// The user's mail account. The password is an app password; it never leaves
/// the core except to the runtime's mail client, and is never printed.
#[derive(Clone, PartialEq, Eq)]
pub struct EmailAccount {
    /// The account's address (sender of MEM mails).
    pub address: String,
    /// IMAP server.
    pub imap_host: String,
    /// IMAP port (993 for TLS).
    pub imap_port: u16,
    /// SMTP server.
    pub smtp_host: String,
    /// SMTP port (465 implicit TLS, 587 STARTTLS).
    pub smtp_port: u16,
    /// Login name (often the address).
    pub username: String,
    /// App password.
    pub password: Zeroizing<String>,
}

impl fmt::Debug for EmailAccount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EmailAccount")
            .field("imap_port", &self.imap_port)
            .field("smtp_port", &self.smtp_port)
            .finish_non_exhaustive()
    }
}

/// A plausible address: one `@`, something on both sides, no spaces, bounded.
pub(crate) fn valid_address(a: &str) -> bool {
    a.len() <= MAX_ADDRESS_LEN
        && !a.chars().any(|c| c.is_whitespace() || c.is_control())
        && a.split_once('@')
            .is_some_and(|(l, d)| !l.is_empty() && !d.is_empty() && !d.contains('@'))
}

fn valid_host(h: &str) -> bool {
    !h.is_empty()
        && h.len() <= MAX_HOST_LEN
        && h.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '[' | ']'))
}

impl EmailAccount {
    pub(crate) fn valid(&self) -> bool {
        valid_address(&self.address)
            && valid_host(&self.imap_host)
            && valid_host(&self.smtp_host)
            && self.imap_port != 0
            && self.smtp_port != 0
            && !self.username.is_empty()
            && self.username.len() <= MAX_SECRET_LEN
            && !self.password.is_empty()
            && self.password.len() <= MAX_SECRET_LEN
    }
}

/// Volatile and persisted email state.
#[derive(Default)]
pub(crate) struct EmailState {
    /// The runtime's mail client is connected (IMAP logged in).
    pub(crate) online: bool,
    /// Messages whose current bundle went out by email (not repeated; persisted).
    pub(crate) sent: BTreeSet<MsgId>,
}

impl Agent {
    /// Dispatches the email inputs.
    pub(super) fn email_input(&mut self, input: crate::Input) -> Vec<Output> {
        use crate::Input;
        match input {
            Input::SetEmailAccount(account) => match account {
                Some(a) if !a.valid() => {
                    alloc::vec![Output::Rejected(RejectReason::InvalidEmailAccount)]
                }
                a => {
                    self.email_account = a;
                    self.email.online = false;
                    alloc::vec![Output::Persist(StorageOp::SaveSnapshot)]
                }
            },
            Input::SetContactEmail { contact, address } => {
                let address = address
                    .map(|a| String::from(a.trim()))
                    .filter(|a| !a.is_empty());
                if address.as_deref().is_some_and(|a| !valid_address(a)) {
                    return alloc::vec![Output::Rejected(RejectReason::InvalidEmailAddress)];
                }
                match self.contacts.get_mut(&contact) {
                    Some(c) if !c.group_only => {
                        c.email = address;
                        alloc::vec![Output::Persist(StorageOp::SaveSnapshot)]
                    }
                    _ => alloc::vec![Output::Rejected(RejectReason::UnknownContact)],
                }
            }
            Input::EmailConnected => {
                if self.disabled.contains(&ClaId::Email) || self.email_account.is_none() {
                    return Vec::new();
                }
                self.email.online = true;
                let mut out = alloc::vec![Output::ReachabilityChanged];
                out.extend(self.flush_email());
                out
            }
            Input::EmailDisconnected => {
                if core::mem::replace(&mut self.email.online, false) {
                    alloc::vec![Output::ReachabilityChanged]
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        }
    }

    /// The address a send to `contact` goes to, if email can reach them at all.
    pub(super) fn email_to(&self, contact: ContactId) -> Option<String> {
        self.email_account.as_ref()?;
        self.contacts.get(&contact)?.email.clone()
    }

    /// Sends by email whatever the gate allows without asking (a standing rule, since email is
    /// below the default floor) and has not gone by email yet. Called on ticks and new messages.
    pub(super) fn flush_email(&mut self) -> Vec<Output> {
        if !self.email.online || self.disabled.contains(&ClaId::Email) {
            return Vec::new();
        }
        let due: Vec<MsgId> = self
            .outbox
            .iter()
            .filter(|(id, item)| {
                !self.email.sent.contains(id)
                    && self.email_to(item.contact).is_some()
                    && !self.ops.values().any(|op| op.is_upload(**id, ClaId::Email))
            })
            .map(|(id, _)| *id)
            .collect();
        let mut out = Vec::new();
        for message in due {
            if let Ok(auth) = self.authorize(message, ClaId::Email, Ask::Never) {
                out.extend(self.transmit(auth, None));
            }
        }
        out
    }

    /// The account, without its password, for settings screens.
    #[must_use]
    pub fn email_account(&self) -> Option<EmailAccount> {
        self.email_account.clone()
    }

    /// Whether the mail client is connected now.
    #[must_use]
    pub const fn email_online(&self) -> bool {
        self.email.online
    }
}

#[cfg(test)]
mod tests {
    use super::valid_address;

    #[test]
    fn addresses() {
        assert!(valid_address("ana@example.org"));
        assert!(valid_address("a.b+mem@mail.example"));
        assert!(!valid_address("ana@"));
        assert!(!valid_address("@example.org"));
        assert!(!valid_address("ana example@x.org"));
        assert!(!valid_address("a@b@c"));
    }
}
