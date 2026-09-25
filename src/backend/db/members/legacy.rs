//! First-login continuity with Phoenix AuthController.find_or_create_user.
//! The old service used a normalized, freshly verified handle, not a stored AP
//! actor ID. Resolve that handle only after a valid new public proof, then pin
//! its actor. Neither a handle string nor a legacy row is authentication alone.
use super::*;
use crate::backend::db::schema::member_legacy_claims as claims;

#[derive(Clone, Debug, PartialEq, Queryable, Selectable)]
#[diesel(table_name = claims)]
pub(super) struct Claim {
    pub member_id: Uuid,
    pub handle: String,
    pub actor_url: Option<String>,
    pub claimed_at: Option<DateTime<Utc>>,
}

pub(super) async fn lookup(
    conn: &mut AsyncPgConnection,
    handle: &str,
    actor: &str,
) -> Result<Option<Claim>, FlowError> {
    let found = claims::table
        .filter(
            claims::handle
                .eq(handle.to_lowercase())
                .or(claims::actor_url.eq(actor)),
        )
        .select(Claim::as_select())
        .load::<Claim>(conn)
        .await?;
    if found.len() > 1 {
        // Two historical memberships must not be merged by proving one actor.
        return Err(FlowError::AlreadyLinked);
    }
    Ok(found.into_iter().next())
}

pub(super) fn validate(
    claim: Option<&Claim>,
    actor: &str,
    owners: &[(Uuid, String)],
    session: Option<&AuthenticatedSession>,
) -> Result<(), FlowError> {
    let Some(claim) = claim else { return Ok(()) };
    if claim.actor_url.as_deref().is_some_and(|old| old != actor) {
        // Once bound, a recycled handle cannot claim the old membership.
        return Err(FlowError::LegacyIdentity);
    }
    if owners.iter().any(|(member, _)| *member != claim.member_id)
        || session.is_some_and(|s| s.member.id != claim.member_id)
    {
        return Err(FlowError::AlreadyLinked);
    }
    if claim.actor_url.is_some() && owners.is_empty() && session.is_none() {
        // An intentionally unlinked identity is NOT a recovery backdoor. The
        // owner can reattach it from their remaining, freshly authenticated login.
        return Err(FlowError::LegacyIdentity);
    }
    Ok(())
}

pub(super) async fn pin(
    conn: &mut AsyncPgConnection,
    claim: Option<&Claim>,
    proof: &VerifiedIdentity,
) -> Result<(), FlowError> {
    if let Some(claim) = claim.filter(|c| c.actor_url.is_none()) {
        // Caller holds the member lock and has rechecked this exact claim after
        // waiting. The conditional update is still required; failure rolls back
        // the account attachment, challenge consumption and session as well.
        let changed = diesel::update(
            claims::table
                .find(claim.member_id)
                .filter(claims::actor_url.is_null()),
        )
        .set((
            claims::actor_url.eq(Some(&proof.actor().id)),
            claims::claimed_at.eq(Some(proof.verified_at())),
        ))
        .execute(conn)
        .await;
        match changed {
            Ok(1) => {}
            Ok(_)
            | Err(diesel::result::Error::DatabaseError(
                diesel::result::DatabaseErrorKind::UniqueViolation,
                _,
            )) => return Err(FlowError::AlreadyLinked),
            Err(_) => return Err(FlowError::Unavailable),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
