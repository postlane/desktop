// SPDX-License-Identifier: BUSL-1.1

//! SSRF validation for external URLs (22.7.4 Step 3).
//!
//! Validates that a URL does not point to a private/loopback address range.
//! The host is resolved to an IP before checking — string matching alone is
//! insufficient because hostnames can resolve to private IPs.
//!
//! Two entry points:
//!   - `validate_ssrf_url` — full DNS-aware validator (used for outbound API calls)
//!   - `is_private_url` — static string check without DNS (used for image URL guards)

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Returns `Ok(())` if the URL is safe to fetch, `Err(reason)` otherwise.
///
/// Rejects:
/// - `http://` scheme (only `https://` is permitted)
/// - Bare IP address hosts
/// - URLs whose resolved IP falls in a private or loopback range
///
/// Private ranges checked: 127.x, 10.x, 172.16–31.x, 192.168.x, 169.254.x,
/// fc00::/7 ULA (covers fd00::/8 and fc00::/8), ::1, and IPv4-mapped equivalents.
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

/// Returns `true` if the IP falls within a private, loopback, link-local, or reserved range.
pub fn is_private_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_private_v4(v4),
        IpAddr::V6(v6) => is_private_v6(v6),
    }
}

/// Returns `true` if a bare hostname or literal IP string represents a private address.
/// Recognises `localhost` aliases in addition to literal IP ranges.
pub fn is_private_host_str(host: &str) -> bool {
    if matches!(host, "localhost" | "localhost.localdomain" | "ip6-localhost" | "ip6-loopback") {
        return true;
    }
    host.parse::<IpAddr>().is_ok_and(|ip| is_private_ip(&ip))
}

/// Returns `true` if a URL resolves to a private or reserved address via static string check.
/// Returns `true` on parse failure or missing host (fail-closed).
/// For DNS-aware validation use `validate_ssrf_url` instead.
pub fn is_private_url(url: &str) -> bool {
    let parsed = match url::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return true,
    };
    match parsed.host() {
        None => true,
        Some(url::Host::Domain(d)) => is_private_host_str(d),
        Some(url::Host::Ipv4(v4)) => is_private_v4(&v4),
        Some(url::Host::Ipv6(v6)) => is_private_v6(&v6),
    }
}

fn is_private_v4(ip: &Ipv4Addr) -> bool {
    let octs = ip.octets();
    ip.is_loopback()               // 127.x
        || octs[0] == 10           // 10.x
        || (octs[0] == 172 && octs[1] >= 16 && octs[1] <= 31)  // 172.16-31
        || (octs[0] == 192 && octs[1] == 168)  // 192.168.x
        || (octs[0] == 169 && octs[1] == 254)  // 169.254.x (link-local)
        || ip.is_unspecified()     // 0.0.0.0
        || ip.is_broadcast()       // 255.255.255.255
}

fn is_private_v6(ip: &Ipv6Addr) -> bool {
    let segs = ip.segments();
    // ::1 loopback
    if *ip == Ipv6Addr::LOCALHOST { return true; }
    // fc00::/7 — Unique Local Addresses (RFC 4193): covers fc00::/8 and fd00::/8
    if segs[0] & 0xfe00 == 0xfc00 { return true; }
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
#[path = "ssrf_validation_tests.rs"]
mod tests;
