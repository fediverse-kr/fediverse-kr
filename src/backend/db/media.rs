use super::{Database, StoreError};
use crate::backend::{
    auth,
    media::{Error, Target},
    storage::ObjectRef,
};
use crate::membership::ProfileMedia;
use diesel::{
    prelude::*,
    sql_types::{Array, BigInt, Binary, Bool, Nullable, Text},
};
use diesel_async::RunQueryDsl;

// A live row (including an explicitly removed avatar) overrides the immutable
// legacy fallback. Never use COALESCE to resurrect a removed old picture.
const PROFILE: &str = "SELECT avatar_key,emojis FROM member_profile_media WHERE member_id=u.id UNION ALL SELECT avatar_key,emojis FROM legacy_members WHERE id=u.id AND NOT EXISTS(SELECT 1 FROM member_profile_media WHERE member_id=u.id)";

#[derive(QueryableByName)]
struct Row {
    #[diesel(sql_type=Text)]
    hash: String,
    #[diesel(sql_type=BigInt)]
    bytes: i64,
}
impl Database {
    pub async fn media_reference(
        &self,
        target: &Target,
        token: Option<&str>,
    ) -> Result<Option<ObjectRef>, Error> {
        let mut conn = self.pool.get().await.map_err(|_| Error::Unavailable)?;
        let row=match target {
   Target::Software(name)=>diesel::sql_query("SELECT f.sha256 AS hash,f.byte_count AS bytes FROM catalog_software s JOIN stored_files f ON f.object_key=s.logo_key WHERE s.name=$1 AND (s.logo_key LIKE 'software-logos/%' OR s.logo_key LIKE 'software/%')").bind::<Text,_>(name).get_result::<Row>(&mut conn).await.optional(),
   Target::Site(domain)=>diesel::sql_query("SELECT f.sha256 AS hash,f.byte_count AS bytes FROM directory_sites s JOIN legacy_sites l ON l.id=s.id JOIN stored_files f ON f.object_key=l.favicon_key WHERE s.domain=$1 AND NOT s.is_hidden AND NOT s.is_force_hidden AND l.favicon_key LIKE 'favicons/%'").bind::<Text,_>(domain).get_result::<Row>(&mut conn).await.optional(),
   Target::OwnAvatar|Target::OwnEmoji(_)=>{
    let hash=token.and_then(auth::token_hash).ok_or(Error::Unauthorized)?;
    // Distinguish an expired session from an authenticated account without media.
    #[derive(QueryableByName)] struct Valid {#[diesel(sql_type=Bool)] value:bool}
    if !diesel::sql_query("SELECT EXISTS(SELECT 1 FROM member_sessions s JOIN member_users u ON u.id=s.member_id WHERE s.token_hash=$1 AND s.expires_at>now() AND NOT u.is_banned) AS value").bind::<Binary,_>(&hash).get_result::<Valid>(&mut conn).await.map_err(|_|Error::Unavailable)?.value{return Err(Error::Unauthorized)}
    let (select,name,prefix)=match target {Target::OwnEmoji(name)=>("l.emojis->>$2",name.as_str(),"emojis/%"),_=>("l.avatar_key","","avatars/%")};
    diesel::sql_query(format!("SELECT f.sha256 AS hash,f.byte_count AS bytes FROM member_sessions s JOIN member_users u ON u.id=s.member_id JOIN LATERAL ({PROFILE}) l ON true JOIN stored_files f ON f.object_key={select} WHERE s.token_hash=$1 AND $2::text IS NOT NULL AND ({select}) LIKE $3 AND s.expires_at>now() AND NOT u.is_banned"))
      .bind::<Binary,_>(&hash).bind::<Text,_>(name).bind::<Text,_>(prefix).get_result::<Row>(&mut conn).await.optional()
   }
  }.map_err(|_|Error::Unavailable)?;
        row.map(|r| {
            let object = ObjectRef {
                hash: r.hash,
                bytes: r.bytes,
            };
            if object.valid() {
                Ok(object)
            } else {
                Err(Error::Unavailable)
            }
        })
        .transpose()
    }
    pub async fn own_profile_media(&self, token: &str) -> Result<ProfileMedia, auth::AuthError> {
        let hash = auth::token_hash(token).ok_or(auth::AuthError::Unauthenticated)?;
        #[derive(QueryableByName)]
        struct Profile {
            #[diesel(sql_type=Bool)]
            avatar_available: bool,
            #[diesel(sql_type=Array<Text>)]
            emojis: Vec<String>,
            #[diesel(sql_type=BigInt)]
            revision: i64,
            #[diesel(sql_type=Bool)]
            refreshing: bool,
            #[diesel(sql_type=Bool)]
            refresh_failed: bool,
            #[diesel(sql_type=Nullable<Text>)]
            source_account_id: Option<String>,
        }
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|_| auth::AuthError::Unavailable)?;
        let row=diesel::sql_query(format!("SELECT EXISTS(SELECT 1 FROM stored_files f WHERE f.object_key=l.avatar_key AND l.avatar_key LIKE 'avatars/%') AS avatar_available,ARRAY(SELECT e.key FROM jsonb_each_text(CASE WHEN jsonb_typeof(l.emojis)='object' THEN l.emojis ELSE '{{}}'::jsonb END) e JOIN stored_files f ON f.object_key=e.value AND e.value LIKE 'emojis/%' WHERE octet_length(e.key)<=1024 ORDER BY e.key LIMIT 32) AS emojis,coalesce(p.revision,0) AS revision,coalesce(p.request_id IS NOT NULL AND p.requested_at>now()-interval '2 minutes',false) AS refreshing,coalesce(p.refresh_failed OR (p.request_id IS NOT NULL AND p.requested_at<=now()-interval '2 minutes'),false) AS refresh_failed,p.source_account_id::text FROM member_sessions s JOIN member_users u ON u.id=s.member_id LEFT JOIN LATERAL ({PROFILE}) l ON true LEFT JOIN member_profile_media p ON p.member_id=u.id WHERE s.token_hash=$1 AND s.expires_at>now() AND NOT u.is_banned"))
   .bind::<Binary,_>(hash).get_result::<Profile>(&mut conn).await.optional().map_err(|_|auth::AuthError::Unavailable)?.ok_or(auth::AuthError::Unauthenticated)?;
        Ok(ProfileMedia {
            revision: row.revision,
            refreshing: row.refreshing,
            refresh_failed: row.refresh_failed,
            source_account_id: row.source_account_id,
            avatar_available: row.avatar_available,
            emojis: row
                .emojis
                .into_iter()
                .filter(|n| crate::backend::media::emoji_name(n))
                .collect(),
        })
    }
}

impl From<StoreError> for Error {
    fn from(_: StoreError) -> Self {
        Self::Unavailable
    }
}
#[cfg(test)]
mod tests;
