use super::{members::authorize_action, Database};
use crate::backend::{
    auth::{AuthError, AuthenticatedSession},
    profile_refresh::{Asset, Authority, BatchJob, Download, Downloads, Error, Ticket},
    storage::{self, ObjectStore},
};
use diesel::{
    prelude::*,
    sql_types::{BigInt, Bool, Nullable, Text, Uuid as SqlUuid},
};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use serde_json::{Map, Value};
use uuid::Uuid;
mod batches;

impl From<diesel::result::Error> for Error {
    fn from(_: diesel::result::Error) -> Self {
        Self::Unavailable
    }
}
#[derive(QueryableByName)]
struct Account {
    #[diesel(sql_type=Text)]
    actor_id: String,
    #[diesel(sql_type=Text)]
    handle: String,
}
#[derive(QueryableByName)]
struct Row {
    #[diesel(sql_type=Nullable<SqlUuid>)]
    source: Option<Uuid>,
    #[diesel(sql_type=Nullable<Text>)]
    avatar: Option<String>,
    #[diesel(sql_type=Text)]
    emojis: String,
    #[diesel(sql_type=Nullable<SqlUuid>)]
    request: Option<Uuid>,
    #[diesel(sql_type=Bool)]
    cooling: bool,
}
async fn row(conn: &mut AsyncPgConnection, member: Uuid) -> Result<Row, Error> {
    Ok(diesel::sql_query("SELECT source_account_id AS source,avatar_key AS avatar,emojis::text,request_id AS request,coalesce(requested_at>now()-interval '1 minute',false) AS cooling FROM member_profile_media WHERE member_id=$1")
        .bind::<SqlUuid,_>(member).get_result(conn).await?)
}
async fn account(
    conn: &mut AsyncPgConnection,
    session: &AuthenticatedSession,
    id: Uuid,
) -> Result<Account, Error> {
    authorize_action(conn, session, false).await?;
    diesel::sql_query("SELECT actor_url AS actor_id,handle FROM member_linked_accounts WHERE id=$1 AND member_id=$2")
        .bind::<SqlUuid,_>(id).bind::<SqlUuid,_>(session.member.id).get_result(conn).await.optional()?.ok_or(Error::Auth(AuthError::AccountNotFound))
}
async fn current(conn: &mut AsyncPgConnection, ticket: &Ticket) -> Result<Row, Error> {
    match &ticket.authority {
        Authority::Member(session) => {
            if session.member.id != ticket.member {
                return Err(Error::Superseded);
            }
            let account = account(conn, session, ticket.account.ok_or(Error::Superseded)?).await?;
            if Some(account.actor_id) != ticket.actor_id {
                return Err(Error::ActorChanged);
            }
        }
        Authority::Batch(job) => {
            batches::authorize_job(conn, job).await?;
            let source = batches::source(conn, job.member)
                .await?
                .ok_or(Error::Superseded)?;
            if job.member != ticket.member
                || source.account != ticket.account
                || source.actor_id != ticket.actor_id
                || source.handle != ticket.handle
            {
                return Err(Error::Superseded);
            }
        }
    }
    let row = row(conn, ticket.member).await?;
    if row.request != Some(ticket.request) {
        return Err(Error::Superseded);
    }
    Ok(row)
}
fn keys(row: &Row) -> Result<Vec<String>, Error> {
    let emojis: Map<String, Value> =
        serde_json::from_str(&row.emojis).map_err(|_| Error::Unavailable)?;
    Ok(row
        .avatar
        .iter()
        .cloned()
        .chain(emojis.values().filter_map(Value::as_str).map(str::to_owned))
        .collect())
}

impl Database {
    pub(crate) async fn begin_profile_refresh(
        &self,
        session: &AuthenticatedSession,
        id: Uuid,
        automatic: bool,
    ) -> Result<Option<Ticket>, Error> {
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn).transaction(async |conn| {
            let account = account(conn, session, id).await?;
            diesel::sql_query("INSERT INTO member_profile_media(member_id,avatar_key,emojis) SELECT u.id,l.avatar_key,CASE WHEN jsonb_typeof(l.emojis)='object' THEN l.emojis ELSE '{}'::jsonb END FROM member_users u LEFT JOIN legacy_members l ON l.id=u.id WHERE u.id=$1 ON CONFLICT DO NOTHING")
                .bind::<SqlUuid,_>(session.member.id).execute(conn).await?;
            let before = row(conn, session.member.id).await?;
            // Linking/re-authenticating another identity must not silently change
            // the profile source that the member already chose.
            if automatic && before.source.is_some_and(|source| source != id) { return Ok(None); }
            if before.cooling { return Err(Error::Busy); }
            let request = Uuid::new_v4();
            diesel::sql_query("UPDATE member_profile_media SET request_id=$2,requested_at=now(),refresh_failed=false WHERE member_id=$1")
                .bind::<SqlUuid,_>(session.member.id).bind::<SqlUuid,_>(request).execute(conn).await?;
            Ok(Some(Ticket { authority:Authority::Member(session.clone()), member:session.member.id, account:Some(id), actor_id:Some(account.actor_id), handle:account.handle, request }))
        }).await
    }

    pub(crate) async fn fail_profile_refresh(&self, ticket: &Ticket) -> Result<(), Error> {
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        // This only closes this exact request; it cannot alter files/ownership.
        diesel::sql_query("UPDATE member_profile_media SET request_id=NULL,refresh_failed=true WHERE member_id=$1 AND request_id=$2")
            .bind::<SqlUuid,_>(ticket.member).bind::<SqlUuid,_>(ticket.request).execute(&mut conn).await?;
        Ok(())
    }

    pub(crate) async fn finish_profile_refresh(
        &self,
        ticket: &Ticket,
        store: &ObjectStore,
        files: Downloads,
    ) -> Result<u32, Error> {
        let mutation = store.try_mutation().map_err(|e| match e {
            storage::Error::Busy => Error::Busy,
            _ => Error::Unavailable,
        })?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        // Reserve orphan cleanup BEFORE any immutable publication. Both this
        // phase and the final commit recheck session, link and request identity.
        (&mut *conn).transaction::<_, Error, _>(async |conn| {
            current(conn, ticket).await?;
            diesel::sql_query("LOCK TABLE stored_files IN SHARE ROW EXCLUSIVE MODE").execute(conn).await?;
            for file in std::iter::once(&files.avatar).chain(files.emojis.values()) {
                if let Download::Image(file) = file {
                    #[derive(QueryableByName)] struct Flag { #[diesel(sql_type=Bool)] value: bool }
                    let invalid = diesel::sql_query("SELECT EXISTS(SELECT 1 FROM stored_files WHERE sha256=$1 AND byte_count<>$2 UNION ALL SELECT 1 FROM media_deletions WHERE sha256=$1 AND byte_count<>$2) AS value")
                        .bind::<Text,_>(&file.object.hash).bind::<BigInt,_>(file.object.bytes).get_result::<Flag>(conn).await?.value;
                    if invalid { return Err(Error::Unavailable); }
                    diesel::sql_query("INSERT INTO media_deletions(sha256,byte_count) VALUES($1,$2) ON CONFLICT DO NOTHING")
                        .bind::<Text,_>(&file.object.hash).bind::<BigInt,_>(file.object.bytes).execute(conn).await?;
                }
            }
            Ok(())
        }).await?;
        let result = (&mut *conn).transaction(async |conn| {
            let before = current(conn, ticket).await?;
            let old_keys = keys(&before)?;
            let old_emojis: Map<String, Value> = serde_json::from_str(&before.emojis).map_err(|_| Error::Unavailable)?;
            diesel::sql_query("LOCK TABLE stored_files IN SHARE ROW EXCLUSIVE MODE").execute(conn).await?;
            let avatar = match files.avatar {
                Download::Absent => None,
                Download::Failed => before.avatar,
                Download::Image(asset) => Some(publish(conn, &mutation, "avatars", asset).await?),
            };
            let mut emojis = Map::new();
            for (name, download) in files.emojis {
                let key = match download {
                    Download::Image(asset) => Some(publish(conn, &mutation, "emojis", asset).await?),
                    Download::Failed => old_emojis.get(&name).and_then(Value::as_str).map(str::to_owned),
                    Download::Absent => None,
                };
                if let Some(key) = key { emojis.insert(name, Value::String(key)); }
            }
            diesel::sql_query("UPDATE member_profile_media SET source_account_id=$2,avatar_key=$3,emojis=$4::jsonb,revision=revision+1,request_id=NULL,refreshed_at=now(),refresh_failed=$5 WHERE member_id=$1")
                .bind::<SqlUuid,_>(ticket.member).bind::<Nullable<SqlUuid>,_>(ticket.account).bind::<Nullable<Text>,_>(avatar)
                .bind::<Text,_>(Value::Object(emojis).to_string()).bind::<Bool,_>(files.failed>0).execute(conn).await?;
            super::media_cleanup::retire_keys(conn, &old_keys).await.map_err(|_| Error::Unavailable)?;
            Ok(files.failed)
        }).await;
        drop(mutation);
        result
    }
}

async fn publish(
    conn: &mut AsyncPgConnection,
    mutation: &storage::Mutation,
    prefix: &str,
    asset: Asset,
) -> Result<String, Error> {
    mutation
        .publish(asset.object.clone(), asset.bytes)
        .await
        .map_err(|_| Error::Unavailable)?;
    let key = format!("{prefix}/{}", Uuid::new_v4().simple());
    diesel::sql_query("INSERT INTO stored_files(object_key,sha256,byte_count) VALUES($1,$2,$3)")
        .bind::<Text, _>(&key)
        .bind::<Text, _>(asset.object.hash)
        .bind::<BigInt, _>(asset.object.bytes)
        .execute(conn)
        .await?;
    Ok(key)
}

pub(super) async fn detach(
    conn: &mut AsyncPgConnection,
    member: Uuid,
    account: Uuid,
) -> Result<(), AuthError> {
    let before = diesel::sql_query("SELECT source_account_id AS source,avatar_key AS avatar,emojis::text,request_id AS request,false AS cooling FROM member_profile_media WHERE member_id=$1 AND source_account_id=$2")
        .bind::<SqlUuid,_>(member).bind::<SqlUuid,_>(account).get_result::<Row>(conn).await.optional()?;
    // Invalidate any in-flight request, including one switching source.
    diesel::sql_query("UPDATE member_profile_media SET request_id=NULL WHERE member_id=$1")
        .bind::<SqlUuid, _>(member)
        .execute(conn)
        .await?;
    if let Some(before) = before {
        let keys = keys(&before).map_err(|_| AuthError::Unavailable)?;
        diesel::sql_query("UPDATE member_profile_media SET source_account_id=NULL,avatar_key=NULL,emojis='{}',revision=revision+1,refreshed_at=NULL,refresh_failed=false WHERE member_id=$1")
            .bind::<SqlUuid,_>(member).execute(conn).await?;
        super::media_cleanup::retire_keys(conn, &keys)
            .await
            .map_err(|_| AuthError::Unavailable)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
