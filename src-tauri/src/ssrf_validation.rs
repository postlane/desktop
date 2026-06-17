// SPDX-License-Identifier: BUSL-1.1

//! SSRF validation for external URLs (22.7.4 Step 3).
//!
//! Validates that a URL does not point to a private/loopback address range.
//! The host is resolved to an IP before checking — string matching alone is
//! insufficient because hostnames can resolve to private IPs.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Returns `Ok(())` if the URL is safe to fetch, `Err(reason)` otherwise.
///
/// Rejects:
/// - `http://` scheme (only `https://` is permitted)
/// - Bare IP address hosts
/// - URLs whose resolved IP falls in a private or loopback range
///
/// Private ranges checked: 127.x, 10.x, 172.16–31.x, 192.168.x, 169.254.x,
/// fd00::/8, ::1, and IPv4-mapped equivalents (::ffff:10.x, etc.).
pub fn validate_ssrf_url(url: &str) -> Result<(), String> {
    validate_ssrf_url_with_resolver(url, resolve_host)
}

/// Testable variant that accepts a custom resolver function.
pub fn validate_ssrf_url_with_resolver(
    url: &str,
    resolver: fn(&str) -> Result<Vec<IpAddr>, String>,
) -> Result<(), String> {
    let parsed = url::Url::parse(url)
        .map_err(|e| format!("invalid URL: {}", e))?;

    if parsed.scheme() != "https" {
        return Err(format!("PL-DEL-003: URL must use https, got '{}'", parsed.scheme()));
    }

    let host = parsed.host_str()
        .ok_or_else(|| "PL-DEL-003: URL has no host".to_string())?;

    // Reject bare IP addresses in the host (must use a hostname)
    if host.parse::<Ipv4Addr>().is_ok() {
        return Err("PL-DEL-003: bare IPv4 address not permitted".to_string());
    }
    let bracketed = host.trim_matches(|c| c == '[' || c == ']');
    if bracketed.parse::<Ipv6Addr>().is_ok() {
        return Err("PL-DEL-003: bare IPv6 address not permitted".to_string());
    }

    let ips = resolver(host)?;
    for ip in &ips {
        if is_private_ip(ip) {
            return Err(format!("PL-DEL-003: host resolves to private IP {}", ip));
        }
    }

    Ok(())
}

fn resolve_host(host: &str) -> Result<Vec<IpAddr>, String> {
    use std::net::ToSocketAddrs;
    let addrs = (host, 443_u16)
        .to_socket_addrs()
        .map_err(|e| format!("DNS resolution failed for '{}': {}", host, e))?;
    Ok(addrs.map(|a| a.ip()).collect())
}

pub fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_private_v4(v4),
        IpAddr::V6(v6) => is_private_v6(v6),
    }
}

fn is_private_v4(ip: &Ipv4Addr) -> bool {
    let octs = ip.octets();
    ip.is_loopback()               // 127.x
        || octs[0] == 10           // 10.x
        || (octs[0] == 172 && octs[1] >= 16 && octs[1] <= 31)  // 172.16-31
        || (octs[0] == 192 && octs[1] == 168)  // 192.168.x
        || (octs[0] == 169 && octs[1] == 254)  // 169.254.x (link-local)
        || ip.is_unspecified()
}

fn is_private_v6(ip: &Ipv6Addr) -> bool {
    let segs = ip.segments();
    // ::1 loopback
    if *ip == Ipv6Addr::LOCALHOST { return true; }
    // fd00::/8
    if (segs[0] >> 8) == 0xfd { return true; }
    // ::ffff:0:0/96 (IPv4-mapped) — check the mapped IPv4 address
    if segs[0] == 0 && segs[1] == 0 && segs[2] == 0 && segs[3] == 0
        && segs[4] == 0 && segs[5] == 0xffff
    {
        let v4 = Ipv4Addr::new(
            (segs[6] >> 8) as u8, segs[6] as u8,
            (segs[7] >> 8) as u8, segs[7] as u8,
        );
        return is_private_v4(&v4);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_resolver_public(_host: &str) -> Result<Vec<IpAddr>, String> {
        Ok(vec!["203.0.113.1".parse().unwrap()])
    }

    fn mock_resolver_private_10(_host: &str) -> Result<Vec<IpAddr>, String> {
        Ok(vec!["10.0.0.1".parse().unwrap()])
    }

    // ── 22.7.12: SSRF blocklist tests ────────────────────────────────────────

    #[test]
    fn test_rejects_http_scheme() {
        let res = validate_ssrf_url_with_resolver(
            "http://10.0.0.1/oauth/token", mock_resolver_public,
        );
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("PL-DEL-003"));
    }

    #[test]
    fn test_rejects_bare_ipv4_10() {
        let res = validate_ssrf_url_with_resolver(
            "https://10.0.0.1/oauth/token", mock_resolver_public,
        );
        assert!(res.is_err(), "bare 10.x IPv4 must be rejected");
        assert!(res.unwrap_err().contains("PL-DEL-003"));
    }

    #[test]
    fn test_rejects_bare_ipv4_192_168() {
        let res = validate_ssrf_url_with_resolver(
            "https://192.168.1.1/oauth/token", mock_resolver_public,
        );
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("PL-DEL-003"));
    }

    #[test]
    fn test_rejects_bare_ipv4_127() {
        let res = validate_ssrf_url_with_resolver(
            "https://127.0.0.1/oauth/token", mock_resolver_public,
        );
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("PL-DEL-003"));
    }

    #[test]
    fn test_rejects_bare_ipv6_loopback() {
        let res = validate_ssrf_url_with_resolver(
            "https://[::1]/oauth/token", mock_resolver_public,
        );
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("PL-DEL-003"));
    }

    #[test]
    fn test_rejects_bare_ipv6_mapped_10() {
        // ::ffff:10.0.0.1 — IPv4-mapped address in private range
        let res = validate_ssrf_url_with_resolver(
            "https://[::ffff:10.0.0.1]/oauth/token", mock_resolver_public,
        );
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("PL-DEL-003"));
    }

    #[test]
    fn test_rejects_hostname_resolving_to_private() {
        let res = validate_ssrf_url_with_resolver(
            "https://internal.example.com/oauth/token", mock_resolver_private_10,
        );
        assert!(res.is_err());
        assert!(res.unwrap_err().contains("PL-DEL-003"));
    }

    #[test]
    fn test_accepts_valid_https_public_url() {
        let res = validate_ssrf_url_with_resolver(
            "https://gitlab.example.com/oauth/token", mock_resolver_public,
        );
        assert!(res.is_ok(), "public https URL must be accepted: {:?}", res);
    }

    // ── is_private_ip unit tests ──────────────────────────────────────────────

    #[test]
    fn test_is_private_ip_loopback_v4() {
        assert!(is_private_ip(&"127.0.0.1".parse().unwrap()));
    }

    #[test]
    fn test_is_private_ip_10_block() {
        assert!(is_private_ip(&"10.1.2.3".parse().unwrap()));
    }

    #[test]
    fn test_is_private_ip_172_16_block() {
        assert!(is_private_ip(&"172.16.0.1".parse().unwrap()));
        assert!(is_private_ip(&"172.31.255.255".parse().unwrap()));
        assert!(!is_private_ip(&"172.15.0.1".parse().unwrap()));
        assert!(!is_private_ip(&"172.32.0.1".parse().unwrap()));
    }

    #[test]
    fn test_is_private_ip_192_168() {
        assert!(is_private_ip(&"192.168.0.1".parse().unwrap()));
    }

    #[test]
    fn test_is_private_ip_169_254() {
        assert!(is_private_ip(&"169.254.1.1".parse().unwrap()));
    }

    #[test]
    fn test_is_private_ip_public_v4() {
        assert!(!is_private_ip(&"8.8.8.8".parse().unwrap()));
        assert!(!is_private_ip(&"203.0.113.1".parse().unwrap()));
    }

    #[test]
    fn test_is_private_ip_v6_loopback() {
        assert!(is_private_ip(&"::1".parse().unwrap()));
    }

    #[test]
    fn test_is_private_ip_v6_fd00() {
        assert!(is_private_ip(&"fd12:3456::1".parse().unwrap()));
    }

    #[test]
    fn test_is_private_ip_v6_mapped_private() {
        // ::ffff:10.0.0.1
        assert!(is_private_ip(&"::ffff:10.0.0.1".parse().unwrap()));
        assert!(is_private_ip(&"::ffff:192.168.1.1".parse().unwrap()));
    }

    #[test]
    fn test_is_private_ip_v6_public() {
        assert!(!is_private_ip(&"2001:db8::1".parse().unwrap()));
    }

    // ── Additional edge cases ─────────────────────────────────────────────────

    fn mock_resolver_empty(_host: &str) -> Result<Vec<IpAddr>, String> {
        Ok(vec![])
    }

    fn mock_resolver_dns_error(_host: &str) -> Result<Vec<IpAddr>, String> {
        Err("DNS resolution failed for 'example.com': timeout".to_string())
    }

    #[test]
    fn test_rejects_ftp_scheme() {
        let res = validate_ssrf_url_with_resolver(
            "ftp://example.com/file",
            mock_resolver_public,
        );
        assert!(res.is_err(), "ftp:// must be rejected");
        assert!(res.unwrap_err().contains("PL-DEL-003"));
    }

    #[test]
    fn test_rejects_bare_ipv4_172_16() {
        let res = validate_ssrf_url_with_resolver(
            "https://172.16.0.1/path",
            mock_resolver_public,
        );
        assert!(res.is_err(), "bare 172.16.x.x IPv4 must be rejected");
        assert!(res.unwrap_err().contains("PL-DEL-003"));
    }

    #[test]
    fn test_rejects_bare_ipv4_169_254() {
        let res = validate_ssrf_url_with_resolver(
            "https://169.254.1.1/path",
            mock_resolver_public,
        );
        assert!(res.is_err(), "bare 169.254.x.x link-local IPv4 must be rejected");
        assert!(res.unwrap_err().contains("PL-DEL-003"));
    }

    #[test]
    fn test_rejects_bare_ipv4_public() {
        // Policy: all bare IPv4 addresses are rejected regardless of range.
        let res = validate_ssrf_url_with_resolver(
            "https://8.8.8.8/dns",
            mock_resolver_public,
        );
        assert!(res.is_err(), "bare public IPv4 must also be rejected");
        assert!(res.unwrap_err().contains("PL-DEL-003"));
    }

    #[test]
    fn test_accepts_https_when_resolver_returns_no_ips() {
        // Empty IP list means no private address found — should pass.
        let res = validate_ssrf_url_with_resolver(
            "https://example.com/api",
            mock_resolver_empty,
        );
        assert!(res.is_ok(), "empty resolver result must be accepted: {:?}", res);
    }

    #[test]
    fn test_rejects_url_when_resolver_fails() {
        let res = validate_ssrf_url_with_resolver(
            "https://example.com/api",
            mock_resolver_dns_error,
        );
        assert!(res.is_err(), "DNS resolution error must be propagated as Err");
    }

    #[test]
    fn test_is_private_ip_unspecified_v4() {
        assert!(
            is_private_ip(&"0.0.0.0".parse().unwrap()),
            "0.0.0.0 (unspecified) must be private"
        );
    }

    #[test]
    fn test_is_private_ip_v6_mapped_172_16() {
        assert!(
            is_private_ip(&"::ffff:172.16.0.1".parse().unwrap()),
            "::ffff:172.16.0.1 is private (IPv4-mapped 172.16.x)"
        );
    }

    #[test]
    fn test_is_private_ip_v6_mapped_loopback() {
        assert!(
            is_private_ip(&"::ffff:127.0.0.1".parse().unwrap()),
            "::ffff:127.0.0.1 is private (IPv4-mapped loopback)"
        );
    }

    fn mock_resolver_mixed(_host: &str) -> Result<Vec<IpAddr>, String> {
        // One public IP and one private IP — the private one must trigger rejection.
        Ok(vec![
            "203.0.113.1".parse().unwrap(),
            "10.0.0.1".parse().unwrap(),
        ])
    }

    #[test]
    fn test_rejects_when_any_resolved_ip_is_private() {
        let res = validate_ssrf_url_with_resolver(
            "https://mixed.example.com/api",
            mock_resolver_mixed,
        );
        assert!(res.is_err(), "any private IP in resolver results must cause rejection");
        let msg = res.unwrap_err();
        assert!(msg.contains("PL-DEL-003"), "got: {}", msg);
        assert!(msg.contains("10.0.0.1"), "error must name the offending IP, got: {}", msg);
    }

    #[test]
    fn test_rejects_invalid_url_format() {
        let res = validate_ssrf_url_with_resolver("not-a-valid-url", mock_resolver_public);
        assert!(res.is_err(), "malformed URL must be rejected");
        let msg = res.unwrap_err();
        assert!(msg.contains("invalid URL"), "got: {}", msg);
    }

    /// Documents the current implementation boundary: fd00::/8 is blocked
    /// but fc00::/8 (the other half of the ULA fc00::/7 range) is not.
    #[test]
    fn test_is_private_ip_fc00_block_is_not_blocked_by_current_impl() {
        let fc00: IpAddr = "fc00::1".parse().unwrap();
        assert!(
            !is_private_ip(&fc00),
            "fc00::1 is outside the fd00::/8 check — current impl returns false"
        );
    }
}
