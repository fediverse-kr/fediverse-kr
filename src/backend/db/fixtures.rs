//! Test-only persistence helpers; there is no application endpoint for these.
use super::{
    schema::{
        member_auth_rate_limits as rates, member_link_challenges as challenges,
        member_sessions as sessions, member_users as users,
    },
    Database,
};
use crate::backend::auth::{self, AuthenticatedMember, SessionGrant};
use chrono::{Duration, Utc};
use diesel::prelude::*;
use diesel_async::{AsyncConnection, RunQueryDsl};
use uuid::Uuid;

pub async fn database() -> Database {
    let url = std::env::var("FEDKR_TEST_DATABASE_URL").expect("explicit disposable test database");
    assert!(
        crate::backend::settings::local_database(&url)
            && url::Url::parse(&url).unwrap().path() == "/fedkr_test",
        "Only the isolated loopback fedkr_test database is allowed"
    );
    Database::migrate(&url).await.unwrap();
    Database::connect(&url).await.unwrap()
}
pub async fn member(db: &Database) -> SessionGrant {
    let mut conn = db.pool.get().await.unwrap();
    (&mut *conn)
        .transaction::<_, diesel::result::Error, _>(async move |conn| {
            let (id, login_id, display_name, created_at) = diesel::insert_into(users::table)
                .values((
                    users::id.eq(Uuid::new_v4()),
                    users::display_name.eq("core test fixture"),
                ))
                .returning((
                    users::id,
                    users::login_id,
                    users::display_name,
                    users::created_at,
                ))
                .get_result(conn)
                .await?;
            let grant = auth::new_session(AuthenticatedMember {
                id,
                login_id,
                display_name,
                created_at,
            });
            diesel::insert_into(sessions::table)
                .values((
                    sessions::id.eq(Uuid::new_v4()),
                    sessions::member_id.eq(id),
                    sessions::token_hash.eq(auth::token_hash(&grant.token).unwrap()),
                    sessions::expires_at.eq(grant.expires_at),
                ))
                .execute(conn)
                .await?;
            Ok(grant)
        })
        .await
        .unwrap()
}
pub async fn session_hash(db: &Database, member: Uuid) -> Vec<u8> {
    let mut conn = db.pool.get().await.unwrap();
    sessions::table
        .filter(sessions::member_id.eq(member))
        .select(sessions::token_hash)
        .first(&mut conn)
        .await
        .unwrap()
}
pub async fn age_session(db: &Database, id: Uuid) {
    let mut conn = db.pool.get().await.unwrap();
    diesel::update(sessions::table.find(id))
        .set((
            sessions::created_at.eq(Utc::now() - Duration::minutes(16)),
            sessions::authenticated_at.eq(Utc::now() - Duration::minutes(17)),
            sessions::federated_authenticated_at.eq(None::<chrono::DateTime<Utc>>),
        ))
        .execute(&mut conn)
        .await
        .unwrap();
}
pub async fn ban(db: &Database, id: Uuid, banned: bool) {
    let mut conn = db.pool.get().await.unwrap();
    diesel::update(users::table.find(id))
        .set(users::is_banned.eq(banned))
        .execute(&mut conn)
        .await
        .unwrap();
}
pub async fn delete_members(db: &Database, ids: &[Uuid]) {
    let mut conn = db.pool.get().await.unwrap();
    diesel::delete(users::table.filter(users::id.eq_any(ids)))
        .execute(&mut conn)
        .await
        .unwrap();
}
pub async fn delete_rate(db: &Database, scope: &str, hash: &[u8]) {
    let mut conn = db.pool.get().await.unwrap();
    diesel::delete(rates::table.find((scope, hash)))
        .execute(&mut conn)
        .await
        .unwrap();
}
pub async fn expire_challenge(db: &Database, id: Uuid) {
    let mut conn = db.pool.get().await.unwrap();
    diesel::update(challenges::table.find(id))
        .set((
            challenges::created_at.eq(Utc::now() - Duration::minutes(31)),
            challenges::expires_at.eq(Utc::now() - Duration::minutes(1)),
        ))
        .execute(&mut conn)
        .await
        .unwrap();
}
pub async fn delete_challenges(db: &Database, handles: &[&str]) {
    let mut conn = db.pool.get().await.unwrap();
    diesel::delete(challenges::table.filter(challenges::handle.eq_any(handles)))
        .execute(&mut conn)
        .await
        .unwrap();
}
