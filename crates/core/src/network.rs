//! Who is behind an address, as far as counting them goes.

use std::net::{IpAddr, Ipv6Addr};

/// The one party an address stands for: an IPv4 address itself, or the
/// network an IPv6 one belongs to.
///
/// A single customer is handed billions of IPv6 addresses, a whole `/64` and
/// often more, so counting each apart would let one machine start afresh
/// with every connection or every try. An IPv4 address written the IPv6 way
/// is the IPv4 address it carries.
pub fn party_of(address: IpAddr) -> IpAddr {
    match address {
        IpAddr::V4(_) => address,
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => IpAddr::V4(v4),
            None => IpAddr::V6(Ipv6Addr::from(u128::from(v6) & (u128::MAX << 64))),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn party(address: &str) -> IpAddr {
        party_of(address.parse().expect("an address"))
    }

    #[test]
    fn one_ipv6_network_is_one_party() {
        assert_eq!(party("2001:db8:1:2::1"), party("2001:db8:1:2:ffff::9"));
        assert_ne!(party("2001:db8:1:2::1"), party("2001:db8:1:3::1"));
    }

    #[test]
    fn an_ipv4_address_is_its_own_party_however_it_is_written() {
        assert_eq!(party("::ffff:203.0.113.9"), party("203.0.113.9"));
        assert_ne!(party("203.0.113.9"), party("203.0.113.10"));
    }
}
