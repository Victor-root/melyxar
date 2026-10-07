//! Where a request came from, for the journal to say, and whether it came
//! encrypted, for the cookie to say.
//!
//! The address the connection came from, unless it came from this machine
//! or the local network and carries the address a proxy in front passed on:
//! behind nginx, every request arrives from nginx, and the address worth
//! writing down is the one it saw. A header from anywhere further away is
//! not believed, since anybody can write one.
//!
//! Written down to be read by the administrator, and counted by the brake on
//! wrong passwords, which holds back an address rather than an account. That
//! is the one thing it decides, and why a proxy in front has to write the
//! address it saw itself rather than pass on what the browser claimed.

use std::convert::Infallible;
use std::net::IpAddr;

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::request::Parts;
use axum::http::HeaderMap;

use crate::door::Peer;

/// Where a request came from, when it can be told, and whether it travelled
/// encrypted to whoever the browser spoke to.
pub struct Caller {
    pub address: Option<IpAddr>,
    pub encrypted: bool,
    /// Whether it came from this machine or the local network.
    pub local: bool,
}

impl<S: Send + Sync> FromRequestParts<S> for Caller {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let peer = parts.extensions.get::<ConnectInfo<Peer>>().map(|ConnectInfo(peer)| *peer);
        let ip = peer.map(|peer| peer.address.ip());
        let address = address_of(ip, &parts.headers);
        Ok(Self {
            local: address.is_some_and(nearby),
            address,
            encrypted: peer.is_some_and(|peer| peer.encrypted) || proxy_encrypted(ip, &parts.headers),
        })
    }
}

/// Whether a proxy in front says the browser spoke to it encrypted.
fn proxy_encrypted(peer: Option<IpAddr>, headers: &HeaderMap) -> bool {
    peer.is_some_and(nearby)
        && headers
            .get("x-forwarded-proto")
            .and_then(|value| value.to_str().ok())
            .is_some_and(|proto| proto.trim().eq_ignore_ascii_case("https"))
}

/// Whether a proxy may sit at this address: this machine or the local
/// network, which is where the one in front of this server lives.
pub(crate) fn nearby(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private() || v4.is_link_local(),
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unique_local()
                || v6.is_unicast_link_local()
                || v6.to_ipv4_mapped().is_some_and(|v4| nearby(IpAddr::V4(v4)))
        }
    }
}

/// What a proxy said it saw: the last address of `X-Forwarded-For`, which is
/// the one the nearest proxy added, else `X-Real-IP`.
fn passed_on(headers: &HeaderMap) -> Option<IpAddr> {
    let header = |name: &str| headers.get(name).and_then(|value| value.to_str().ok());
    header("x-forwarded-for")
        .and_then(|list| list.rsplit(',').next())
        .or_else(|| header("x-real-ip"))
        .and_then(|text| text.trim().parse().ok())
}

pub fn address_of(peer: Option<IpAddr>, headers: &HeaderMap) -> Option<IpAddr> {
    match peer {
        Some(peer) if !nearby(peer) => Some(peer),
        _ => passed_on(headers).or(peer),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in pairs {
            map.insert(*name, value.parse().expect("header"));
        }
        map
    }

    fn ip(text: &str) -> IpAddr {
        text.parse().expect("address")
    }

    #[test]
    fn behind_a_proxy_on_the_local_network_the_address_it_saw_is_kept() {
        let said = headers(&[("x-forwarded-for", "203.0.113.9, 198.51.100.4")]);
        assert_eq!(address_of(Some(ip("192.168.1.20")), &said), Some(ip("198.51.100.4")));
        let said = headers(&[("x-real-ip", "203.0.113.9")]);
        assert_eq!(address_of(Some(ip("127.0.0.1")), &said), Some(ip("203.0.113.9")));
    }

    #[test]
    fn a_header_from_far_away_is_not_believed() {
        let said = headers(&[("x-forwarded-for", "10.0.0.1")]);
        assert_eq!(address_of(Some(ip("203.0.113.9")), &said), Some(ip("203.0.113.9")));
    }

    #[test]
    fn only_a_proxy_nearby_is_believed_about_encryption() {
        let said = headers(&[("x-forwarded-proto", "https")]);
        assert!(proxy_encrypted(Some(ip("192.168.1.20")), &said));
        assert!(!proxy_encrypted(Some(ip("203.0.113.9")), &said));
        assert!(!proxy_encrypted(Some(ip("192.168.1.20")), &HeaderMap::new()));
    }

    #[test]
    fn without_a_proxy_the_connection_says_it() {
        assert_eq!(address_of(Some(ip("192.168.1.30")), &HeaderMap::new()), Some(ip("192.168.1.30")));
        let said = headers(&[("x-forwarded-for", "not an address")]);
        assert_eq!(address_of(Some(ip("192.168.1.30")), &said), Some(ip("192.168.1.30")));
        assert_eq!(address_of(None, &HeaderMap::new()), None);
    }
}
