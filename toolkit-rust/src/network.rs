//! HTTP transport policy for caller supplied public destinations.
use {
    reqwest::{
        dns::{Addrs, Name, Resolve, Resolving},
        redirect::Policy,
        ClientBuilder,
        Url,
    },
    std::{
        net::{IpAddr, Ipv4Addr, Ipv6Addr},
        sync::Arc,
        time::Duration,
    },
};

/// Reject private addresses and names before opening a connection.
pub fn validate_public_url(url: &Url) -> Result<(), &'static str> {
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("URL must use HTTP or HTTPS without embedded credentials");
    }
    validate_host(url.host_str().ok_or("URL must have a host")?)
}

fn validate_host(host: &str) -> Result<(), &'static str> {
    if let Some(ip) = host_as_ip(host) {
        return if is_public_ip(ip) {
            Ok(())
        } else {
            Err("Private destinations are disabled")
        };
    }
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if !host.contains('.')
        || [".internal", ".local", ".localhost", ".arpa"]
            .iter()
            .any(|suffix| host.ends_with(suffix))
    {
        return Err("Private destinations are disabled");
    }
    Ok(())
}

#[derive(Debug)]
struct PublicResolver;

impl Resolve for PublicResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_owned();
        Box::pin(async move {
            validate_host(&host)?;
            let addresses: Vec<_> = tokio::time::timeout(
                Duration::from_secs(5),
                tokio::net::lookup_host((host.as_str(), 0)),
            )
            .await??
            .collect();
            if addresses.is_empty() || addresses.iter().any(|address| !is_public_ip(address.ip())) {
                return Err("Destination does not resolve exclusively to public addresses".into());
            }
            // These exact addresses are passed to the connector. There is no second lookup.
            Ok(Box::new(addresses.into_iter()) as Addrs)
        })
    }
}

/// Build a transport with public DNS resolution and checked redirects.
/// Call `validate_public_url` on the initial URL before creating a request.
pub fn public_client_builder(follow_redirects: bool) -> ClientBuilder {
    let policy = Policy::custom(move |attempt| {
        if !follow_redirects {
            return attempt.stop();
        }
        if attempt.previous().len() >= 3 {
            return attempt.error("Too many redirects");
        }
        if let Err(error) = validate_public_url(attempt.url()) {
            return attempt.error(error);
        }
        attempt.follow()
    });
    reqwest::Client::builder()
        .no_proxy()
        .dns_resolver(Arc::new(PublicResolver))
        .redirect(policy)
}

fn host_as_ip(host: &str) -> Option<IpAddr> {
    host.strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host)
        .parse()
        .ok()
}

/// Whether an address is permitted by the public destination policy.
/// Special purpose ranges are conservatively excluded.
pub fn is_public_ip(addr: IpAddr) -> bool {
    match addr {
        IpAddr::V4(ip) => is_public_ipv4(ip),
        IpAddr::V6(ip) => is_public_ipv6(ip),
    }
}

fn is_public_ipv4(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    !(ip.is_loopback()
        || ip.is_private()
        // 169.254.0.0/16 link-local, where every cloud metadata server lives
        || ip.is_link_local()
        || ip.is_multicast()
        || ip.is_documentation()
        // 0.0.0.0/8 "this network", which includes the unspecified address
        || o[0] == 0
        // 100.64.0.0/10 carrier-grade NAT
        || (o[0] == 100 && (o[1] & 0xc0) == 64)
        // 192.0.0.0/24 IETF protocol assignments
        || (o[0] == 192 && o[1] == 0 && o[2] == 0)
        // 192.88.99.0/24 former 6to4 relay anycast
        || (o[0] == 192 && o[1] == 88 && o[2] == 99)
        // 198.18.0.0/15 benchmarking
        || (o[0] == 198 && (o[1] & 0xfe) == 18)
        // 240.0.0.0/4 reserved, up to and including the broadcast address
        || o[0] >= 240)
}

fn is_public_ipv6(ip: Ipv6Addr) -> bool {
    // A v6 address carrying a v4 one reaches that v4 address, so the v4
    // ranges are what decide.
    if let Some(v4) = embedded_ipv4(ip) {
        return is_public_ipv4(v4);
    }

    let s = ip.segments();
    // Limit native IPv6 to global unicast, excluding special purpose ranges.
    // https://www.iana.org/assignments/iana-ipv6-special-registry/
    if (s[0] & 0xe000) != 0x2000 {
        return false;
    }
    // 2001::/23 protocol assignments, including Teredo and benchmarking
    !((s[0] == 0x2001 && s[1] < 0x0200)
        // 2001:db8::/32 documentation
        || (s[0] == 0x2001 && s[1] == 0x0db8)
        // 3fff::/20 documentation
        || (s[0] == 0x3fff && (s[1] & 0xf000) == 0))
}

/// The v4 address a v6 address stands in for, across the mapped, compatible,
/// 6to4 and NAT64 forms.
fn embedded_ipv4(ip: Ipv6Addr) -> Option<Ipv4Addr> {
    // Both ::ffff:a.b.c.d (mapped) and ::a.b.c.d (compatible).
    if let Some(v4) = ip.to_ipv4() {
        return Some(v4);
    }

    let s = ip.segments();
    let embedded = |hi: u16, lo: u16| {
        Ipv4Addr::new(
            (hi >> 8) as u8,
            (hi & 0xff) as u8,
            (lo >> 8) as u8,
            (lo & 0xff) as u8,
        )
    };

    // 2002::/16 6to4
    if s[0] == 0x2002 {
        return Some(embedded(s[1], s[2]));
    }
    // 64:ff9b::/96 NAT64
    if s[0] == 0x0064 && s[1] == 0xff9b && s[2..6] == [0, 0, 0, 0] {
        return Some(embedded(s[6], s[7]));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_urls_are_rejected_in_all_common_forms() {
        for url in [
            "http://metadata/",
            "http://metadata.google.internal./",
            "http://169.254.169.254/",
            "http://127.1/",
            "http://2130706433/",
            "http://[::1]/",
            "http://[::ffff:127.0.0.1]/",
            "http://10.0.0.1/",
            "file:///etc/passwd",
            "https://user:password@example.com/",
        ] {
            assert!(
                validate_public_url(&Url::parse(url).unwrap()).is_err(),
                "{url}"
            );
        }
        assert!(
            validate_public_url(&Url::parse("https://example.com/path?value=1").unwrap()).is_ok()
        );
    }

    #[test]
    fn special_ipv6_ranges_and_embedded_private_addresses_are_rejected() {
        for address in [
            "fec0::1",
            "64:ff9b:1::a9fe:a9fe",
            "2001::1",
            "2001:2::1",
            "3fff::1",
            "5f00::1",
            "64:ff9b::a9fe:a9fe",
            "2002:a9fe:a9fe::1",
            "::ffff:169.254.169.254",
        ] {
            assert!(!is_public_ip(address.parse().unwrap()), "{address}");
        }
        for address in ["8.8.8.8", "2606:4700::1111", "64:ff9b::808:808"] {
            assert!(is_public_ip(address.parse().unwrap()), "{address}");
        }
    }

    #[tokio::test]
    async fn private_dns_answers_are_rejected() {
        assert!(PublicResolver
            .resolve("localhost".parse().unwrap())
            .await
            .is_err());
    }

    #[tokio::test]
    async fn redirects_cannot_switch_to_a_private_address() {
        let mut server = mockito::Server::new_async().await;
        let redirect = server
            .mock("GET", "/start")
            .with_status(302)
            .with_header("location", "http://169.254.169.254/secret")
            .create_async()
            .await;
        // Only the initial loopback URL bypasses validation to reach the test server.
        let response = public_client_builder(true)
            .build()
            .unwrap()
            .get(format!("{}/start", server.url()))
            .send()
            .await;
        assert!(response.unwrap_err().is_redirect());
        redirect.assert_async().await;
    }
}
