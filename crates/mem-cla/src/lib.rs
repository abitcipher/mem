//! Convergence Layer Adapters: channel identity, capabilities, exposure profiles and privacy
//! grades.
//!
//! This crate is `no_std` so the sans-IO core can reason about channels. The async `Cla` trait
//! that drives real radios and sockets lives with the runtime.
#![no_std]
#![forbid(unsafe_code)]

/// Privacy grade of a channel for a given send. Lower is more private.
///
/// Grades order policy decisions; they are not a safety claim. The UI always shows the
/// [`ExposureProfile`] as well.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PrivacyGrade {
    /// Direct physical: only bystanders learn that devices exchanged something.
    P0,
    /// Direct network: local network / ISP see IPs, timing, size.
    P1,
    /// Own relay or hub.
    P2,
    /// Third-party MEM couriers.
    P3,
    /// Foreign platform carrying the end-to-end encrypted payload.
    P4,
    /// Plaintext fallback: content exposed.
    P5,
}

impl PrivacyGrade {
    /// Whether sending at this grade is allowed under `floor` without asking.
    #[must_use]
    pub fn within(self, floor: Self) -> bool {
        self <= floor
    }

    /// Short label, e.g. `"P0"`.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::P0 => "P0",
            Self::P1 => "P1",
            Self::P2 => "P2",
            Self::P3 => "P3",
            Self::P4 => "P4",
            Self::P5 => "P5",
        }
    }
}

/// Default privacy floor for contacts added on a new install: couriers
/// (P3) are allowed without asking; foreign platforms (P4) and plaintext (P5) always ask.
pub const DEFAULT_FLOOR: PrivacyGrade = PrivacyGrade::P3;

/// The earlier default. Installs whose state predates the default-floor setting keep it,
/// so an update never lowers anyone's floor silently.
pub const LEGACY_DEFAULT_FLOOR: PrivacyGrade = PrivacyGrade::P2;

impl PrivacyGrade {
    /// Whether this grade may be a privacy floor. P5 never may: a P5 floor would let plaintext
    /// leave without the per-message consent P5 always needs.
    #[must_use]
    pub const fn allowed_as_floor(self) -> bool {
        !matches!(self, Self::P5)
    }
}

/// How much of one dimension a third party learns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// Nothing.
    None,
    /// Partial or coarse information.
    Limited,
    /// Fully visible.
    Exposed,
}

/// Who observes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Observer {
    /// People physically nearby.
    NearbyPeople,
    /// The local network operator.
    LocalNetwork,
    /// A MEM courier device.
    Courier,
    /// Our MEM hub.
    OurHub,
    /// An Iroh relay operator.
    IrohRelayOperator,
    /// A third-party platform (messenger, email, social network).
    ThirdPartyPlatform,
    /// The mobile carrier.
    MobileCarrier,
    /// A push notification provider.
    PushProvider,
    /// The owner of a contact's device acting as a hub: knows which of their contacts uploaded
    /// and fetched, when, and how much.
    DeviceHubOwner,
}

/// The exposure dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dimension {
    /// Message plaintext.
    Content,
    /// Real-world identity or account of sender or receiver.
    Identity,
    /// That these two parties communicate.
    SocialGraph,
    /// When a message was sent or received.
    Timing,
    /// Approximate message size.
    Size,
    /// IP addresses.
    NetworkAddr,
    /// That two devices were physically near each other.
    Proximity,
    /// That the user runs MEM at all.
    Existence,
}

impl Dimension {
    /// All dimensions in display order.
    pub const ALL: [Self; 8] = [
        Self::Content,
        Self::Identity,
        Self::SocialGraph,
        Self::Timing,
        Self::Size,
        Self::NetworkAddr,
        Self::Proximity,
        Self::Existence,
    ];
}

/// What third parties can learn from one send over a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExposureProfile {
    /// Message plaintext.
    pub content: Level,
    /// Identity or account.
    pub identity: Level,
    /// Who talks to whom.
    pub social_graph: Level,
    /// Timing.
    pub timing: Level,
    /// Size.
    pub size: Level,
    /// IP addresses.
    pub network_addr: Level,
    /// Physical proximity.
    pub proximity: Level,
    /// Use of MEM.
    pub existence: Level,
    /// Who observes. A fixed-size list keeps this crate allocation-free.
    pub observers: &'static [Observer],
}

impl ExposureProfile {
    /// Level for one dimension.
    #[must_use]
    pub const fn level(&self, dimension: Dimension) -> Level {
        match dimension {
            Dimension::Content => self.content,
            Dimension::Identity => self.identity,
            Dimension::SocialGraph => self.social_graph,
            Dimension::Timing => self.timing,
            Dimension::Size => self.size,
            Dimension::NetworkAddr => self.network_addr,
            Dimension::Proximity => self.proximity,
            Dimension::Existence => self.existence,
        }
    }

    /// The grade this profile justifies (never grade higher than the profile).
    #[must_use]
    pub fn derived_grade(&self) -> PrivacyGrade {
        let watched_by = |o: Observer| self.observers.contains(&o);
        if self.content != Level::None {
            PrivacyGrade::P5
        } else if watched_by(Observer::ThirdPartyPlatform) || watched_by(Observer::MobileCarrier) {
            PrivacyGrade::P4
        } else if watched_by(Observer::Courier) || watched_by(Observer::DeviceHubOwner) {
            PrivacyGrade::P3
        } else if watched_by(Observer::OurHub) || watched_by(Observer::IrohRelayOperator) {
            PrivacyGrade::P2
        } else if self.network_addr != Level::None || watched_by(Observer::LocalNetwork) {
            PrivacyGrade::P1
        } else {
            PrivacyGrade::P0
        }
    }
}

/// Whether push wake-ups are on, for [`ClaId::exposure_with`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushWake {
    /// No push registration.
    Off,
    /// The hub wakes this device through a push service.
    On,
}

/// Channel identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ClaId {
    /// Animated, fountain-coded QR shown on screen and scanned by the receiver's camera
    /// (`cla-qr`).
    Qr,
    /// Iroh direct QUIC between nodes on the same local network, found by mDNS, relays off
    /// (`cla-iroh`).
    Iroh,
    /// Bluetooth LE between nearby phones, without any network (`app/plugins/mem_radio`).
    /// Off until the user turns it on.
    Ble,
    /// The MEM hub: a store-and-forward mailbox reached over the Internet (`cla-hub`).
    /// Off until the user turns it on.
    Hub,
    /// Armored text handed to another app via share sheet or clipboard (`cla-share`).
    Share,
    /// Carried by other MEM devices that are not the recipient (store-carry-forward).
    /// Not a transport of its own: hand-offs ride on encounters over Iroh or BLE. Used
    /// for consent, standing rules and provenance ("via couriers").
    Courier,
    /// The message text itself, unencrypted, through another app: the P5 fallback.
    /// Only ever after a per-message consent.
    Plain,
    /// Sealed bundles through the user's own mail account, sent and fetched by MEM.
    /// Off until the user adds an account.
    Email,
    /// Wi-Fi Direct between nearby phones, no router or Internet (Android).
    /// The encounter runs over Iroh on the P2P link. Off until the user turns it on.
    WifiDirect,
    /// Sound between phones in the same room (`cla-audio`, ggwave). Carries
    /// **pairing cards only**, never bundles, so it is not in [`ClaId::ALL`] and no message can
    /// pick it.
    Audio,
    /// A contact's device acting as a hub for its owner's contacts, over a direct link (Iroh on
    /// the local network or Wi-Fi Direct). Used for a contact only after the user turns it on.
    DeviceHub,
}

impl ClaId {
    /// All channels that carry bundles, most private first. [`ClaId::Audio`] carries only pairing
    /// cards and is deliberately absent.
    pub const ALL: [Self; 10] = [
        Self::Qr,
        Self::Ble,
        Self::WifiDirect,
        Self::Iroh,
        Self::Hub,
        Self::Courier,
        Self::DeviceHub,
        Self::Email,
        Self::Share,
        Self::Plain,
    ];

    /// Whether the channel is on for a new install. BLE announces that MEM runs to every scanner
    /// nearby, so the user turns it on knowingly.
    #[must_use]
    pub const fn on_by_default(self) -> bool {
        // The hub too: connecting tells the network that the device uses MEM.
        !matches!(self, Self::Ble | Self::Hub | Self::Email | Self::WifiDirect)
    }

    /// Static capabilities.
    #[must_use]
    pub const fn caps(self) -> ClaCaps {
        match self {
            // QR: fountain-coded frames carry any v1 bundle; 64 KiB takes about 170 frames.
            Self::Qr | Self::Share | Self::Plain => ClaCaps {
                max_bundle_len: 65_536,
                needs_user_action: true,
                needs_peer: false,
            },
            Self::Iroh | Self::Ble | Self::WifiDirect | Self::DeviceHub => ClaCaps {
                max_bundle_len: 65_536,
                needs_user_action: false,
                needs_peer: true,
            },
            // Cards only: no bundle fits.
            Self::Audio => ClaCaps {
                max_bundle_len: 0,
                needs_user_action: true,
                needs_peer: false,
            },
            // Hub: automatic, no peer (uploads go whenever it is connected). Courier: never starts
            // an encounter itself; `needs_peer` is false so a peer cannot claim it.
            Self::Hub | Self::Courier | Self::Email => ClaCaps {
                max_bundle_len: 65_536,
                needs_user_action: false,
                needs_peer: false,
            },
        }
    }

    /// Exposure profile of a send over this channel.
    #[must_use]
    #[allow(
        clippy::too_many_lines,
        reason = "one table entry per channel; splitting it would hide the comparison"
    )]
    pub const fn exposure(self) -> ExposureProfile {
        match self {
            Self::Qr => ExposureProfile {
                content: Level::None,
                identity: Level::None,
                social_graph: Level::Limited,
                timing: Level::Limited,
                size: Level::Limited,
                network_addr: Level::None,
                proximity: Level::Exposed,
                existence: Level::Limited,
                observers: &[Observer::NearbyPeople],
            },
            // Everyone within earshot, and any recording device, hears that phones
            // exchanged something (and can record the public card, as with a pairing QR code).
            Self::Audio => ExposureProfile {
                content: Level::None,
                identity: Level::None,
                social_graph: Level::Limited,
                timing: Level::Limited,
                size: Level::Limited,
                network_addr: Level::None,
                proximity: Level::Exposed,
                existence: Level::Exposed,
                observers: &[Observer::NearbyPeople],
            },
            // Relays are disabled, so only the local network observes: mDNS
            // announcements and the QUIC handshake (ALPN) reveal that a device runs MEM; IP
            // and MAC addresses, timing and padded sizes are visible. Transport keys are
            // random per app start, so no identity is exposed.
            Self::Iroh => ExposureProfile {
                content: Level::None,
                identity: Level::None,
                social_graph: Level::Limited,
                timing: Level::Exposed,
                size: Level::Limited,
                network_addr: Level::Exposed,
                proximity: Level::Limited,
                existence: Level::Exposed,
                observers: &[Observer::LocalNetwork],
            },
            // The advertisement holds only the fixed MEM service UUID and a link
            // nonce rotated at least every 15 minutes; the OS randomises the address. No IP.
            Self::Ble => ExposureProfile {
                content: Level::None,
                identity: Level::None,
                social_graph: Level::Limited,
                timing: Level::Exposed,
                size: Level::Limited,
                network_addr: Level::None,
                proximity: Level::Exposed,
                existence: Level::Exposed,
                observers: &[Observer::NearbyPeople],
            },
            // MEM's advertisement holds no identity, but Android broadcasts the
            // phone's Wi-Fi Direct device name (often the owner's name) during discovery. No
            // network operator, no Internet.
            Self::WifiDirect => ExposureProfile {
                content: Level::None,
                identity: Level::Limited,
                social_graph: Level::Limited,
                timing: Level::Exposed,
                size: Level::Limited,
                network_addr: Level::None,
                proximity: Level::Exposed,
                existence: Level::Exposed,
                observers: &[Observer::NearbyPeople],
            },
            // No accounts; the hub sees IPs, timing, padded sizes and 2-byte tag
            // prefixes; the network sees connections to a MEM hub.
            Self::Hub => ExposureProfile {
                content: Level::None,
                identity: Level::None,
                social_graph: Level::Limited,
                timing: Level::Exposed,
                size: Level::Limited,
                network_addr: Level::Exposed,
                proximity: Level::None,
                existence: Level::Exposed,
                observers: &[Observer::OurHub, Observer::LocalNetwork],
            },
            // Whoever runs or can read the other app sees the text itself.
            Self::Plain => ExposureProfile {
                content: Level::Exposed,
                identity: Level::Exposed,
                social_graph: Level::Exposed,
                timing: Level::Exposed,
                size: Level::Exposed,
                network_addr: Level::Exposed,
                proximity: Level::None,
                existence: Level::None,
                observers: &[Observer::ThirdPartyPlatform, Observer::MobileCarrier],
            },
            // Share, and Email: the platform or mail provider sees accounts (often real
            // identities), who talks to whom, when, how much, IPs and that it is MEM traffic;
            // never the content.
            Self::Share | Self::Email => ExposureProfile {
                content: Level::None,
                identity: Level::Exposed,
                social_graph: Level::Exposed,
                timing: Level::Exposed,
                size: Level::Exposed,
                network_addr: Level::Exposed,
                proximity: Level::None,
                existence: Level::Exposed,
                observers: &[Observer::ThirdPartyPlatform],
            },
            // Couriers see envelopes only: a blinded tag rotating daily, the size bucket, rough
            // timing, and that devices met. The local network sees the encounter.
            Self::Courier => ExposureProfile {
                content: Level::None,
                identity: Level::None,
                social_graph: Level::Limited,
                timing: Level::Limited,
                size: Level::Limited,
                network_addr: Level::Limited,
                proximity: Level::Limited,
                existence: Level::Exposed,
                observers: &[
                    Observer::Courier,
                    Observer::LocalNetwork,
                    Observer::NearbyPeople,
                ],
            },
            // The hub device's owner is a contact who learns which of their contacts uploaded
            // and fetched, the tags, timing and sizes, and so who exchanges with whom among
            // their contacts; the local network sees the connection.
            Self::DeviceHub => ExposureProfile {
                content: Level::None,
                identity: Level::Exposed,
                social_graph: Level::Exposed,
                timing: Level::Exposed,
                size: Level::Limited,
                network_addr: Level::Exposed,
                proximity: Level::Exposed,
                existence: Level::Exposed,
                observers: &[Observer::DeviceHubOwner, Observer::LocalNetwork],
            },
        }
    }

    /// Exposure profile with the user's settings that change it: with push wake-ups on, the push
    /// service also learns when the hub has something for this device. The grade is
    /// the same either way.
    #[must_use]
    pub const fn exposure_with(self, push: PushWake) -> ExposureProfile {
        match (self, push) {
            (Self::Hub, PushWake::On) => ExposureProfile {
                observers: &[
                    Observer::OurHub,
                    Observer::LocalNetwork,
                    Observer::PushProvider,
                ],
                ..self.exposure()
            },
            _ => self.exposure(),
        }
    }

    /// Grade, derived from the exposure profile so it can never be over-stated.
    #[must_use]
    pub fn grade(self) -> PrivacyGrade {
        self.exposure().derived_grade()
    }

    /// Stable identifier for storage and FFI.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Qr => "qr",
            Self::Iroh => "iroh",
            Self::Ble => "ble",
            Self::WifiDirect => "wifi-direct",
            Self::Audio => "audio",
            Self::Hub => "hub",
            Self::Share => "share",
            Self::Plain => "plain",
            Self::Email => "email",
            Self::Courier => "courier",
            Self::DeviceHub => "device-hub",
        }
    }

    /// Inverse of [`ClaId::as_str`].
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.as_str() == s)
    }
}

/// Static capabilities of a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClaCaps {
    /// Largest encoded bundle the channel carries in one transfer.
    pub max_bundle_len: usize,
    /// Whether every transfer needs the user to do something (show/scan, pick an app).
    pub needs_user_action: bool,
    /// Whether a send needs a connected peer that advertised the recipient's tag (network
    /// channels), rather than a user hand-off.
    pub needs_peer: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// With push on, the hub's profile names the push service; the grade stays P2.
    #[test]
    fn push_adds_an_observer_not_a_grade() {
        let on = ClaId::Hub.exposure_with(PushWake::On);
        assert!(on.observers.contains(&Observer::PushProvider));
        assert!(
            !ClaId::Hub
                .exposure()
                .observers
                .contains(&Observer::PushProvider)
        );
        assert_eq!(on.derived_grade(), PrivacyGrade::P2);
        assert_eq!(
            ClaId::Email.exposure_with(PushWake::On),
            ClaId::Email.exposure()
        );
    }

    #[test]
    fn grades() {
        assert_eq!(ClaId::Qr.grade(), PrivacyGrade::P0);
        assert_eq!(ClaId::Iroh.grade(), PrivacyGrade::P1);
        assert_eq!(ClaId::Ble.grade(), PrivacyGrade::P0);
        assert_eq!(ClaId::WifiDirect.grade(), PrivacyGrade::P0);
        // Sound: pairing cards only, never a message channel.
        assert_eq!(ClaId::Audio.grade(), PrivacyGrade::P0);
        assert_eq!(ClaId::Audio.caps().max_bundle_len, 0);
        assert!(!ClaId::ALL.contains(&ClaId::Audio));
        assert!(!ClaId::WifiDirect.on_by_default());
        assert!(!ClaId::Ble.on_by_default());
        assert_eq!(ClaId::Hub.grade(), PrivacyGrade::P2);
        assert!(!ClaId::Hub.on_by_default());
        assert!(ClaId::Hub.grade().within(DEFAULT_FLOOR));
        assert!(ClaId::Iroh.grade().within(DEFAULT_FLOOR));
        assert_eq!(ClaId::Share.grade(), PrivacyGrade::P4);
        assert_eq!(ClaId::Plain.grade(), PrivacyGrade::P5);
        assert_eq!(ClaId::Email.grade(), PrivacyGrade::P4);
        assert!(!ClaId::Email.grade().within(DEFAULT_FLOOR));
        assert!(!ClaId::Email.on_by_default());
        assert_eq!(ClaId::Courier.grade(), PrivacyGrade::P3);
        assert_eq!(ClaId::DeviceHub.grade(), PrivacyGrade::P3);
        assert!(ClaId::Courier.grade().within(DEFAULT_FLOOR));
        assert!(!ClaId::Courier.grade().within(LEGACY_DEFAULT_FLOOR));
        assert!(ClaId::Qr.grade().within(DEFAULT_FLOOR));
        assert!(!ClaId::Share.grade().within(DEFAULT_FLOOR));
    }

    #[test]
    fn content_exposure_is_always_p5() {
        let mut p = ClaId::Qr.exposure();
        p.content = Level::Limited;
        assert_eq!(p.derived_grade(), PrivacyGrade::P5);
    }

    #[test]
    fn ids_roundtrip() {
        for c in ClaId::ALL {
            assert_eq!(ClaId::parse(c.as_str()), Some(c));
        }
    }
}
