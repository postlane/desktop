// SPDX-License-Identifier: BUSL-1.1

use std::net::IpAddr;

/// Returns true if the IP falls within a private, loopback, link-local, or reserved range.
pub fn is_private_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback() || v4.is_private() || v4.is_link_local()
                || v4.is_broadcast() || v4.is_unspecified()
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                || (v6.segments()[0] & 0xfe00 == 0xfc00) // fc00::/7 unique-local
                || (v6.segments()[0] & 0xffc0 == 0xfe80) // fe80::/10 link-local
        }
    }
}

/// Returns true if a bare hostname or literal IP string represents a private address.
pub fn is_private_host_str(host: &str) -> bool {
    if matches!(host, "localhost" | "localhost.localdomain" | "ip6-localhost" | "ip6-loopback") {
        return true;
    }
    host.parse::<IpAddr>().is_ok_and(is_private_ip)
}

/// Returns true if a full URL resolves to a private or reserved address.
/// Returns true on parse failure or missing host.
pub fn is_private_url(url: &str) -> bool {
    let parsed = match url::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return true,
    };
    match parsed.host() {
        None => true,
        Some(url::Host::Domain(d)) => is_private_host_str(d),
        Some(url::Host::Ipv4(v4)) => is_private_ip(IpAddr::V4(v4)),
        Some(url::Host::Ipv6(v6)) => is_private_ip(IpAddr::V6(v6)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    // --- is_private_ip ---

    #[test]
    fn ipv4_loopback_is_private() {
        let ip: IpAddr = "127.0.0.1".parse().unwrap();
        assert!(is_private_ip(ip));
    }

    #[test]
    fn ipv4_rfc1918_10_is_private() {
        let ip: IpAddr = "10.0.0.1".parse().unwrap();
        assert!(is_private_ip(ip));
    }

    #[test]
    fn ipv4_rfc1918_172_is_private() {
        let ip: IpAddr = "172.20.0.1".parse().unwrap();
        assert!(is_private_ip(ip));
    }

    #[test]
    fn ipv4_rfc1918_192_168_is_private() {
        let ip: IpAddr = "192.168.1.1".parse().unwrap();
        assert!(is_private_ip(ip));
    }

    #[test]
    fn ipv4_link_local_is_private() {
        let ip: IpAddr = "169.254.169.254".parse().unwrap();
        assert!(is_private_ip(ip));
    }

    #[test]
    fn ipv4_broadcast_is_private() {
        let ip: IpAddr = "255.255.255.255".parse().unwrap();
        assert!(is_private_ip(ip));
    }

    #[test]
    fn ipv4_unspecified_is_private() {
        let ip: IpAddr = "0.0.0.0".parse().unwrap();
        assert!(is_private_ip(ip));
    }

    #[test]
    fn ipv6_loopback_is_private() {
        let ip: IpAddr = "::1".parse().unwrap();
        assert!(is_private_ip(ip));
    }

    #[test]
    fn ipv6_unique_local_fc00_is_private() {
        let ip: IpAddr = "fc00::1".parse().unwrap();
        assert!(is_private_ip(ip));
    }

    #[test]
    fn ipv6_unique_local_fd00_is_private() {
        let ip: IpAddr = "fd00::1".parse().unwrap();
        assert!(is_private_ip(ip));
    }

    #[test]
    fn ipv6_link_local_is_private() {
        let ip: IpAddr = "fe80::1".parse().unwrap();
        assert!(is_private_ip(ip));
    }

    #[test]
    fn ipv4_public_is_not_private() {
        let ip: IpAddr = "93.184.216.34".parse().unwrap();
        assert!(!is_private_ip(ip));
    }

    // --- is_private_host_str ---

    #[test]
    fn localhost_is_private_host() {
        assert!(is_private_host_str("localhost"));
    }

    #[test]
    fn localhost_localdomain_is_private_host() {
        assert!(is_private_host_str("localhost.localdomain"));
    }

    #[test]
    fn ip6_localhost_is_private_host() {
        assert!(is_private_host_str("ip6-localhost"));
    }

    #[test]
    fn ip6_loopback_is_private_host() {
        assert!(is_private_host_str("ip6-loopback"));
    }

    #[test]
    fn private_ip_string_is_private_host() {
        assert!(is_private_host_str("192.168.1.1"));
    }

    #[test]
    fn public_hostname_is_not_private_host() {
        assert!(!is_private_host_str("example.com"));
    }

    // --- is_private_url ---

    #[test]
    fn url_loopback_ipv4_is_private() {
        assert!(is_private_url("https://127.0.0.1/path"));
    }

    #[test]
    fn url_loopback_ipv4_port_is_private() {
        assert!(is_private_url("https://127.0.0.1:8080/path"));
    }

    #[test]
    fn url_rfc1918_10_is_private() {
        assert!(is_private_url("https://10.0.0.1/"));
    }

    #[test]
    fn url_rfc1918_172_is_private() {
        assert!(is_private_url("https://172.20.0.1/"));
    }

    #[test]
    fn url_rfc1918_192_168_is_private() {
        assert!(is_private_url("https://192.168.1.1/"));
    }

    #[test]
    fn url_aws_metadata_is_private() {
        assert!(is_private_url("https://169.254.169.254/latest/meta-data/"));
    }

    #[test]
    fn url_localhost_is_private() {
        assert!(is_private_url("https://localhost/"));
    }

    #[test]
    fn url_localhost_localdomain_is_private() {
        assert!(is_private_url("https://localhost.localdomain/"));
    }

    #[test]
    fn url_ipv6_loopback_is_private() {
        assert!(is_private_url("https://[::1]/"));
    }

    #[test]
    fn url_ipv6_unique_local_fd00_is_private() {
        assert!(is_private_url("https://[fd00::1]/"));
    }

    #[test]
    fn url_ipv6_unique_local_fc00_is_private() {
        assert!(is_private_url("https://[fc00::1]/"));
    }

    #[test]
    fn url_broadcast_is_private() {
        assert!(is_private_url("https://255.255.255.255/"));
    }

    #[test]
    fn url_unspecified_is_private() {
        assert!(is_private_url("https://0.0.0.0/"));
    }

    #[test]
    fn url_public_domain_is_not_private() {
        assert!(!is_private_url("https://example.com/image.png"));
    }

    #[test]
    fn url_cdn_is_not_private() {
        assert!(!is_private_url("https://images.unsplash.com/photo-123"));
    }

    #[test]
    fn unparseable_url_is_private() {
        assert!(is_private_url("not-a-url"));
    }

    #[test]
    fn file_url_no_host_is_private() {
        assert!(is_private_url("file:///etc/passwd"));
    }
}
