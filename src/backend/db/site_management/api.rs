use super::*;
use crate::backend::site_management::api_verification::{Candidate, Family, VerifiedApi};
use chrono::{DateTime, Utc};
use diesel::sql_types::Timestamptz;
#[cfg(test)]
mod tests;

#[derive(QueryableByName)]
struct Linked {
    #[diesel(sql_type=Text)]
    actor_url: String,
    #[diesel(sql_type=Text)]
    handle: String,
    #[diesel(sql_type=Timestamptz)]
    verified_at: DateTime<Utc>,
}
#[derive(QueryableByName)]
struct Software {
    #[diesel(sql_type=Text)]
    software: String,
}
async fn candidate(
    conn: &mut AsyncPgConnection,
    session: &AuthenticatedSession,
    account_id: Uuid,
    domain: &str,
) -> Result<(Candidate, OwnerRow), Error> {
    // Member lock serializes unlink/reverification; do not disclose a private
    // site or take its lock until this account belongs to the caller.
    let linked = diesel::sql_query("SELECT actor_url,handle,verified_at FROM member_linked_accounts WHERE id=$1 AND member_id=$2 AND provider='activitypub_post'")
        .bind::<SqlUuid,_>(account_id).bind::<SqlUuid,_>(session.member.id)
        .get_result::<Linked>(conn).await.optional()?.ok_or(Error::ApiNotOwner)?;
    let handle = crate::backend::federation::webfinger::AccountHandle::parse(&linked.handle)
        .map_err(|_| Error::ApiNotOwner)?;
    let actor = crate::backend::federation::transport::safe_url(&linked.actor_url)
        .map_err(|_| Error::ApiNotOwner)?;
    if domain != handle.domain && Some(domain) != actor.domain() {
        return Err(Error::ApiNotOwner);
    }
    let row = locked_site(conn, domain).await?;
    if row.owner_method.as_deref() == Some("dns") {
        return Err(Error::DnsPriority);
    }
    let software = diesel::sql_query(
        "SELECT coalesce(software,'') AS software FROM directory_observations WHERE site_id=$1",
    )
    .bind::<SqlUuid, _>(row.site_id)
    .get_result::<Software>(conn)
    .await
    .optional()?
    .ok_or(Error::ApiUnsupported)?;
    Ok((
        Candidate {
            member_id: session.member.id,
            session_id: session.id,
            account_id,
            actor_url: linked.actor_url,
            handle: linked.handle,
            verified_at: linked.verified_at,
            site_id: row.site_id,
            domain: domain.to_owned(),
            revision: row.revision,
            family: Family::parse(&software.software)?,
        },
        row,
    ))
}
impl Database {
    pub(crate) async fn api_owner_candidate(
        &self,
        session: &AuthenticatedSession,
        account_id: Uuid,
        domain: &str,
    ) -> Result<Candidate, Error> {
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn)
            .transaction(async move |conn| {
                authorize_action(conn, session, true).await?;
                Ok(candidate(conn, session, account_id, domain).await?.0)
            })
            .await
    }
    pub(crate) async fn claim_site_api(
        &self,
        session: &AuthenticatedSession,
        proof: &VerifiedApi,
    ) -> Result<(), Error> {
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn).transaction(async move |conn| {
            authorize_action(conn, session, true).await?;
            let prior = proof.candidate();
            if prior.member_id != session.member.id || prior.session_id != session.id { return Err(Error::ApiNotOwner); }
            let (current, row) = candidate(conn, session, prior.account_id, &prior.domain).await?;
            if current != *prior { return Err(Error::Conflict); }
            audit(conn, &row, session.member.id, "api_claim").await?;
            diesel::sql_query("UPDATE directory_site_details SET owner_id=$2,owner_method='api',revision=revision+1,updated_at=now() WHERE site_id=$1")
                .bind::<SqlUuid,_>(row.site_id).bind::<SqlUuid,_>(session.member.id).execute(conn).await?;
            // Pending DNS proofs retain priority and may supersede this claim.
            Ok(())
        }).await
    }
}
