//! Which peers may talk to the LAN server, and which address to show.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, UdpSocket};

/// Whether `ip` belongs to a private network the server serves: loopback,
/// RFC 1918, link-local, CGNAT 100.64/10 (Tailscale) and IPv6 ULA/link-local.
/// Everything else — the public internet — is refused.
pub fn is_allowed_peer(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_private_v4(v4),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => is_private_v4(v4),
            None => is_private_v6(v6),
        },
    }
}

fn is_private_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        // CGNAT 100.64.0.0/10, used by Tailscale and some office VPNs.
        || (a == 100 && (64..=127).contains(&b))
}

fn is_private_v6(ip: Ipv6Addr) -> bool {
    let first = ip.segments()[0];
    ip.is_loopback()
        // Unique local fc00::/7.
        || (first & 0xfe00) == 0xfc00
        // Link-local fe80::/10.
        || (first & 0xffc0) == 0xfe80
}

/// The IPv4 address other computers in the LAN reach this one at, if it is a
/// private address. A UDP "connect" picks the outgoing interface without
/// sending any packet.
pub fn primary_ipv4() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    // TEST-NET-1 (RFC 5737): never routed, nothing is sent by connect().
    socket.connect((Ipv4Addr::new(192, 0, 2, 1), 9)).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if is_private_v4(ip) && !ip.is_loopback() => Some(ip),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed(s: &str) -> bool {
        is_allowed_peer(s.parse().unwrap())
    }

    #[test]
    fn private_networks_are_allowed() {
        for ip in [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.5",
            "172.31.255.1",
            "192.168.1.10",
            "169.254.3.4",
            "100.64.0.1",
            "100.127.255.254",
            "::1",
            "fd12:3456::1",
            "fe80::1",
            "::ffff:192.168.0.7",
        ] {
            assert!(allowed(ip), "{ip}");
        }
    }

    #[test]
    fn public_addresses_are_refused() {
        for ip in [
            "8.8.8.8",
            "172.32.0.1",
            "100.128.0.1",
            "192.169.0.1",
            "2001:db8::1",
            "2a00:1450::1",
            "::ffff:8.8.8.8",
        ] {
            assert!(!allowed(ip), "{ip}");
        }
    }
}
