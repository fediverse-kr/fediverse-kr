use super::*;
use crate::backend::{
    auth,
    crawler::{FetchFuture, Reply, SiteTransport},
    db::{
        fixtures,
        schema::{directory_observations as obs, member_sessions},
    },
    flow::{
        self,
        tests::{actor, proof},
    },
    site_registration::inspect,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicUsize, Ordering};
use url::Url;

struct Remote {
    calls: AtomicUsize,
    valid: bool,
}
impl Remote {
    fn good() -> Self {
        Self {
            calls: AtomicUsize::new(0),
            valid: true,
        }
    }
}
impl SiteTransport for Remote {
    fn get<'a>(&'a self, url: &'a Url, _cap: usize) -> FetchFuture<'a> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async move {
            let body=match url.path() {
                "/"=>b"<html>A sample server</html>".to_vec(),
                "/.well-known/nodeinfo"=>serde_json::to_vec(&json!({"links":[{"rel":"http://nodeinfo.diaspora.software/ns/schema/2.1","href":url.join("/nodeinfo/2.1").unwrap().as_str()}]})).unwrap(),
                _=>serde_json::to_vec(&if self.valid {json!({"version":"2.1","software":{"name":"test-social","version":"1.2"},"openRegistrations":false,"usage":{"users":{"total":0}},"metadata":{"nodeName":"테스트 서버","nodeDescription":"<script>not markup</script>"}})}else{json!({"version":"2.1"})}).unwrap(),
            };
            Ok(Reply {
                url: url.clone(),
                status: 200,
                body: Ok(body),
            })
        })
    }
}
struct Fixture {
    db: Database,
    s: AuthenticatedSession,
    token: String,
    actor_id: String,
    sites: Vec<Uuid>,
}
impl Fixture {
    async fn new() -> Self {
        let db = fixtures::database().await;
        let member = fixtures::member(&db).await;
        let s = auth::get_session_details(&db, &member.token)
            .await
            .unwrap()
            .unwrap();
        let mut a = actor(&format!("reg_{}", Uuid::new_v4().simple()));
        a.published = Some(Utc::now() - Duration::days(181));
        let nonce = "d".repeat(64);
        let c = flow::begin_challenge(&db, &a, &nonce, Some(&s))
            .await
            .unwrap();
        let at = db.challenge(c.id).await.unwrap().unwrap().created_at;
        let p = proof(&a, &c.code, at).await;
        let grant = flow::complete_challenge(&db, c.id, &nonce, &c.code, Some(&member.token), &p)
            .await
            .unwrap();
        let s = auth::get_session_details(&db, &grant.token)
            .await
            .unwrap()
            .unwrap();
        Self {
            db,
            s,
            token: grant.token,
            actor_id: a.id,
            sites: vec![],
        }
    }
    async fn checked(&self) -> CheckedSite {
        let host = format!("registration-{}.example.org", Uuid::new_v4().simple());
        inspect(&self.db, &self.s, &host, &Remote::good())
            .await
            .unwrap()
    }
    async fn remember(&mut self, domain: &str) -> Uuid {
        let id = sites::table
            .filter(sites::domain.eq(domain))
            .select(sites::id)
            .first(&mut self.db.pool.get().await.unwrap())
            .await
            .unwrap();
        self.sites.push(id);
        id
    }
    async fn clean(self) {
        let mut c = self.db.pool.get().await.unwrap();
        diesel::delete(sites::table.filter(sites::id.eq_any(self.sites)))
            .execute(&mut c)
            .await
            .unwrap();
        diesel::delete(
            super::super::schema::member_link_challenges::table
                .filter(super::super::schema::member_link_challenges::actor_url.eq(self.actor_id)),
        )
        .execute(&mut c)
        .await
        .unwrap();
        drop(c);
        fixtures::delete_members(&self.db, &[self.s.member.id]).await;
        fixtures::delete_rate(
            &self.db,
            "site_registration",
            &Sha256::digest(self.s.member.id.as_bytes()),
        )
        .await;
    }
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn inspected_registration_is_atomic_observed_not_editorial_and_does_not_grant_ownership() {
    let mut f = Fixture::new().await;
    let checked = f.checked().await;
    let host = checked.domain().to_owned();
    assert!(f.db.public_site(&host).await.unwrap().is_none());
    let preview = checked.preview();
    assert_eq!(preview.users, Some(0));
    assert_eq!(preview.registration_open, Some(false));
    assert_eq!(
        f.db.register_checked_site(&f.s, &checked).await.unwrap(),
        host
    );
    let id = f.remember(&host).await;
    assert!(f.db.owned_sites(&f.s, 0).await.unwrap().sites.is_empty());
    let public = f.db.public_site(&host).await.unwrap().unwrap();
    assert_eq!(public.name, "테스트 서버");
    let mut c = f.db.pool.get().await.unwrap();
    assert_eq!(
        sites::table
            .find(id)
            .select((sites::name, sites::description))
            .first::<(Option<String>, Option<String>)>(&mut c)
            .await
            .unwrap(),
        (None, None)
    );
    assert_eq!(
        obs::table
            .find(id)
            .select((obs::software, obs::user_count, obs::registration_open))
            .first::<(Option<String>, Option<i64>, Option<bool>)>(&mut c)
            .await
            .unwrap(),
        (Some("test-social".into()), Some(0), Some(false))
    );
    assert_eq!(
        registrations::table
            .find(id)
            .select(registrations::member_id)
            .first::<Option<Uuid>>(&mut c)
            .await
            .unwrap(),
        Some(f.s.member.id)
    );
    assert_eq!(
        jobs::table
            .filter(jobs::site_id.eq(id))
            .select(jobs::state)
            .first::<String>(&mut c)
            .await
            .unwrap(),
        "pending"
    );
    drop(c);
    assert_eq!(
        f.db.register_checked_site(&f.s, &checked).await,
        Err(Error::Existing)
    );
    // Restarted pool sees the durable job; no timer or second container needed.
    let restarted = fixtures::database().await;
    assert_eq!(restarted.claim_site().await.unwrap().unwrap().site_id, id);
    f.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn duplicates_race_once_and_hidden_entries_are_never_republished() {
    let mut a = Fixture::new().await;
    let b = Fixture::new().await;
    let checked = a.checked().await;
    let (ra, rb) = tokio::join!(
        a.db.register_checked_site(&a.s, &checked),
        b.db.register_checked_site(&b.s, &checked)
    );
    assert_eq!([&ra, &rb].iter().filter(|r| r.is_ok()).count(), 1);
    assert!([ra, rb].contains(&Err(Error::Existing)));
    let id = a.remember(checked.domain()).await;
    diesel::update(sites::table.find(id))
        .set((
            sites::is_hidden.eq(true),
            sites::is_force_hidden.eq(true),
            sites::is_closed.eq(true),
            sites::name.eq(Some("owner text")),
        ))
        .execute(&mut a.db.pool.get().await.unwrap())
        .await
        .unwrap();
    let remote = Remote::good();
    assert!(matches!(
        inspect(&a.db, &a.s, checked.domain(), &remote).await,
        Err(Error::Existing)
    ));
    assert_eq!(AtomicUsize::load(&remote.calls, Ordering::SeqCst), 0);
    assert_eq!(
        a.db.register_checked_site(&a.s, &checked).await,
        Err(Error::Existing)
    );
    assert!(a.db.public_site(checked.domain()).await.unwrap().is_none());
    assert_eq!(
        sites::table
            .find(id)
            .select((
                sites::name,
                sites::is_hidden,
                sites::is_force_hidden,
                sites::is_closed
            ))
            .first::<(Option<String>, bool, bool, bool)>(&mut a.db.pool.get().await.unwrap())
            .await
            .unwrap(),
        (Some("owner text".into()), true, true, true)
    );
    a.clean().await;
    b.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn registration_checks_age_without_restricting_login_and_rechecks_after_io() {
    let f = Fixture::new().await;
    let checked = f.checked().await;
    let mut c = f.db.pool.get().await.unwrap();
    for published in [
        None,
        Some(Utc::now() - Duration::days(179)),
        Some(Utc::now() + Duration::days(2)),
    ] {
        diesel::update(accounts::table.filter(accounts::member_id.eq(f.s.member.id)))
            .set(accounts::account_created_at.eq(published))
            .execute(&mut c)
            .await
            .unwrap();
        assert_eq!(
            f.db.register_checked_site(&f.s, &checked).await,
            Err(Error::Ineligible)
        );
        assert!(auth::get_session_details(&f.db, &f.token)
            .await
            .unwrap()
            .is_some());
    }
    diesel::update(accounts::table.filter(accounts::member_id.eq(f.s.member.id)))
        .set((
            accounts::account_created_at.eq(Some(Utc::now() - Duration::days(181))),
            accounts::account_created_at_verified_at.eq(None::<DateTime<Utc>>),
        ))
        .execute(&mut c)
        .await
        .unwrap();
    assert_eq!(
        f.db.register_checked_site(&f.s, &checked).await,
        Err(Error::Ineligible)
    );
    diesel::update(accounts::table.filter(accounts::member_id.eq(f.s.member.id)))
        .set(accounts::account_created_at_verified_at.eq(Some(Utc::now())))
        .execute(&mut c)
        .await
        .unwrap();
    fixtures::ban(&f.db, f.s.member.id, true).await;
    assert!(matches!(
        f.db.register_checked_site(&f.s, &checked).await,
        Err(Error::Auth(_))
    ));
    fixtures::ban(&f.db, f.s.member.id, false).await;
    diesel::delete(member_sessions::table.find(f.s.id))
        .execute(&mut c)
        .await
        .unwrap();
    assert!(matches!(
        f.db.register_checked_site(&f.s, &checked).await,
        Err(Error::Auth(_))
    ));
    assert!(f.db.public_site(checked.domain()).await.unwrap().is_none());
    drop(c);
    f.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn unavailable_or_invalid_servers_never_create_directory_or_job_rows() {
    let f = Fixture::new().await;
    let remote = Remote {
        calls: AtomicUsize::new(0),
        valid: false,
    };
    assert!(matches!(
        inspect(&f.db, &f.s, "http://127.0.0.1/", &remote).await,
        Err(Error::InvalidDomain)
    ));
    assert_eq!(AtomicUsize::load(&remote.calls, Ordering::SeqCst), 0);
    let host = format!("registration-{}.example.org", Uuid::new_v4().simple());
    assert!(matches!(
        inspect(&f.db, &f.s, &host, &remote).await,
        Err(Error::NotFederated)
    ));
    assert!(f.db.public_site(&host).await.unwrap().is_none());
    f.clean().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn registration_daily_limit_serializes_and_withdrawal_detaches_attribution_only() {
    let mut f = Fixture::new().await;
    let checked = f.checked().await;
    for _ in 0..9 {
        let id =
            f.db.add_site(&format!(
                "registration-{}.example.org",
                Uuid::new_v4().simple()
            ))
            .await
            .unwrap();
        f.sites.push(id);
        diesel::insert_into(registrations::table)
            .values((
                registrations::site_id.eq(id),
                registrations::member_id.eq(f.s.member.id),
            ))
            .execute(&mut f.db.pool.get().await.unwrap())
            .await
            .unwrap();
    }
    let other = f.checked().await;
    let (ra, rb) = tokio::join!(
        f.db.register_checked_site(&f.s, &checked),
        f.db.register_checked_site(&f.s, &other)
    );
    assert_eq!([&ra, &rb].iter().filter(|r| r.is_ok()).count(), 1);
    let host = ra.as_ref().or(rb.as_ref()).unwrap().clone();
    let id = f.remember(&host).await;
    assert!([ra, rb].contains(&Err(Error::RateLimited)));
    assert_eq!(
        f.db.site_registration_eligible(&f.s).await,
        Err(Error::RateLimited)
    );
    auth::withdraw(&f.db, &f.token, "탈퇴").await.unwrap();
    assert_eq!(
        registrations::table
            .find(id)
            .select(registrations::member_id)
            .first::<Option<Uuid>>(&mut f.db.pool.get().await.unwrap())
            .await
            .unwrap(),
        None
    );
    assert!(f.db.public_site(&host).await.unwrap().is_some());
    f.clean().await;
}
