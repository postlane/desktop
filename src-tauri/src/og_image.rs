// SPDX-License-Identifier: BUSL-1.1

use std::net::IpAddr;
use std::time::Duration;
use url::Url;

fn check_ip_safe(ip: IpAddr) -> Result<(), String> {
    match ip {
        IpAddr::V4(v4) => {
            if v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified() {
                return Err(format!("IP address {} is in a private or reserved range", ip));
            }
        }
        IpAddr::V6(v6) => {
            let bytes = v6.octets();
            let is_unique_local = bytes[0] & 0xfe == 0xfc;
            let is_link_local = bytes[0] == 0xfe && (bytes[1] & 0xc0) == 0x80;
            if v6.is_loopback() || v6.is_unspecified() || is_unique_local || is_link_local {
                return Err(format!("IPv6 address {} is in a private or reserved range", ip));
            }
        }
    }
    Ok(())
}

async fn check_ssrf(url: &str) -> Result<(), String> {
    let parsed = Url::parse(url).map_err(|e| format!("Invalid URL: {}", e))?;
    if parsed.scheme() != "https" {
        return Err(format!("URL scheme '{}' is not permitted; only https is allowed", parsed.scheme()));
    }
    let host = parsed.host_str().ok_or_else(|| "URL has no host".to_string())?;
    if let Ok(ip) = host.parse::<IpAddr>() {
        return check_ip_safe(ip);
    }
    let target = format!("{}:443", host);
    let addrs = tokio::time::timeout(Duration::from_secs(2), tokio::net::lookup_host(&target))
        .await
        .map_err(|_| format!("DNS timeout resolving '{}'", host))?
        .map_err(|e| format!("DNS resolution failed for '{}': {}", host, e))?;
    let addrs: Vec<_> = addrs.collect();
    if addrs.is_empty() {
        return Err(format!("DNS returned no addresses for '{}'", host));
    }
    for addr in &addrs {
        check_ip_safe(addr.ip())?;
    }
    Ok(())
}

fn extract_og_image(html: &str) -> Result<Option<String>, String> {
    let pat1 = regex::Regex::new(r#"<meta[^>]+property="og:image"[^>]+content="([^"]+)""#)
        .expect("static regex");
    let pat2 = regex::Regex::new(r#"<meta[^>]+content="([^"]+)"[^>]+property="og:image""#)
        .expect("static regex");
    let og_url = pat1.captures(html)
        .or_else(|| pat2.captures(html))
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_owned());
    let Some(u) = og_url else { return Ok(None) };
    if u.starts_with("javascript:") {
        return Err("OG image URL uses javascript: scheme".to_string());
    }
    if u.starts_with("data:") {
        return Err("OG image URL uses data: scheme".to_string());
    }
    if !u.starts_with("https://") {
        return Ok(None);
    }
    Ok(Some(u))
}

async fn fetch_og_via_client(
    start_url: &str,
    client: &reqwest::Client,
) -> Result<Option<String>, String> {
    let mut url = start_url.to_owned();
    for _ in 0..5_usize {
        let resp = match client.get(&url).send().await {
            Ok(r) => r,
            Err(e) if e.is_timeout() || e.is_connect() => return Ok(None),
            Err(e) => return Err(format!("Request failed: {}", e)),
        };
        if resp.status().is_redirection() {
            let location = resp.headers().get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            let Some(next) = location else { return Ok(None) };
            let base = Url::parse(&url).map_err(|e| format!("Invalid URL: {}", e))?;
            let resolved = base.join(&next)
                .map_err(|e| format!("Invalid redirect URL: {}", e))?;
            // Reject cross-host redirects — DNS rebinding can make a hostname resolve
            // to a private IP on the second lookup even if it was public on the first.
            if resolved.host_str() != base.host_str() {
                return Ok(None);
            }
            url = resolved.to_string();
            continue;
        }
        if !resp.status().is_success() { return Ok(None); }
        return match resp.text().await {
            Ok(html) => extract_og_image(&html),
            Err(_) => Ok(None),
        };
    }
    Ok(None)
}

#[tauri::command]
pub async fn validate_url_safe(url: String) -> Result<(), String> {
    check_ssrf(&url).await
}

#[tauri::command]
pub async fn fetch_og_image(url: String) -> Result<Option<String>, String> {
    check_ssrf(&url).await?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent("Mozilla/5.0 (compatible; Postlane/1.0; +https://postlane.dev)")
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;
    fetch_og_via_client(&url, &client).await
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::prelude::*;

    fn short_client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(Duration::from_millis(300))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("client")
    }

    // ── fetch_og_image — SSRF rejection (tests 1–8, 15–16) ──────────────────

    #[tokio::test]
    async fn test_fetch_og_image_rejects_http_scheme() {
        let r = fetch_og_image("http://example.com".to_string()).await;
        assert!(r.is_err(), "expected Err for http:// scheme");
    }

    #[tokio::test]
    async fn test_fetch_og_image_rejects_javascript_scheme() {
        let r = fetch_og_image("javascript:alert(1)".to_string()).await;
        assert!(r.is_err(), "expected Err for javascript: scheme");
    }

    #[tokio::test]
    async fn test_fetch_og_image_rejects_private_ipv4_10() {
        let r = fetch_og_image("https://10.0.0.1/".to_string()).await;
        assert!(r.is_err(), "expected Err for 10.x private IP");
    }

    #[tokio::test]
    async fn test_fetch_og_image_rejects_private_ipv4_172_16() {
        let r = fetch_og_image("https://172.16.0.1/".to_string()).await;
        assert!(r.is_err(), "expected Err for 172.16.x private IP");
    }

    #[tokio::test]
    async fn test_fetch_og_image_rejects_private_ipv4_192_168() {
        let r = fetch_og_image("https://192.168.1.1/".to_string()).await;
        assert!(r.is_err(), "expected Err for 192.168.x private IP");
    }

    #[tokio::test]
    async fn test_fetch_og_image_rejects_localhost_ipv4() {
        let r = fetch_og_image("https://127.0.0.1/".to_string()).await;
        assert!(r.is_err(), "expected Err for 127.0.0.1 loopback");
    }

    #[tokio::test]
    async fn test_fetch_og_image_rejects_link_local_ipv4() {
        let r = fetch_og_image("https://169.254.169.254/".to_string()).await;
        assert!(r.is_err(), "expected Err for 169.254.x link-local");
    }

    #[tokio::test]
    async fn test_fetch_og_image_rejects_localhost_ipv6_bracket() {
        let r = fetch_og_image("https://[::1]/path".to_string()).await;
        assert!(r.is_err(), "expected Err for [::1] IPv6 loopback");
    }

    #[tokio::test]
    async fn test_fetch_og_image_rejects_unique_local_ipv6_bracket() {
        let r = fetch_og_image("https://[fd00::1]/path".to_string()).await;
        assert!(r.is_err(), "expected Err for [fd00::1] IPv6 unique-local");
    }

    #[tokio::test]
    async fn test_fetch_og_image_rejects_domain_resolving_to_private_ip() {
        let r = fetch_og_image("https://localhost/".to_string()).await;
        assert!(r.is_err(), "expected Err — localhost resolves to loopback");
    }

    // ── fetch_og_image — redirect SSRF (test 9) ─────────────────────────────

    #[tokio::test]
    async fn test_fetch_og_image_returns_none_on_redirect_to_private_ip() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/");
            then.status(301).header("Location", "https://127.0.0.1/");
        });
        let client = short_client();
        let r = fetch_og_via_client(&server.url("/"), &client).await;
        assert_eq!(r, Ok(None), "redirect to private IP must yield Ok(None)");
    }

    #[tokio::test]
    async fn test_fetch_og_via_client_rejects_cross_host_redirect() {
        let server = MockServer::start();
        // Redirect to a different hostname — cross-host redirect must be blocked
        // to prevent DNS rebinding: the original host resolves at validation time,
        // but a different host could resolve to a private IP on the next lookup.
        server.mock(|when, then| {
            when.method(GET).path("/");
            then.status(301).header("Location", "http://other.example.internal/");
        });
        let client = short_client();
        // server.url("/") is http://127.0.0.1:PORT/ — host "127.0.0.1"
        // redirect host is "other.example.internal" — different, must be rejected
        let r = fetch_og_via_client(&server.url("/"), &client).await;
        assert_eq!(r, Ok(None), "cross-host redirect must yield Ok(None)");
    }

    // ── fetch_og_image — OG parsing (tests 10–14) ───────────────────────────

    #[tokio::test]
    async fn test_fetch_og_image_returns_some_for_valid_og_tag() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/page");
            then.status(200).header("Content-Type", "text/html").body(
                r#"<html><head><meta property="og:image" content="https://example.com/og.png"/></head></html>"#,
            );
        });
        let client = short_client();
        let r = fetch_og_via_client(&server.url("/page"), &client).await;
        assert_eq!(r, Ok(Some("https://example.com/og.png".to_string())));
    }

    #[tokio::test]
    async fn test_fetch_og_image_returns_none_when_no_og_tag() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/");
            then.status(200).header("Content-Type", "text/html").body("<html><head></head></html>");
        });
        let client = short_client();
        let r = fetch_og_via_client(&server.url("/"), &client).await;
        assert_eq!(r, Ok(None));
    }

    #[tokio::test]
    async fn test_fetch_og_image_returns_none_on_timeout() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        std::thread::spawn(move || {
            if let Ok((_stream, _)) = listener.accept() {
                std::thread::sleep(Duration::from_secs(30));
            }
        });
        let client = short_client();
        let r = fetch_og_via_client(&format!("http://127.0.0.1:{}/", port), &client).await;
        assert_eq!(r, Ok(None), "timeout must yield Ok(None)");
    }

    #[tokio::test]
    async fn test_fetch_og_image_rejects_javascript_in_og_content() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/");
            then.status(200).body(r#"<meta property="og:image" content="javascript:alert(1)"/>"#);
        });
        let client = short_client();
        let r = fetch_og_via_client(&server.url("/"), &client).await;
        assert!(r.is_err(), "javascript: OG content must be Err");
    }

    #[tokio::test]
    async fn test_fetch_og_image_rejects_data_url_in_og_content() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(GET).path("/");
            then.status(200).body(r#"<meta property="og:image" content="data:image/png;base64,abc"/>"#);
        });
        let client = short_client();
        let r = fetch_og_via_client(&server.url("/"), &client).await;
        assert!(r.is_err(), "data: OG content must be Err");
    }

    // ── validate_url_safe (7 tests) ──────────────────────────────────────────

    #[tokio::test]
    async fn test_validate_url_safe_accepts_valid_https_ip() {
        let r = validate_url_safe("https://1.1.1.1/".to_string()).await;
        assert!(r.is_ok(), "public IP must be Ok(())");
    }

    #[tokio::test]
    async fn test_validate_url_safe_rejects_http_scheme() {
        let r = validate_url_safe("http://example.com".to_string()).await;
        assert!(r.is_err(), "http:// must be Err");
    }

    #[tokio::test]
    async fn test_validate_url_safe_rejects_javascript_scheme() {
        let r = validate_url_safe("javascript:alert(1)".to_string()).await;
        assert!(r.is_err(), "javascript: must be Err");
    }

    #[tokio::test]
    async fn test_validate_url_safe_rejects_private_ip_literal() {
        let r = validate_url_safe("https://10.0.0.1/".to_string()).await;
        assert!(r.is_err(), "private IP literal must be Err");
    }

    #[tokio::test]
    async fn test_validate_url_safe_rejects_localhost_domain() {
        let r = validate_url_safe("https://localhost/".to_string()).await;
        assert!(r.is_err(), "localhost resolves to loopback, must be Err");
    }

    #[tokio::test]
    async fn test_validate_url_safe_rejects_ipv6_loopback_bracket() {
        let r = validate_url_safe("https://[::1]/path".to_string()).await;
        assert!(r.is_err(), "[::1] must be Err");
    }

    #[tokio::test]
    async fn test_validate_url_safe_rejects_ipv6_unique_local_bracket() {
        let r = validate_url_safe("https://[fd00::1]/path".to_string()).await;
        assert!(r.is_err(), "[fd00::1] must be Err");
    }
}
