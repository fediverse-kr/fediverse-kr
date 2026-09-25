use super::{http_signature::RequestSigner, FederationError};
use chrono::Utc;
use serde_json::Value;
use std::{
    future::Future,
    net::{IpAddr, SocketAddr, ToSocketAddrs},
    pin::Pin,
    sync::{Arc, OnceLock},
    time::Duration,
};
use url::{Host, Url};

pub type JsonFuture<'a> = Pin<Box<dyn Future<Output = Result<Value, FederationError>> + Send + 'a>>;
/// Injected transport enables complete proof-flow tests without the network.
pub trait FederationTransport: Send + Sync {
    fn get_json<'a>(&'a self, url: &'a Url, signed: bool) -> JsonFuture<'a>;
    /// Read-only JSON POST used by compatibility APIs such as Misskey
    /// `/api/users/show`. Implementations must retain the same URL, timeout,
    /// redirect, DNS and response-body boundaries as `get_json`.
    fn post_json<'a>(&'a self, _url: &'a Url, _body: &'a Value) -> JsonFuture<'a> {
        Box::pin(async { Err(FederationError::Network) })
    }
}
pub const MAX_BODY_BYTES: usize = 256 * 1024;
pub const REQUEST_TIMEOUT_SECONDS: u64 = 10;
const MAX_DNS_LOOKUPS: usize = 8;
static DNS_PERMITS: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();

pub struct HttpTransport {
    signer: Arc<dyn RequestSigner>,
}
impl HttpTransport {
    pub fn new(signer: Arc<dyn RequestSigner>) -> Self {
        Self { signer }
    }
    async fn fetch(
        &self,
        input: &Url,
        method: reqwest::Method,
        signed: bool,
        body: Option<&Value>,
    ) -> Result<Value, FederationError> {
        let url = safe_url(input.as_str())?;
        let client = pinned_client(&url).await?;
        let mut request = client
            .request(method, url.clone())
            .header("user-agent", "fediverse.kr/2 account-verification")
            .header(
                "accept",
                if body.is_some() || !signed {
                    "application/json"
                } else {
                    "application/activity+json, application/ld+json"
                },
            );
        if let Some(body) = body {
            request = request.json(body);
        }
        if signed {
            for (key, value) in self.signer.sign_get(&url, Utc::now())? {
                request = request.header(key, value);
            }
        }
        let mut response = request.send().await.map_err(http_error)?;
        if response.status().is_redirection() {
            return Err(FederationError::RedirectNotAllowed);
        }
        if response.status().as_u16() != 200 {
            return Err(FederationError::HttpStatus(response.status().as_u16()));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_BODY_BYTES as u64)
        {
            return Err(FederationError::BodyTooLarge);
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(http_error)? {
            if bytes.len().saturating_add(chunk.len()) > MAX_BODY_BYTES {
                return Err(FederationError::BodyTooLarge);
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).map_err(|_| FederationError::InvalidJson)
    }
}

/// Shared outbound safety boundary. Each connection pins vetted DNS results.
pub(crate) async fn pinned_client(input: &Url) -> Result<reqwest::Client, FederationError> {
    let url = safe_url(input.as_str())?;
    let host = url.domain().ok_or(FederationError::UnsafeUrl)?;
    let owned_host = host.to_owned();
    let gate = DNS_PERMITS
        .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(MAX_DNS_LOOKUPS)))
        .clone();
    let addresses = tokio::time::timeout(
        Duration::from_secs(3),
        bounded_dns(gate, move || {
            (owned_host.as_str(), 443)
                .to_socket_addrs()
                .map(|addresses| addresses.take(17).collect())
                .map_err(|_| FederationError::Dns)
        }),
    )
    .await
    .map_err(|_| FederationError::Timeout)??;
    validate_addresses(&addresses)?;
    // Pin checked IPs for connection; preserve hostname/SNI/TLS validation.
    // Fresh client prevents pooled connections or another DNS lookup from
    // undermining this check. No redirects and no ambient proxy variables.
    reqwest::Client::builder()
        .https_only(true)
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .resolve_to_addrs(host, &addresses)
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECONDS))
        .build()
        .map_err(|_| FederationError::Network)
}

async fn bounded_dns<F>(
    gate: Arc<tokio::sync::Semaphore>,
    lookup: F,
) -> Result<Vec<SocketAddr>, FederationError>
where
    F: FnOnce() -> Result<Vec<SocketAddr>, FederationError> + Send + 'static,
{
    let permit = gate
        .acquire_owned()
        .await
        .map_err(|_| FederationError::Dns)?;
    tokio::task::spawn_blocking(move || {
        // OS DNS may outlive cancellation. Keep its slot inside the blocking
        // task so repeated timed-out requests cannot grow unlimited lookups.
        let _permit = permit;
        lookup()
    })
    .await
    .map_err(|_| FederationError::Dns)?
}
fn http_error(error: reqwest::Error) -> FederationError {
    if error.is_timeout() {
        FederationError::Timeout
    } else {
        FederationError::Network
    }
}
impl FederationTransport for HttpTransport {
    fn get_json<'a>(&'a self, url: &'a Url, signed: bool) -> JsonFuture<'a> {
        Box::pin(async move {
            // Includes DNS, connection and bounded body/parsing.
            tokio::time::timeout(
                Duration::from_secs(REQUEST_TIMEOUT_SECONDS),
                self.fetch(url, reqwest::Method::GET, signed, None),
            )
            .await
            .map_err(|_| FederationError::Timeout)?
        })
    }
    fn post_json<'a>(&'a self, url: &'a Url, body: &'a Value) -> JsonFuture<'a> {
        Box::pin(async move {
            tokio::time::timeout(
                Duration::from_secs(REQUEST_TIMEOUT_SECONDS),
                self.fetch(url, reqwest::Method::POST, false, Some(body)),
            )
            .await
            .map_err(|_| FederationError::Timeout)?
        })
    }
}

pub fn safe_url(input: &str) -> Result<Url, FederationError> {
    if input.is_empty()
        || input.len() > 2048
        || input.trim() != input
        || input.chars().any(char::is_control)
        || input.contains('\\')
    {
        return Err(FederationError::UnsafeUrl);
    }
    let url = Url::parse(input).map_err(|_| FederationError::UnsafeUrl)?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port_or_known_default() != Some(443)
        || url.fragment().is_some()
    {
        return Err(FederationError::UnsafeUrl);
    }
    let Some(Host::Domain(domain)) = url.host() else {
        return Err(FederationError::UnsafeUrl);
    };
    if domain.len() > 253
        || !domain.contains('.')
        || domain.ends_with('.')
        || domain.split('.').any(|part| {
            part.is_empty()
                || part.len() > 63
                || part.starts_with('-')
                || part.ends_with('-')
                || !part.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        })
        || [
            "localhost",
            "local",
            "internal",
            "home",
            "lan",
            "test",
            "invalid",
            "example",
            "onion",
        ]
        .iter()
        .any(|suffix| domain == *suffix || domain.ends_with(&format!(".{suffix}")))
    {
        return Err(FederationError::UnsafeUrl);
    }
    Ok(url)
}

pub fn validate_addresses(addresses: &[SocketAddr]) -> Result<(), FederationError> {
    if addresses.is_empty()
        || addresses.len() > 16
        || addresses
            .iter()
            .any(|address| address.port() != 443 || !is_public_address(address.ip()))
    {
        return Err(FederationError::UnsafeAddress);
    }
    Ok(())
}
fn is_public_address(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(a == 0
                || a == 10
                || a == 127
                || a >= 224
                || (a == 100 && (64..=127).contains(&b))
                || (a == 169 && b == 254)
                || (a == 172 && (16..=31).contains(&b))
                || (a == 192
                    && (b == 168 || (b == 0 && (c == 0 || c == 2)) || (b == 88 && c == 99)))
                || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
                || (a == 203 && b == 0 && c == 113))
        }
        IpAddr::V6(ip) => {
            let p = ip.segments();
            (p[0] & 0xe000) == 0x2000
                && !(p[0] == 0x2001 && (p[1] < 0x200 || p[1] == 0xdb8))
                && p[0] != 0x2002
                && !(p[0] == 0x3fff && (p[1] & 0xf000) == 0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_ssrf_targets_and_mixed_dns_answers() {
        for url in [
            "http://social.example.com",
            "https://social.example.com:8443",
            "https://a:b@social.example.com",
            "https://127.0.0.1",
            "https://2130706433",
            "https://[::1]",
            "https://printer.local",
            "https://cluster.internal",
            "https://localhost",
            "https://social.example.com/#x",
            "https://social.example.com.",
            "https://social.example.com\\@127.0.0.1",
        ] {
            assert!(safe_url(url).is_err(), "accepted {url}");
        }
        let public = SocketAddr::new("93.184.215.14".parse().unwrap(), 443);
        assert!(validate_addresses(&[public]).is_ok());
        assert!(validate_addresses(&[]).is_err());
        for ip in [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.0.1",
            "192.168.0.25",
            "169.254.169.254",
            "100.64.0.1",
            "192.0.0.9",
            "192.0.2.1",
            "198.18.0.1",
            "198.51.100.1",
            "203.0.113.1",
            "224.0.0.1",
            "::1",
            "::ffff:127.0.0.1",
            "fc00::1",
            "fe80::1",
            "ff02::1",
            "64:ff9b::7f00:1",
            "2001:db8::1",
            "2001::1",
            "2002:7f00:1::",
            "3fff::1",
        ] {
            assert!(
                validate_addresses(&[public, SocketAddr::new(ip.parse().unwrap(), 443)]).is_err(),
                "accepted {ip}"
            );
        }
        assert!(validate_addresses(&[SocketAddr::new(
            "2606:4700:4700::1111".parse().unwrap(),
            443
        )])
        .is_ok());
    }

    #[tokio::test]
    async fn cancelled_dns_keeps_its_concurrency_slot_until_os_work_finishes() {
        let gate = Arc::new(tokio::sync::Semaphore::new(1));
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let request_gate = gate.clone();
        let task = tokio::spawn(async move {
            bounded_dns(request_gate, move || {
                let _ = started_tx.send(());
                let _ = release_rx.recv_timeout(Duration::from_secs(2));
                Ok(vec![])
            })
            .await
        });
        started_rx.await.unwrap();
        task.abort();
        let _ = task.await;
        assert_eq!(gate.available_permits(), 0);
        release_tx.send(()).unwrap();
        let permit = tokio::time::timeout(Duration::from_secs(2), gate.acquire())
            .await
            .unwrap()
            .unwrap();
        drop(permit);
        assert_eq!(gate.available_permits(), 1);
    }

    #[tokio::test]
    async fn local_fetch_targets_fail_before_signing_or_network() {
        struct NeverSign;
        impl RequestSigner for NeverSign {
            fn sign_get(
                &self,
                _: &Url,
                _: chrono::DateTime<chrono::Utc>,
            ) -> Result<Vec<(String, String)>, FederationError> {
                panic!("unsafe target reached signer");
            }
        }
        let transport = HttpTransport::new(Arc::new(NeverSign));
        for value in [
            "http://127.0.0.1:12239/actor",
            "https://127.0.0.1/",
            "https://10.0.0.1/",
            "https://localhost/",
        ] {
            assert!(matches!(
                transport.get_json(&Url::parse(value).unwrap(), true).await,
                Err(FederationError::UnsafeUrl)
            ));
        }
    }
}
