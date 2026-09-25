//! Existing MIT Phoenix behavior: RSA over `(request-target) host date`.
use super::FederationError;
use base64::{engine::general_purpose::STANDARD, Engine};
use chrono::{DateTime, Utc};
use rsa::{
    pkcs1::DecodeRsaPrivateKey,
    pkcs1v15::SigningKey,
    pkcs8::{DecodePrivateKey, EncodePublicKey, LineEnding},
    signature::{SignatureEncoding, Signer},
    traits::PublicKeyParts,
    RsaPrivateKey,
};
use sha2::Sha256;
use url::{Host, Url};

pub trait RequestSigner: Send + Sync {
    fn sign_get(
        &self,
        url: &Url,
        now: DateTime<Utc>,
    ) -> Result<Vec<(String, String)>, FederationError>;
}
pub struct RsaHttpSigner {
    key_id: String,
    key: RsaPrivateKey,
}
impl RsaHttpSigner {
    pub fn from_pem(key_id: String, pem: &str) -> Result<Self, FederationError> {
        validate_key_id(&key_id)?;
        if pem.len() > 16 * 1024 {
            return Err(FederationError::InvalidSigningKey);
        }
        // Some Phoenix-generated records have an extra terminal line ending.
        // Normalize only the parsing view; the retained PEM stays byte-identical.
        let parse_pem = pem.trim_end_matches(['\r', '\n']);
        let key = RsaPrivateKey::from_pkcs8_pem(parse_pem)
            .or_else(|_| RsaPrivateKey::from_pkcs1_pem(parse_pem))
            .map_err(|_| FederationError::InvalidSigningKey)?;
        key.validate()
            .map_err(|_| FederationError::InvalidSigningKey)?;
        if key.n().bits() < 2048 || key.n().bits() > 4096 {
            return Err(FederationError::InvalidSigningKey);
        }
        Ok(Self { key_id, key })
    }
    pub fn public_key_pem(&self) -> Result<String, FederationError> {
        self.key
            .to_public_key()
            .to_public_key_pem(LineEnding::LF)
            .map_err(|_| FederationError::InvalidSigningKey)
    }
}

fn validate_key_id(input: &str) -> Result<(), FederationError> {
    if input.len() > 2048 || input.chars().any(char::is_control) || input.contains(['"', '\\']) {
        return Err(FederationError::InvalidSigningKey);
    }
    let id = Url::parse(input).map_err(|_| FederationError::InvalidSigningKey)?;
    // Only the advertised *local signing identity* may use development HTTP.
    // This does not relax HTTPS/public-IP policy for any remote fetch target.
    let local_http = id.scheme() == "http"
        && match id.host() {
            Some(Host::Ipv4(ip)) => ip.is_loopback(),
            Some(Host::Ipv6(ip)) => ip.is_loopback(),
            _ => false,
        };
    if (id.scheme() != "https" && !local_http)
        || id.host_str().is_none()
        || !id.username().is_empty()
        || id.password().is_some()
        || id.query().is_some()
        || id.fragment().is_none_or(str::is_empty)
        || id.port() == Some(0)
        || id.as_str() != input
    {
        return Err(FederationError::InvalidSigningKey);
    }
    Ok(())
}
impl RequestSigner for RsaHttpSigner {
    fn sign_get(
        &self,
        url: &Url,
        now: DateTime<Utc>,
    ) -> Result<Vec<(String, String)>, FederationError> {
        super::transport::safe_url(url.as_str())?;
        let host = url.host_str().ok_or(FederationError::UnsafeUrl)?;
        let date = now.format("%a, %d %b %Y %H:%M:%S GMT").to_string();
        let target = match url.query() {
            Some(query) => format!("{}?{query}", url.path()),
            None => url.path().to_owned(),
        };
        let signing_string = format!("(request-target): get {target}\nhost: {host}\ndate: {date}");
        let signature = SigningKey::<Sha256>::new(self.key.clone()).sign(signing_string.as_bytes());
        let encoded = STANDARD.encode(signature.to_bytes());
        Ok(vec![("host".into(), host.into()), ("date".into(), date),
            ("signature".into(), format!("keyId=\"{}\",algorithm=\"rsa-sha256\",headers=\"(request-target) host date\",signature=\"{encoded}\"", self.key_id))])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{rngs::StdRng, SeedableRng};
    use rsa::{
        pkcs1::EncodeRsaPrivateKey,
        pkcs1v15::{Signature, VerifyingKey},
        pkcs8::EncodePrivateKey,
        signature::Verifier,
    };

    #[test]
    fn http_signing_identity_is_limited_to_canonical_literal_loopback() {
        for key in [
            "https://fediverse.kr/actor#main-key",
            "http://127.0.0.1:12239/actor#main-key",
            "http://[::1]:12239/actor#main-key",
        ] {
            assert!(
                validate_key_id(key).is_ok(),
                "rejected local signing identity"
            );
        }
        for key in [
            "http://fediverse.kr/actor#main-key",
            "http://localhost:12239/actor#main-key",
            "http://192.168.0.1:12239/actor#main-key",
            "http://2130706433:12239/actor#main-key",
            "http://127.0.0.1:0/actor#main-key",
            "http://127.0.0.1:12239/actor#",
            "http://user@127.0.0.1:12239/actor#main-key",
            "https://fediverse.kr/actor?x=1#main-key",
            "https://fediverse.kr/actor#main-key\r\nattack",
        ] {
            assert!(
                validate_key_id(key).is_err(),
                "accepted invalid signing identity"
            );
        }
        assert!(super::super::transport::safe_url("http://127.0.0.1:12239/actor").is_err());
    }

    #[test]
    fn generated_test_key_signs_query_and_signature_verifies() {
        // Deterministic, test-only key; never save or use for a deployed actor.
        let private = RsaPrivateKey::new(&mut StdRng::seed_from_u64(77), 2048).unwrap();
        let pem = private.to_pkcs8_pem(LineEnding::LF).unwrap();
        let signer =
            RsaHttpSigner::from_pem("https://fediverse.kr/actor#main-key".into(), pem.as_str())
                .unwrap();
        let now = DateTime::parse_from_rfc3339("2026-09-12T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let url = Url::parse("https://social.example.com/outbox?page=true&min_id=3").unwrap();
        let headers = signer.sign_get(&url, now).unwrap();
        let date = headers
            .iter()
            .find(|(key, _)| key == "date")
            .unwrap()
            .1
            .clone();
        assert_eq!(date, "Sat, 12 Sep 2026 12:00:00 GMT");
        let header = &headers
            .iter()
            .find(|(key, _)| key == "signature")
            .unwrap()
            .1;
        let encoded = header
            .split("signature=\"")
            .nth(1)
            .unwrap()
            .trim_end_matches('"');
        let bytes = STANDARD.decode(encoded).unwrap();
        let signature = Signature::try_from(bytes.as_slice()).unwrap();
        let payload = format!("(request-target): get /outbox?page=true&min_id=3\nhost: social.example.com\ndate: {date}");
        let verifier = VerifyingKey::<Sha256>::new(private.to_public_key());
        assert!(verifier.verify(payload.as_bytes(), &signature).is_ok());
        assert!(verifier
            .verify(
                payload.replace("min_id=3", "min_id=4").as_bytes(),
                &signature
            )
            .is_err());
        assert!(signer
            .public_key_pem()
            .unwrap()
            .contains("BEGIN PUBLIC KEY"));
        let pkcs1_with_extra_lf = format!(
            "{}\n",
            private.to_pkcs1_pem(LineEnding::LF).unwrap().as_str()
        );
        let phoenix_signer = RsaHttpSigner::from_pem(
            "https://fediverse.kr/actor#main-key".into(),
            &pkcs1_with_extra_lf,
        )
        .unwrap();
        let phoenix_headers = phoenix_signer.sign_get(&url, now).unwrap();
        let phoenix_signature = phoenix_headers
            .iter()
            .find(|(key, _)| key == "signature")
            .unwrap()
            .1
            .split("signature=\"")
            .nth(1)
            .unwrap()
            .trim_end_matches('"');
        let phoenix_bytes = STANDARD.decode(phoenix_signature).unwrap();
        let phoenix_signature = Signature::try_from(phoenix_bytes.as_slice()).unwrap();
        assert!(verifier
            .verify(payload.as_bytes(), &phoenix_signature)
            .is_ok());
        assert!(RsaHttpSigner::from_pem(
            "https://fediverse.kr/actor#key\"injection".into(),
            pem.as_str()
        )
        .is_err());
    }
}
