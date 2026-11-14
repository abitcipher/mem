//! Human-readable consent disclosures, generated only from the exposure profile.
//!
//! The UI may style this text but must not paraphrase exposures away. Wording is plain and
//! factual: no marketing language, no reassurance beyond what the profile states.

use alloc::string::String;
use alloc::vec::Vec;

use mem_cla::{ClaId, Dimension, ExposureProfile, Level, Observer, PrivacyGrade};

/// UI languages supported from day one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    /// English.
    En,
    /// Ukrainian.
    Uk,
}

/// A rendered disclosure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disclosure {
    /// One-line summary.
    pub title: String,
    /// Who observes, as a phrase.
    pub observers: String,
    /// What observers learn fully.
    pub exposed: Vec<String>,
    /// What observers learn partially.
    pub limited: Vec<String>,
    /// What stays hidden from observers.
    pub protected: Vec<String>,
}

/// Renders the disclosure for sending over `cla` at `grade`, below `floor`.
#[must_use]
pub fn render(
    cla: ClaId,
    grade: PrivacyGrade,
    floor: PrivacyGrade,
    profile: &ExposureProfile,
    locale: Locale,
) -> Disclosure {
    let title = match locale {
        Locale::En => alloc::format!(
            "{} is {}, below this contact's privacy floor {}.",
            channel_name(cla, locale),
            grade.label(),
            floor.label()
        ),
        Locale::Uk => alloc::format!(
            "{} має рівень {}, нижчий за поріг приватності цього контакту {}.",
            channel_name(cla, locale),
            grade.label(),
            floor.label()
        ),
    };
    let observers = profile
        .observers
        .iter()
        .map(|o| observer_name(*o, locale))
        .collect::<Vec<_>>()
        .join(", ");

    let mut out = Disclosure {
        title,
        observers,
        exposed: Vec::new(),
        limited: Vec::new(),
        protected: Vec::new(),
    };
    for d in Dimension::ALL {
        let text = String::from(channel_dimension_text(cla, d, locale));
        match profile.level(d) {
            Level::Exposed => out.exposed.push(text),
            Level::Limited => out.limited.push(text),
            Level::None => out.protected.push(text),
        }
    }
    out
}

/// Localized channel name.
#[must_use]
pub const fn channel_name(cla: ClaId, locale: Locale) -> &'static str {
    match (cla, locale) {
        (ClaId::Qr, Locale::En) => "QR code on screen",
        (ClaId::Qr, Locale::Uk) => "QR-код на екрані",
        (ClaId::Iroh, Locale::En) => "Wi-Fi: the same local network",
        (ClaId::Iroh, Locale::Uk) => "Wi-Fi: та сама локальна мережа",
        (ClaId::Ble, Locale::En) => "Bluetooth: phones next to each other",
        (ClaId::Ble, Locale::Uk) => "Bluetooth: телефони поруч",
        (ClaId::WifiDirect, Locale::En) => "Wi-Fi Direct: phones nearby, no router",
        (ClaId::WifiDirect, Locale::Uk) => "Wi-Fi Direct: телефони поруч, без роутера",
        (ClaId::Hub, Locale::En) => "MEM hub: a mailbox on the Internet",
        (ClaId::Hub, Locale::Uk) => "Хаб MEM: поштова скринька в інтернеті",
        (ClaId::Plain, Locale::En) => "Plain text through another app (not encrypted)",
        (ClaId::Plain, Locale::Uk) => "Відкритий текст через інший застосунок (без шифрування)",
        (ClaId::Email, Locale::En) => "Email, through your mail account",
        (ClaId::Email, Locale::Uk) => "Електронна пошта, через ваш поштовий акаунт",
        (ClaId::Share, Locale::En) => "Sharing through another app",
        (ClaId::Share, Locale::Uk) => "Надсилання через інший застосунок",
        (ClaId::Courier, Locale::En) => "Carried by nearby MEM devices",
        (ClaId::Courier, Locale::Uk) => "Через пристрої MEM поруч",
        (ClaId::Audio, Locale::En) => "Sound: phones in the same room",
        (ClaId::Audio, Locale::Uk) => "Звук: телефони в одній кімнаті",
        (ClaId::DeviceHub, Locale::En) => "A contact's device acting as a hub",
        (ClaId::DeviceHub, Locale::Uk) => "Пристрій контакту як хаб",
    }
}

const fn observer_name(o: Observer, locale: Locale) -> &'static str {
    match (o, locale) {
        (Observer::NearbyPeople, Locale::En) => "people nearby",
        (Observer::NearbyPeople, Locale::Uk) => "люди поруч",
        (Observer::LocalNetwork, Locale::En) => "the local network operator",
        (Observer::LocalNetwork, Locale::Uk) => "оператор локальної мережі",
        (Observer::Courier, Locale::En) => "MEM courier devices",
        (Observer::Courier, Locale::Uk) => "пристрої-кур'єри MEM",
        (Observer::OurHub, Locale::En) => "the MEM hub",
        (Observer::OurHub, Locale::Uk) => "хаб MEM",
        (Observer::IrohRelayOperator, Locale::En) => "the relay operator",
        (Observer::IrohRelayOperator, Locale::Uk) => "оператор ретранслятора",
        (Observer::ThirdPartyPlatform, Locale::En) => "the app you share through and its servers",
        (Observer::ThirdPartyPlatform, Locale::Uk) => {
            "застосунок, через який ви надсилаєте, і його сервери"
        }
        (Observer::MobileCarrier, Locale::En) => "your mobile carrier",
        (Observer::MobileCarrier, Locale::Uk) => "ваш мобільний оператор",
        (Observer::PushProvider, Locale::En) => "the push service you chose (for example ntfy)",
        (Observer::PushProvider, Locale::Uk) => "обраний вами push-сервіс (наприклад, ntfy)",
        (Observer::DeviceHubOwner, Locale::En) => {
            "the contact whose device is the hub (they see which of their contacts send and fetch)"
        }
        (Observer::DeviceHubOwner, Locale::Uk) => {
            "контакт, чий пристрій є хабом (бачить, хто з їхніх контактів надсилає й отримує)"
        }
    }
}

/// A dimension's wording, where a channel needs its own (on Wi-Fi Direct the identity
/// that leaks is the phone's name, not an account).
const fn channel_dimension_text(cla: ClaId, d: Dimension, locale: Locale) -> &'static str {
    match (cla, d, locale) {
        (ClaId::WifiDirect, Dimension::Identity, Locale::En) => {
            "your phone's Wi-Fi Direct name (often your own name)"
        }
        (ClaId::WifiDirect, Dimension::Identity, Locale::Uk) => {
            "назву вашого телефона у Wi-Fi Direct (часто ваше ім'я)"
        }
        _ => dimension_text(d, locale),
    }
}

const fn dimension_text(d: Dimension, locale: Locale) -> &'static str {
    match (d, locale) {
        (Dimension::Content, Locale::En) => "the message text",
        (Dimension::Content, Locale::Uk) => "текст повідомлення",
        (Dimension::Identity, Locale::En) => "who you are (your account on that service)",
        (Dimension::Identity, Locale::Uk) => "хто ви (ваш обліковий запис у цьому сервісі)",
        (Dimension::SocialGraph, Locale::En) => "that you and this person communicate",
        (Dimension::SocialGraph, Locale::Uk) => "що ви спілкуєтеся з цією людиною",
        (Dimension::Timing, Locale::En) => "when the message was sent",
        (Dimension::Timing, Locale::Uk) => "коли повідомлення було надіслано",
        (Dimension::Size, Locale::En) => "the approximate size of the message",
        (Dimension::Size, Locale::Uk) => "приблизний розмір повідомлення",
        (Dimension::NetworkAddr, Locale::En) => "your IP address",
        (Dimension::NetworkAddr, Locale::Uk) => "вашу IP-адресу",
        (Dimension::Proximity, Locale::En) => "that your devices were close to each other",
        (Dimension::Proximity, Locale::Uk) => "що ваші пристрої були поруч",
        (Dimension::Existence, Locale::En) => "that you use MEM",
        (Dimension::Existence, Locale::Uk) => "що ви користуєтеся MEM",
    }
}

/// The disclosure shown before every attestation. Built here, like channel
/// disclosures, so the UI can style it but not rewrite it.
#[must_use]
pub fn attestation(rp_origin: &str, subject_label: &str, locale: Locale) -> String {
    match locale {
        Locale::En => alloc::format!(
            "This is a signed statement that you agreed to {subject_label} at {rp_origin}, in \
             person, with the phone you are about to scan. Unlike your MEM messages, anyone who \
             gets it can check that it is genuine, and you cannot deny it later. {rp_origin} will \
             see that this phone sent it, your internet address and the time. It does not reveal \
             your MEM identity or contacts. {rp_origin} will recognise you if you confirm with it \
             again."
        ),
        Locale::Uk => alloc::format!(
            "Це підписана заява, що ви погодили {subject_label} на {rp_origin} особисто, з \
             телефоном, який зараз скануватимете. На відміну від ваших повідомлень у MEM, будь-хто, \
             хто її отримає, може перевірити її справжність, і ви не зможете потім від неї \
             відмовитися. {rp_origin} побачить, що її надіслав цей телефон, вашу інтернет-адресу \
             й час. Ваша особа в MEM і ваші контакти не розкриваються. {rp_origin} упізнає вас, \
             якщо ви знову підтвердите щось через нього."
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_disclosure_names_every_exposure_and_keeps_content_protected() {
        let p = ClaId::Share.exposure();
        let d = render(
            ClaId::Share,
            PrivacyGrade::P4,
            PrivacyGrade::P2,
            &p,
            Locale::En,
        );
        assert!(d.title.contains("P4") && d.title.contains("P2"));
        assert_eq!(d.exposed.len(), 6);
        assert!(d.protected.iter().any(|s| s == "the message text"));
        assert!(d.exposed.iter().any(|s| s == "that you use MEM"));
        assert!(d.observers.contains("app you share through"));
    }

    #[test]
    fn every_dimension_is_rendered_exactly_once_in_both_locales() {
        for locale in [Locale::En, Locale::Uk] {
            for cla in ClaId::ALL {
                let p = cla.exposure();
                let d = render(cla, cla.grade(), PrivacyGrade::P2, &p, locale);
                assert_eq!(d.exposed.len() + d.limited.len() + d.protected.len(), 8);
            }
        }
    }

    #[test]
    fn attestation_disclosure_says_it_is_not_deniable() {
        let en = attestation("https://basa.ua", "basa0:abcd", Locale::En);
        assert!(en.contains("cannot deny it later"));
        assert!(en.contains("https://basa.ua will see"));
        assert!(en.contains("basa0:abcd"));
        let uk = attestation("https://basa.ua", "basa0:abcd", Locale::Uk);
        assert!(uk.contains("не зможете"));
    }
}
