// SPDX-License-Identifier: BUSL-1.1
// Tests for ssrf_validation.rs — extracted to keep the main file under 400 lines.

use super::*;

fn mock_resolver_public(_host: &str) -> Result<Vec<IpAddr>, String> {
    Ok(vec!["203.0.113.1".parse().unwrap()])
}

fn mock_resolver_private_10(_host: &str) -> Result<Vec<IpAddr>, String> {
    Ok(vec!["10.0.0.1".parse().unwrap()])
}

fn mock_resolver_empty(_host: &str) -> Result<Vec<IpAddr>, String> {
    Ok(vec![])
}

fn mock_resolver_dns_error(_host: &str) -> Result<Vec<IpAddr>, String> {
    Err("DNS resolution failed for 'example.com': timeout".to_string())
}

fn mock_resolver_mixed(_host: &str) -> Result<Vec<IpAddr>, String> {
    // One public IP and one private IP — the private one must trigger rejection.
    Ok(vec![
        "203.0.113.1".parse().unwrap(),
        "10.0.0.1".parse().unwrap(),
    ])
}

// ── 22.7.12: validate_ssrf_url tests ─────────────────────────────────────────

#[test]
fn test_rejects_http_scheme() {
    let res = validate_ssrf_url_with_resolver("http://10.0.0.1/oauth/token", mock_resolver_public);
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("PL-DEL-003"));
}

#[test]
fn test_rejects_bare_ipv4_10() {
    let res = validate_ssrf_url_with_resolver("https://10.0.0.1/oauth/token", mock_resolver_public);
    assert!(res.is_err(), "bare 10.x IPv4 must be rejected");
    assert!(res.unwrap_err().contains("PL-DEL-003"));
}

#[test]
fn test_rejects_bare_ipv4_192_168() {
    let res = validate_ssrf_url_with_resolver("https://192.168.1.1/oauth/token", mock_resolver_public);
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("PL-DEL-003"));
}

#[test]
fn test_rejects_bare_ipv4_127() {
    let res = validate_ssrf_url_with_resolver("https://127.0.0.1/oauth/token", mock_resolver_public);
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("PL-DEL-003"));
}

#[test]
fn test_rejects_bare_ipv6_loopback() {
    let res = validate_ssrf_url_with_resolver("https://[::1]/oauth/token", mock_resolver_public);
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("PL-DEL-003"));
}

#[test]
fn test_rejects_bare_ipv6_mapped_10() {
    let res = validate_ssrf_url_with_resolver("https://[::ffff:10.0.0.1]/oauth/token", mock_resolver_public);
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("PL-DEL-003"));
}

#[test]
fn test_rejects_hostname_resolving_to_private() {
    let res = validate_ssrf_url_with_resolver("https://internal.example.com/oauth/token", mock_resolver_private_10);
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("PL-DEL-003"));
}

#[test]
fn test_accepts_valid_https_public_url() {
    let res = validate_ssrf_url_with_resolver("https://gitlab.example.com/oauth/token", mock_resolver_public);
    assert!(res.is_ok(), "public https URL must be accepted: {:?}", res);
}

#[test]
fn test_rejects_ftp_scheme() {
    let res = validate_ssrf_url_with_resolver("ftp://example.com/file", mock_resolver_public);
    assert!(res.is_err(), "ftp:// must be rejected");
    assert!(res.unwrap_err().contains("PL-DEL-003"));
}

#[test]
fn test_rejects_bare_ipv4_172_16() {
    let res = validate_ssrf_url_with_resolver("https://172.16.0.1/path", mock_resolver_public);
    assert!(res.is_err(), "bare 172.16.x.x IPv4 must be rejected");
    assert!(res.unwrap_err().contains("PL-DEL-003"));
}

#[test]
fn test_rejects_bare_ipv4_169_254() {
    let res = validate_ssrf_url_with_resolver("https://169.254.1.1/path", mock_resolver_public);
    assert!(res.is_err(), "bare 169.254.x.x link-local IPv4 must be rejected");
    assert!(res.unwrap_err().contains("PL-DEL-003"));
}

#[test]
fn test_rejects_bare_ipv4_public() {
    let res = validate_ssrf_url_with_resolver("https://8.8.8.8/dns", mock_resolver_public);
    assert!(res.is_err(), "bare public IPv4 must also be rejected");
    assert!(res.unwrap_err().contains("PL-DEL-003"));
}

#[test]
fn test_accepts_https_when_resolver_returns_no_ips() {
    let res = validate_ssrf_url_with_resolver("https://example.com/api", mock_resolver_empty);
    assert!(res.is_ok(), "empty resolver result must be accepted: {:?}", res);
}

#[test]
fn test_rejects_url_when_resolver_fails() {
    let res = validate_ssrf_url_with_resolver("https://example.com/api", mock_resolver_dns_error);
    assert!(res.is_err(), "DNS resolution error must be propagated as Err");
}

#[test]
fn test_rejects_when_any_resolved_ip_is_private() {
    let res = validate_ssrf_url_with_resolver("https://mixed.example.com/api", mock_resolver_mixed);
    assert!(res.is_err(), "any private IP in resolver results must cause rejection");
    let msg = res.unwrap_err();
    assert!(msg.contains("PL-DEL-003"), "got: {}", msg);
    assert!(msg.contains("10.0.0.1"), "error must name the offending IP, got: {}", msg);
}

#[test]
fn test_rejects_invalid_url_format() {
    let res = validate_ssrf_url_with_resolver("not-a-valid-url", mock_resolver_public);
    assert!(res.is_err(), "malformed URL must be rejected");
    assert!(res.unwrap_err().contains("invalid URL"));
}

// ── is_private_ip unit tests ──────────────────────────────────────────────────

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
fn test_is_private_ip_fc00_is_blocked() {
    // fc00::/8 is the lower half of the ULA fc00::/7 range (RFC 4193).
    // Both fc00::/8 and fd00::/8 must be blocked.
    assert!(is_private_ip(&"fc00::1".parse().unwrap()), "fc00::1 must be blocked — it is ULA fc00::/7");
}

#[test]
fn test_is_private_ip_v6_mapped_private() {
    assert!(is_private_ip(&"::ffff:10.0.0.1".parse().unwrap()));
    assert!(is_private_ip(&"::ffff:192.168.1.1".parse().unwrap()));
}

#[test]
fn test_is_private_ip_v6_public() {
    assert!(!is_private_ip(&"2001:db8::1".parse().unwrap()));
}

#[test]
fn test_is_private_ip_unspecified_v4() {
    assert!(is_private_ip(&"0.0.0.0".parse().unwrap()), "0.0.0.0 (unspecified) must be private");
}

#[test]
fn test_is_private_ip_v6_mapped_172_16() {
    assert!(is_private_ip(&"::ffff:172.16.0.1".parse().unwrap()), "::ffff:172.16.0.1 is private");
}

#[test]
fn test_is_private_ip_v6_mapped_loopback() {
    assert!(is_private_ip(&"::ffff:127.0.0.1".parse().unwrap()), "::ffff:127.0.0.1 is private");
}

#[test]
fn test_is_private_url_broadcast() {
    assert!(is_private_url("https://255.255.255.255/"));
}

// ── is_private_host_str tests ────────────────────────────────────────────────

#[test]
fn test_is_private_host_str_localhost() {
    assert!(is_private_host_str("localhost"));
}

#[test]
fn test_is_private_host_str_localhost_localdomain() {
    assert!(is_private_host_str("localhost.localdomain"));
}

#[test]
fn test_is_private_host_str_ip6_localhost() {
    assert!(is_private_host_str("ip6-localhost"));
}

#[test]
fn test_is_private_host_str_ip6_loopback() {
    assert!(is_private_host_str("ip6-loopback"));
}

#[test]
fn test_is_private_host_str_private_ip_literal() {
    assert!(is_private_host_str("192.168.1.1"));
}

#[test]
fn test_is_private_host_str_public_hostname() {
    assert!(!is_private_host_str("example.com"));
}

// ── is_private_url tests ─────────────────────────────────────────────────────

#[test]
fn test_is_private_url_loopback_ipv4() {
    assert!(is_private_url("https://127.0.0.1/path"));
}

#[test]
fn test_is_private_url_loopback_ipv4_port() {
    assert!(is_private_url("https://127.0.0.1:8080/path"));
}

#[test]
fn test_is_private_url_rfc1918_10() {
    assert!(is_private_url("https://10.0.0.1/"));
}

#[test]
fn test_is_private_url_rfc1918_172() {
    assert!(is_private_url("https://172.20.0.1/"));
}

#[test]
fn test_is_private_url_rfc1918_192_168() {
    assert!(is_private_url("https://192.168.1.1/"));
}

#[test]
fn test_is_private_url_aws_metadata() {
    assert!(is_private_url("https://169.254.169.254/latest/meta-data/"));
}

#[test]
fn test_is_private_url_localhost() {
    assert!(is_private_url("https://localhost/"));
}

#[test]
fn test_is_private_url_localhost_localdomain() {
    assert!(is_private_url("https://localhost.localdomain/"));
}

#[test]
fn test_is_private_url_ipv6_loopback() {
    assert!(is_private_url("https://[::1]/"));
}

#[test]
fn test_is_private_url_ipv6_unique_local_fd00() {
    assert!(is_private_url("https://[fd00::1]/"));
}

#[test]
fn test_is_private_url_ipv6_unique_local_fc00() {
    assert!(is_private_url("https://[fc00::1]/"));
}

#[test]
fn test_is_private_url_unparseable() {
    assert!(is_private_url("not-a-url"));
}

#[test]
fn test_is_private_url_file_no_host() {
    assert!(is_private_url("file:///etc/passwd"));
}

#[test]
fn test_is_private_url_public_domain() {
    assert!(!is_private_url("https://example.com/image.png"));
}

#[test]
fn test_is_private_url_cdn() {
    assert!(!is_private_url("https://images.unsplash.com/photo-123"));
}
