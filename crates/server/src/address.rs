//! Where a request came from, for the journal to say.
//!
//! The address the connection came from, unless it came from this machine
//! or the local network and carries the address a proxy in front passed on:
//! behind nginx, every request arrives from nginx, and the address worth
//! writing down is the one it saw. A header from anywhere further away is
//! not believed, since anybody can write one.
//!
//! Written down to be read by the administrator, never to decide anything.

use std::convert::Infallible;
use std::net::{IpAddr, SocketAddr};

use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::request::Parts;
use axum::http::HeaderMap;

/// The address a request came from, when it can be told.
pub struct Caller(pub Option<String>);

impl<S: Send + Sync> FromRequestParts<S> for Caller {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let peer = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|ConnectInfo(address)| address.ip());
        Ok(Self(address_of(peer, &parts.headers).map(|address| address.to_string())))
    }
}

/// Whether a proxy may sit at this address: this machine or the local
/// network, which is where the one in front of this server lives.
fn nearby(address: IpAddr) -> bool {
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
    fn without_a_proxy_the_connection_says_it() {
        assert_eq!(address_of(Some(ip("192.168.1.30")), &HeaderMap::new()), Some(ip("192.168.1.30")));
        let said = headers(&[("x-forwarded-for", "not an address")]);
        assert_eq!(address_of(Some(ip("192.168.1.30")), &said), Some(ip("192.168.1.30")));
        assert_eq!(address_of(None, &HeaderMap::new()), None);
    }
}
