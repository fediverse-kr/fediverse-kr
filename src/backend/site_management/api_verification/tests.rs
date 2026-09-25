use super::*;
use crate::backend::federation::transport::JsonFuture;
use std::sync::Mutex;

pub(crate) struct Actor {
    id: String,
    handle: String,
}
impl FederationTransport for Actor {
    fn get_json<'a>(&'a self, url: &'a url::Url, signed: bool) -> JsonFuture<'a> {
        Box::pin(async move {
            let handle = AccountHandle::parse(&self.handle).unwrap();
            if !signed {
                assert_eq!(url, &handle.lookup_url());
                Ok(
                    json!({"subject":handle.resource(),"links":[{"rel":"self","type":"application/activity+json","href":self.id}]}),
                )
            } else {
                assert_eq!(url.as_str(), self.id);
                Ok(
                    json!({"id":self.id,"type":"Person","preferredUsername":handle.username,"outbox":format!("{}/outbox",self.id)}),
                )
            }
        })
    }
}
pub(crate) struct Responses {
    rows: Mutex<Vec<(Endpoint, u16, Value)>>,
}
impl Responses {
    fn new(rows: Vec<(Endpoint, u16, Value)>) -> Self {
        Self {
            rows: Mutex::new(rows),
        }
    }
}
impl OperatorApi for Responses {
    async fn read(
        &self,
        domain: &str,
        endpoint: Endpoint,
        username: &str,
    ) -> Result<ApiResponse, Error> {
        assert!(domain.ends_with(".example.org"));
        assert_eq!(username, "admin");
        let (expected, status, body) = self.rows.lock().unwrap().remove(0);
        assert_eq!(endpoint, expected);
        Ok(ApiResponse { status, body })
    }
}
fn candidate() -> Candidate {
    Candidate {
        member_id: Uuid::new_v4(),
        session_id: Uuid::new_v4(),
        account_id: Uuid::new_v4(),
        actor_url: "https://social.example.org/users/admin".into(),
        handle: "@admin@social.example.org".into(),
        verified_at: Utc::now(),
        site_id: Uuid::new_v4(),
        domain: "social.example.org".into(),
        revision: 0,
        family: Family::Mastodon,
    }
}
pub(crate) fn actor(c: &Candidate) -> FederationClient<Actor> {
    FederationClient::new(Actor {
        id: c.actor_url.clone(),
        handle: c.handle.clone(),
    })
}
fn contact(c: &Candidate) -> Value {
    json!({"username":"Admin","acct":"admin","uri":c.actor_url})
}
pub(crate) fn v2(c: &Candidate) -> Responses {
    Responses::new(vec![(
        Endpoint::MastodonV2,
        200,
        json!({"contact":{"account":contact(c)}}),
    )])
}

#[tokio::test]
async fn mastodon_v2_and_v1_bind_contact_to_the_verified_actor() {
    let c = candidate();
    assert!(verify(c.clone(), &actor(&c), &v2(&c)).await.is_ok());
    let old = Responses::new(vec![
        (Endpoint::MastodonV2, 404, Value::Null),
        (
            Endpoint::MastodonV1,
            200,
            json!({"contact_account":{"username":"admin","acct":"admin"}}),
        ),
    ]);
    assert!(verify(c.clone(), &actor(&c), &old).await.is_ok());
    // No fallback on contradictory v2 contact, redirects, or downtime.
    for (status, value) in [
        (200, json!({"contact":{"account":null}})),
        (301, Value::Null),
        (503, Value::Null),
    ] {
        let api = Responses::new(vec![(Endpoint::MastodonV2, status, value)]);
        assert!(verify(c.clone(), &actor(&c), &api).await.is_err());
        assert!(api.rows.lock().unwrap().is_empty());
    }
    for bad in [
        json!({"username":"other","acct":"other"}),
        json!({"username":"admin","acct":"admin@foreign.example.org"}),
        json!({"username":"admin","acct":"admin","uri":"https://social.example.org/users/other"}),
    ] {
        assert_eq!(
            mastodon_contact(&bad, "admin", &c.actor_url),
            Err(Error::ApiNotOwner)
        );
    }
}
#[tokio::test]
async fn misskey_requires_local_admin_and_matching_username_and_actor() {
    let mut c = candidate();
    c.family = Family::Misskey;
    let good = json!({"username":"admin","host":null,"isAdmin":true,"uri":null});
    assert!(verify(
        c.clone(),
        &actor(&c),
        &Responses::new(vec![(Endpoint::MisskeyUser, 200, good.clone())])
    )
    .await
    .is_ok());
    for (key, value) in [
        ("isAdmin", json!(false)),
        ("host", json!("remote.example.org")),
        ("username", json!("other")),
        ("uri", json!("https://social.example.org/users/other")),
    ] {
        let mut bad = good.clone();
        bad[key] = value;
        assert!(matches!(
            verify(
                c.clone(),
                &actor(&c),
                &Responses::new(vec![(Endpoint::MisskeyUser, 200, bad)])
            )
            .await,
            Err(Error::ApiNotOwner)
        ));
    }
}
#[tokio::test]
async fn address_alias_is_allowed_but_foreign_sites_and_changed_actor_are_not() {
    let mut c = candidate();
    c.handle = "@admin@address.example.org".into();
    assert!(verify(c.clone(), &actor(&c), &v2(&c)).await.is_ok());
    c.domain = "address.example.org".into();
    assert!(verify(c.clone(), &actor(&c), &v2(&c)).await.is_ok());
    c.domain = "foreign.example.org".into();
    assert!(matches!(
        verify(c.clone(), &actor(&c), &Responses::new(vec![])).await,
        Err(Error::ApiNotOwner)
    ));
    c.domain = "social.example.org".into();
    let changed = FederationClient::new(Actor {
        id: "https://social.example.org/users/recreated".into(),
        handle: c.handle.clone(),
    });
    assert!(matches!(
        verify(c.clone(), &changed, &Responses::new(vec![])).await,
        Err(Error::ApiNotOwner)
    ));
    assert_eq!(Family::parse("custom"), Err(Error::ApiUnsupported));
    assert_eq!(Family::parse("Sharkey"), Ok(Family::Misskey));
    assert_eq!(Family::parse("akkoma"), Ok(Family::Mastodon));
}
