//! Phoenix recent_health_checks(server_id, 48), with one visibility snapshot.
use super::*;
use crate::directory::health::{Check, History, HISTORY_LIMIT};

#[derive(QueryableByName)]
struct Row {
    #[diesel(sql_type=Nullable<Timestamptz>)]
    checked_at: Option<DateTime<Utc>>,
    #[diesel(sql_type=Nullable<Bool>)]
    alive: Option<bool>,
    #[diesel(sql_type=Nullable<Integer>)]
    response_ms: Option<i32>,
}
impl Database {
    pub async fn public_health_history(&self, domain: &str) -> Result<Option<History>, StoreError> {
        if domain.is_empty() || domain.len() > 253 || domain.chars().any(char::is_control) {
            return Err(StoreError);
        }
        let mut conn = self.pool.get().await.map_err(|_| StoreError)?;
        // A LEFT JOIN distinguishes a visible site with zero checks from a hidden
        // or nonexistent site. No second visibility query or remote HTTP call.
        let rows = diesel::sql_query(
            "SELECT h.checked_at,h.is_alive AS alive,h.response_time_ms AS response_ms
            FROM directory_sites s LEFT JOIN LATERAL (
                SELECT job_id,checked_at,is_alive,response_time_ms FROM directory_health_checks
                WHERE site_id=s.id ORDER BY checked_at DESC,job_id DESC LIMIT $2
            ) h ON true WHERE s.domain=$1 AND NOT s.is_hidden AND NOT s.is_force_hidden
            ORDER BY h.checked_at,h.job_id",
        )
        .bind::<Text, _>(domain)
        .bind::<BigInt, _>(HISTORY_LIMIT as i64)
        .load::<Row>(&mut conn)
        .await?;
        if rows.is_empty() {
            return Ok(None);
        }
        let mut checks = Vec::new();
        for row in rows {
            let Some(at) = row.checked_at else {
                continue;
            };
            checks.push(Check {
                checked_at: at.to_rfc3339(),
                checked_at_kst: at
                    .with_timezone(&chrono::FixedOffset::east_opt(9 * 3600).expect("KST offset"))
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string(),
                alive: row.alive.ok_or(StoreError)?,
                response_ms: row.response_ms.filter(|ms| *ms >= 0),
            });
        }
        Ok(Some(History {
            preview: false,
            checks,
        }))
    }
}

#[cfg(test)]
mod tests;
