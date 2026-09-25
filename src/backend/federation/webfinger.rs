use super::{
    transport::{safe_url, FederationTransport},
    FederationError,
};
use url::Url;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountHandle {
    pub username: String,
    pub domain: String,
}
impl AccountHandle {
    pub fn parse(input: &str) -> Result<Self, FederationError> {
        let input = input.trim();
        let input = input.strip_prefix('@').unwrap_or(input);
        let (username, domain) = input
            .split_once('@')
            .ok_or(FederationError::InvalidHandle)?;
        if username.is_empty()
            || username.len() > 256
            || !username
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))
            || domain.contains(['@', '/', '?', '#', ':', '\\'])
        {
            return Err(FederationError::InvalidHandle);
        }
        let origin =
            safe_url(&format!("https://{domain}/")).map_err(|_| FederationError::InvalidHandle)?;
        Ok(Self {
            username: username.to_owned(),
            domain: origin.domain().unwrap().to_owned(),
        })
    }
    pub fn resource(&self) -> String {
        format!("acct:{}@{}", self.username, self.domain)
    }
    pub fn display(&self) -> String {
        format!("@{}@{}", self.username, self.domain)
    }
    pub fn lookup_url(&self) -> Url {
        let mut url =
            Url::parse(&format!("https://{}/.well-known/webfinger", self.domain)).unwrap();
        url.query_pairs_mut()
            .append_pair("resource", &self.resource());
        url
    }
}

pub async fn lookup<T: FederationTransport>(
    transport: &T,
    account: &AccountHandle,
) -> Result<Url, FederationError> {
    let json = transport.get_json(&account.lookup_url(), false).await?;
    let subject = json["subject"]
        .as_str()
        .and_then(|s| s.strip_prefix("acct:"))
        .ok_or(FederationError::InvalidWebFinger)?;
    let subject = AccountHandle::parse(subject).map_err(|_| FederationError::InvalidWebFinger)?;
    if subject.domain != account.domain || !subject.username.eq_ignore_ascii_case(&account.username)
    {
        return Err(FederationError::InvalidWebFinger);
    }
    let links = json["links"]
        .as_array()
        .ok_or(FederationError::InvalidWebFinger)?;
    let mut selected = None;
    for link in links.iter().take(64) {
        if link["rel"] != "self" {
            continue;
        }
        let mime = link["type"].as_str().unwrap_or("");
        if mime != "application/activity+json"
            && mime != "application/ld+json; profile=\"https://www.w3.org/ns/activitystreams\""
        {
            continue;
        }
        let href = link["href"]
            .as_str()
            .ok_or(FederationError::InvalidWebFinger)?;
        let url = safe_url(href)?;
        if url.query().is_some() || url.as_str() != href {
            return Err(FederationError::InvalidWebFinger);
        }
        if selected.as_ref().is_some_and(|prior| prior != &url) {
            return Err(FederationError::InvalidWebFinger);
        }
        selected = Some(url);
    }
    selected.ok_or(FederationError::InvalidWebFinger)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_idn_and_rejects_ambiguous_or_local_handles() {
        let handle = AccountHandle::parse(" @패트리샤@냥냥.타워 ").unwrap();
        assert_eq!(handle.username, "패트리샤");
        assert!(handle.domain.starts_with("xn--"));
        assert!(handle
            .lookup_url()
            .query()
            .unwrap()
            .contains("resource=acct%3A"));
        for value in [
            "a",
            "@@a@example.com",
            "a@b@example.com",
            "a@example.com/path",
            "a@127.0.0.1",
            "a@localhost",
            "a@example.com:443",
            "a b@example.com",
            "a@example.com#x",
        ] {
            assert!(AccountHandle::parse(value).is_err(), "accepted {value}");
        }
    }
}
