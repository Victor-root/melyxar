//! Passkeys: the site a key is made for, and the checking of what a device
//! answers, left whole to `webauthn-rs`.
//!
//! Nothing here keeps anything. The two halves of a ceremony, the challenge
//! sent and the answer that comes back, are held by whoever calls between the
//! two; the keys made are kept by the database. What is decided here is the
//! one thing that has to be right before any of it: which site a key belongs
//! to.
//!
//! A key belongs to the name the page was opened under, read from the origin
//! the browser writes on its own request. A browser only makes and offers a
//! key on a page in https under a domain name, so an address, a page in
//! plain http or an origin of any other shape is no site at all. Taking the
//! name from the request rather than from a setting asks nothing of whoever
//! installs the server, behind a proxy or not, and opens nothing: the device
//! signs the origin it saw, so an answer made under another name, or claimed
//! for one, does not pass.

use std::time::Duration;

pub use webauthn_rs::prelude::{
    AuthenticationResult, CreationChallengeResponse, CredentialID, DiscoverableAuthentication, Passkey, PasskeyRegistration, PublicKeyCredential, RegisterPublicKeyCredential,
    RequestChallengeResponse, Url, Webauthn, WebauthnError,
};
use webauthn_rs::WebauthnBuilder;

/// How long a device is given to answer, and a challenge kept waiting for it.
pub const CEREMONY_LASTS: Duration = Duration::from_secs(300);

/// The site a passkey is made for and checked against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Site {
    /// The domain the key is bound to.
    pub rp_id: String,
    /// The origin the page was opened under, port included when it is not
    /// the usual one.
    pub origin: Url,
}

impl Site {
    /// The site an origin names, when a passkey can be made for it: https
    /// under a domain name. Nothing for an address, plain http, or anything
    /// else a browser would never offer a key on.
    pub fn of_origin(origin: &str) -> Option<Self> {
        let origin = Url::parse(origin.trim()).ok()?;
        if origin.scheme() != "https"
            || !origin.username().is_empty()
            || origin.password().is_some()
            || !matches!(origin.path(), "" | "/")
            || origin.query().is_some()
            || origin.fragment().is_some()
        {
            return None;
        }
        let rp_id = match origin.host()? {
            url::Host::Domain(domain) if !domain.is_empty() => domain.to_ascii_lowercase(),
            url::Host::Domain(_) | url::Host::Ipv4(_) | url::Host::Ipv6(_) => return None,
        };
        Some(Self { rp_id, origin })
    }
}

/// What checks the ceremonies of one site: the browser's origin, and the
/// applications allowed to make keys for it, by the origin Android gives them
/// (`android:apk-key-hash:` and the fingerprint of their signature).
pub fn checker_for(site: &Site, server_name: &str, applications: &[String]) -> Result<Webauthn, WebauthnError> {
    let mut builder = WebauthnBuilder::new(&site.rp_id, &site.origin)?
        .rp_name(server_name)
        .timeout(CEREMONY_LASTS);
    for application in applications {
        if let Ok(origin) = Url::parse(application) {
            builder = builder.append_allowed_origin(&origin);
        }
    }
    builder.build()
}

/// The origin Android gives an application signed with the certificate of
/// this SHA-256 fingerprint, written as hexadecimal pairs with or without
/// colons. Nothing for a fingerprint of any other shape.
pub fn application_origin(fingerprint: &str) -> Option<String> {
    use base64::Engine;
    let hex: String = fingerprint.chars().filter(|c| *c != ':').collect();
    if hex.len() != 64 {
        return None;
    }
    let bytes = (0..32)
        .map(|at| u8::from_str_radix(hex.get(at * 2..at * 2 + 2)?, 16).ok())
        .collect::<Option<Vec<u8>>>()?;
    Some(format!(
        "android:apk-key-hash:{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_site_is_a_domain_name_in_https() {
        let site = Site::of_origin("https://Melyxar.Example.org").expect("a site");
        assert_eq!(site.rp_id, "melyxar.example.org");
        let site = Site::of_origin("https://media.example.org:8443").expect("a site");
        assert_eq!(site.rp_id, "media.example.org");
        assert_eq!(site.origin.port(), Some(8443));
    }

    #[test]
    fn an_address_plain_http_or_anything_else_is_no_site() {
        for refused in [
            "http://melyxar.example.org",
            "https://192.168.1.21:2100",
            "https://[fe80::1]",
            "https://melyxar.example.org/door",
            "https://someone:secret@melyxar.example.org",
            "null",
            "",
            "android:apk-key-hash:abc",
        ] {
            assert!(Site::of_origin(refused).is_none(), "{refused}");
        }
    }

    #[test]
    fn an_application_is_known_by_the_hash_of_its_signature() {
        let fingerprint = "F2:2B:D7:B4:63:D8:D8:9C:A1:AC:3B:6C:41:DB:0B:25:AA:C7:7B:86:24:C9:70:E4:52:81:2D:32:19:42:A9:71";
        let origin = application_origin(fingerprint).expect("an origin");
        assert_eq!(origin, "android:apk-key-hash:8ivXtGPY2JyhrDtsQdsLJarHe4YkyXDkUoEtMhlCqXE");
        assert_eq!(application_origin(&fingerprint.replace(':', "")), Some(origin));
        assert!(application_origin("not a fingerprint").is_none());
    }

    #[test]
    fn a_checker_is_made_for_a_site_and_its_applications() {
        let site = Site::of_origin("https://melyxar.example.org").expect("a site");
        let android = application_origin(&"ab".repeat(32)).expect("an origin");
        let checker = checker_for(&site, "Melyxar", &[android]).expect("a checker");
        assert_eq!(checker.get_allowed_origins().len(), 2);
    }
}
