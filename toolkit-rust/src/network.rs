//! Optional HTTP utilities for tools that accept destinations from callers.
//! Available with the `network` feature; no destination policy is imposed on
//! tools that do not use these utilities.
//!
//! Select a destination policy when constructing a client. Request construction,
//! execution, and redirects all apply that policy. The public policy also checks
//! the DNS answers used by the connection. Neither policy uses environment proxies.
//!
//! ```no_run
//! use nexus_toolkit::network::{Client, DestinationPolicy};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let client = Client::builder(DestinationPolicy::Public).build()?;
//! let response = client.get("https://example.com/data")?.send().await?;
//! # Ok(())
//! # }
//! ```

use {
    reqwest::{
        dns::{Addrs, Name, Resolve, Resolving},
        redirect::Policy,
        IntoUrl,
        Method,
        Request,
        RequestBuilder,
        Response,
        Url,
    },
    std::{
        error::Error as StdError,
        fmt,
        net::{IpAddr, Ipv4Addr, Ipv6Addr},
        sync::Arc,
        time::Duration,
    },
};

/// An invalid destination or a transport failure.
#[derive(Debug)]
pub enum Error {
    /// A request violates the selected destination policy.
    Destination(&'static str),
    /// The underlying HTTP client could not build or execute a request.
    Http(reqwest::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Destination(message) => f.write_str(message),
            Self::Http(error) => error.fmt(f),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Http(error) => Some(error),
            Self::Destination(_) => None,
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(error: reqwest::Error) -> Self {
        Self::Http(error)
    }
}

/// Destinations a tool permits. This policy is chosen by the tool author or its
/// operator; it must not be selected from an untrusted invocation payload.
#[derive(Clone, Debug)]
pub enum DestinationPolicy {
    /// HTTP or HTTPS destinations that resolve exclusively to public addresses.
    Public,
    /// One trusted service, including an internal service if configured.
    /// Scheme, host, and effective port must match; all paths are permitted.
    /// Redirects cannot leave this origin.
    Origin(Url),
}

impl DestinationPolicy {
    /// Check a URL without performing I/O. Clients also apply this automatically.
    pub fn validate(&self, url: &Url) -> Result<(), Error> {
        validate_http_url(url).map_err(Error::Destination)?;
        match self {
            Self::Public => validate_host(
                url.host_str()
                    .ok_or(Error::Destination("URL must have a host"))?,
            )
            .map_err(Error::Destination),
            Self::Origin(origin) => {
                validate_http_url(origin).map_err(Error::Destination)?;
                if url.origin() != origin.origin() {
                    return Err(Error::Destination(
                        "Destination must match the configured service origin",
                    ));
                }
                Ok(())
            }
        }
    }
}

/// An HTTP client whose requests and redirects obey a selected destination policy.
#[derive(Clone, Debug)]
pub struct Client {
    inner: reqwest::Client,
    policy: DestinationPolicy,
}

impl Client {
    /// Configure a client. Defaults to a 30 second timeout and no redirects.
    pub fn builder(policy: DestinationPolicy) -> ClientBuilder {
        ClientBuilder {
            policy,
            timeout: Duration::from_secs(30),
            redirect_limit: 0,
        }
    }

    /// Construct a request after checking its destination. The returned builder
    /// supports the usual reqwest headers, authentication, bodies, and `send`.
    pub fn request(&self, method: Method, url: impl IntoUrl) -> Result<RequestBuilder, Error> {
        let url = url.into_url()?;
        self.policy.validate(&url)?;
        Ok(self.inner.request(method, url))
    }

    /// Construct a GET request with the same checks as [`Self::request`].
    pub fn get(&self, url: impl IntoUrl) -> Result<RequestBuilder, Error> {
        self.request(Method::GET, url)
    }

    /// Execute a built request, checking its final URL even if it was modified
    /// after construction or built by a different client.
    pub async fn execute(&self, request: Request) -> Result<Response, Error> {
        self.policy.validate(request.url())?;
        Ok(self.inner.execute(request).await?)
    }
}

/// Configuration that preserves the chosen policy when building the transport.
#[must_use]
pub struct ClientBuilder {
    policy: DestinationPolicy,
    timeout: Duration,
    redirect_limit: usize,
}

impl ClientBuilder {
    /// Limit the entire request, including its response body.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Permit up to this many redirects. Every destination must satisfy the policy.
    /// The default, zero, returns redirect responses without following them.
    pub fn redirect_limit(mut self, limit: usize) -> Self {
        self.redirect_limit = limit;
        self
    }

    /// Build the transport with the configured policy and limits.
    pub fn build(self) -> Result<Client, Error> {
        if let DestinationPolicy::Origin(origin) = &self.policy {
            self.policy.validate(origin)?;
        }
        let mut builder = reqwest::Client::builder()
            .no_proxy()
            .timeout(self.timeout)
            .redirect(redirect_policy(self.policy.clone(), self.redirect_limit));
        if matches!(self.policy, DestinationPolicy::Public) {
            builder = builder.dns_resolver(Arc::new(PublicResolver));
        }
        Ok(Client {
            inner: builder.build()?,
            policy: self.policy,
        })
    }
}

fn redirect_policy(policy: DestinationPolicy, limit: usize) -> Policy {
    Policy::custom(move |attempt| {
        if limit == 0 {
            return attempt.stop();
        }
        if attempt.previous().len() > limit {
            return attempt.error("Too many redirects");
        }
        if let Err(error) = policy.validate(attempt.url()) {
            return attempt.error(error);
        }
        attempt.follow()
    })
}

/// Check the public destination policy without performing DNS resolution.
/// Use [`Client`] for requests so that DNS answers and redirects are checked too.
pub fn validate_public_url(url: &Url) -> Result<(), Error> {
    DestinationPolicy::Public.validate(url)
}

fn validate_http_url(url: &Url) -> Result<(), &'static str> {
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("URL must use HTTP or HTTPS without embedded credentials");
    }
    Ok(())
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
    // Names without a dot use DNS search domains and can reach metadata services.
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
            checked_addresses(addresses)
        })
    }
}

fn checked_addresses(
    addresses: Vec<std::net::SocketAddr>,
) -> Result<Addrs, Box<dyn StdError + Send + Sync>> {
    if addresses.is_empty() || addresses.iter().any(|address| !is_public_ip(address.ip())) {
        return Err("Destination does not resolve exclusively to public addresses".into());
    }
    // Pass these exact addresses to the connector, without a second lookup.
    Ok(Box::new(addresses.into_iter()))
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

    #[tokio::test]
    async fn public_client_checks_construction_and_execution() {
        let client = Client::builder(DestinationPolicy::Public).build().unwrap();
        assert!(matches!(
            client.get("http://169.254.169.254/"),
            Err(Error::Destination(_))
        ));
        let mut request = client.get("https://example.com/").unwrap().build().unwrap();
        *request.url_mut() = Url::parse("http://127.0.0.1/").unwrap();
        assert!(matches!(
            client.execute(request).await,
            Err(Error::Destination(_))
        ));
    }

    #[tokio::test]
    async fn an_explicit_service_origin_allows_local_requests_and_confines_redirects() {
        let mut server = mockito::Server::new_async().await;
        let mut other = mockito::Server::new_async().await;
        let client = Client::builder(DestinationPolicy::Origin(
            Url::parse(&server.url()).unwrap(),
        ))
        .redirect_limit(1)
        .build()
        .unwrap();
        let start = server
            .mock("GET", "/start")
            .with_status(302)
            .with_header("location", "/result")
            .create_async()
            .await;
        let result = server
            .mock("GET", "/result")
            .with_body("ok")
            .create_async()
            .await;
        assert_eq!(
            client
                .get(format!("{}/start", server.url()))
                .unwrap()
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap(),
            "ok"
        );
        start.assert_async().await;
        result.assert_async().await;

        let escape = server
            .mock("GET", "/escape")
            .with_status(302)
            .with_header("location", &other.url())
            .create_async()
            .await;
        let blocked = other.mock("GET", "/").expect(0).create_async().await;
        assert!(client.get(other.url()).is_err());
        assert!(client
            .get(format!("{}/escape", server.url()))
            .unwrap()
            .send()
            .await
            .unwrap_err()
            .is_redirect());
        escape.assert_async().await;
        blocked.assert_async().await;
    }

    #[test]
    fn origin_policy_checks_scheme_host_and_port_and_rejects_credentials() {
        let policy =
            DestinationPolicy::Origin(Url::parse("https://service.internal/base").unwrap());
        assert!(policy
            .validate(&Url::parse("https://service.internal:443/other").unwrap())
            .is_ok());
        for url in [
            "http://service.internal/",
            "https://service.internal:444/",
            "https://other.internal/",
            "https://user:secret@service.internal/",
        ] {
            assert!(policy.validate(&Url::parse(url).unwrap()).is_err(), "{url}");
        }
        assert!(Client::builder(DestinationPolicy::Origin(
            Url::parse("file:///tmp/data").unwrap()
        ))
        .build()
        .is_err());
    }

    #[tokio::test]
    async fn redirects_are_disabled_by_default_and_limited_when_enabled() {
        let mut server = mockito::Server::new_async().await;
        let redirect = server
            .mock("GET", "/cycle")
            .with_status(302)
            .with_header("location", "/cycle")
            .expect(3)
            .create_async()
            .await;
        let policy = DestinationPolicy::Origin(Url::parse(&server.url()).unwrap());
        let url = format!("{}/cycle", server.url());
        let client = Client::builder(policy.clone()).build().unwrap();
        assert_eq!(
            client.get(&url).unwrap().send().await.unwrap().status(),
            302
        );
        let client = Client::builder(policy).redirect_limit(1).build().unwrap();
        assert!(client
            .get(&url)
            .unwrap()
            .send()
            .await
            .unwrap_err()
            .is_redirect());
        redirect.assert_async().await;
    }

    #[test]
    fn dns_answers_must_all_be_public_and_are_used_without_another_lookup() {
        let public = "8.8.8.8:443".parse().unwrap();
        let private = "169.254.169.254:443".parse().unwrap();
        assert!(checked_addresses(vec![]).is_err());
        assert!(checked_addresses(vec![public, private]).is_err());
        assert_eq!(
            checked_addresses(vec![public]).unwrap().collect::<Vec<_>>(),
            vec![public]
        );
    }

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
        let response = reqwest::Client::builder()
            .no_proxy()
            .redirect(redirect_policy(DestinationPolicy::Public, 3))
            .build()
            .unwrap()
            .get(format!("{}/start", server.url()))
            .send()
            .await;
        assert!(response.unwrap_err().is_redirect());
        redirect.assert_async().await;
    }
}
