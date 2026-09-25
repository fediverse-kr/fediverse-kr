use super::{
    members::authorize_action,
    schema::{
        directory_jobs as jobs, directory_site_registrations as registrations,
        directory_sites as sites, member_linked_accounts as accounts,
    },
    Database,
};
use crate::backend::{
    auth::AuthenticatedSession,
    site_registration::{CheckedSite, Error, MAX_REGISTRATIONS_PER_DAY, MIN_ACCOUNT_AGE_DAYS},
};
use chrono::{DateTime, Duration, Utc};
use diesel::{dsl::exists, prelude::*};
use diesel_async::{AsyncConnection, AsyncPgConnection, RunQueryDsl};
use uuid::Uuid;
impl From<diesel::result::Error> for Error {
    fn from(_: diesel::result::Error) -> Self {
        Self::Unavailable
    }
}

async fn eligible(conn: &mut AsyncPgConnection, s: &AuthenticatedSession) -> Result<(), Error> {
    authorize_action(conn, s, false).await?;
    let at = diesel::select(diesel::dsl::sql::<diesel::sql_types::Timestamptz>("now()"))
        .get_result::<DateTime<Utc>>(conn)
        .await?;
    // A server-registration capability, never a signup/login restriction. The
    // timestamp must come from a still-linked, verified remote identity.
    let old_enough = diesel::select(exists(
        accounts::table
            .filter(accounts::member_id.eq(s.member.id))
            .filter(
                accounts::account_created_at.le(Some(at - Duration::days(MIN_ACCOUNT_AGE_DAYS))),
            )
            .filter(accounts::account_created_at_verified_at.is_not_null()),
    ))
    .get_result::<bool>(conn)
    .await?;
    if !old_enough {
        return Err(Error::Ineligible);
    }
    let count =
        registrations::table
            .filter(registrations::member_id.eq(s.member.id))
            .filter(registrations::created_at.gt(
                diesel::dsl::sql::<diesel::sql_types::Timestamptz>("now()-interval '1 day'"),
            ))
            .count()
            .get_result::<i64>(conn)
            .await?;
    if count >= MAX_REGISTRATIONS_PER_DAY {
        return Err(Error::RateLimited);
    }
    Ok(())
}
async fn absent(conn: &mut AsyncPgConnection, domain: &str) -> Result<(), Error> {
    if diesel::select(exists(sites::table.filter(sites::domain.eq(domain))))
        .get_result::<bool>(conn)
        .await?
    {
        return Err(Error::Existing);
    }
    Ok(())
}
impl Database {
    pub(crate) async fn site_registration_eligible(
        &self,
        s: &AuthenticatedSession,
    ) -> Result<(), Error> {
        let mut c = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *c)
            .transaction(async move |c| eligible(c, s).await)
            .await
    }
    pub(crate) async fn check_site_registration(
        &self,
        s: &AuthenticatedSession,
        domain: &str,
    ) -> Result<(), Error> {
        let mut c = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *c)
            .transaction(async move |c| {
                eligible(c, s).await?;
                absent(c, domain).await
            })
            .await
    }
    pub(crate) async fn register_checked_site(
        &self,
        s: &AuthenticatedSession,
        checked: &CheckedSite,
    ) -> Result<String, Error> {
        let mut c = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *c)
            .transaction(async move |c| {
                eligible(c, s).await?;
                let at =
                    diesel::select(diesel::dsl::sql::<diesel::sql_types::Timestamptz>("now()"))
                        .get_result::<DateTime<Utc>>(c)
                        .await?;
                let observed = checked.observation();
                if at - observed.checked_at > Duration::minutes(2)
                    || observed.checked_at - at > Duration::seconds(30)
                {
                    return Err(Error::NotFederated);
                }
                let id = Uuid::new_v4();
                // UNIQUE serializes different members submitting the same domain.
                // A duplicate never overwrites an owner, hidden flag or prior data.
                let changed = diesel::insert_into(sites::table)
                    .values((sites::id.eq(id), sites::domain.eq(checked.domain())))
                    .on_conflict(sites::domain)
                    .do_nothing()
                    .execute(c)
                    .await?;
                if changed != 1 {
                    return Err(Error::Existing);
                }
                diesel::insert_into(registrations::table)
                    .values((
                        registrations::site_id.eq(id),
                        registrations::member_id.eq(s.member.id),
                    ))
                    .execute(c)
                    .await?;
                super::directory::store_observation(c, Uuid::new_v4(), id, observed, false, false)
                    .await
                    .map_err(|_| Error::Unavailable)?;
                diesel::insert_into(jobs::table)
                    .values((
                        jobs::id.eq(Uuid::new_v4()),
                        jobs::site_id.eq(id),
                        jobs::scheduled_at.eq(diesel::dsl::sql::<diesel::sql_types::Timestamptz>(
                            "to_timestamp(floor(extract(epoch FROM now())/1800)*1800)",
                        )),
                    ))
                    .execute(c)
                    .await?;
                Ok(checked.domain().to_owned())
            })
            .await
    }
}
#[cfg(test)]
mod tests;
