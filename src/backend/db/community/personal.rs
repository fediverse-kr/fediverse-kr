use super::*;
use crate::community::{OwnComment, OwnCommentPage};

#[derive(QueryableByName)]
struct OwnRow {
    #[diesel(sql_type=SqlUuid)]
    id: Uuid,
    #[diesel(sql_type=Nullable<Text>)]
    domain: Option<String>,
    #[diesel(sql_type=Bool)]
    server_available: bool,
    #[diesel(sql_type=Bool)]
    reply: bool,
    #[diesel(sql_type=Text)]
    body: String,
    #[diesel(sql_type=Bool)]
    truncated: bool,
    #[diesel(sql_type=Timestamp)]
    inserted_at: NaiveDateTime,
}
impl Database {
    pub async fn own_comments(
        &self,
        session: &AuthenticatedSession,
        page: u32,
    ) -> Result<OwnCommentPage, Error> {
        domain::page(page)?;
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        (&mut *conn).transaction(async move |conn| {
            authorize_action(conn, session, false).await?;
            // The session is the only owner selector. No actor/linked account
            // joins, no other authors' replies, and no public endpoint reuse.
            let mut rows = diesel::sql_query("SELECT c.id,s.domain,coalesce(NOT s.is_hidden AND NOT s.is_force_hidden,false) AS server_available,c.parent_id IS NOT NULL AS reply,left(c.body,2000) AS body,char_length(c.body)>2000 AS truncated,c.inserted_at FROM community_comments c LEFT JOIN directory_sites s ON s.id=c.server_id WHERE c.user_id=$1 AND c.is_deleted IS NOT TRUE ORDER BY c.inserted_at DESC,c.id DESC LIMIT 21 OFFSET $2")
                .bind::<SqlUuid,_>(session.member.id).bind::<BigInt,_>(i64::from(page)*20)
                .load::<OwnRow>(conn).await?;
            let has_next = rows.len() > 20;
            rows.truncate(20);
            Ok(OwnCommentPage { page, has_next, comments: rows.into_iter().map(|row| OwnComment {
                id: row.id.to_string(), domain: row.domain, server_available: row.server_available,
                reply: row.reply, body: row.body, truncated: row.truncated,
                created_at: row.inserted_at.and_utc().to_rfc3339(),
            }).collect() })
        }).await
    }
}
