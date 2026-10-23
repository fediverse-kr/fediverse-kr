use chrono::{DateTime, Utc};
use fedkr_federation::publication::PreparedActivity;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let published = DateTime::parse_from_rfc3339("2026-10-23T01:02:03Z")?.with_timezone(&Utc);
    let activity = PreparedActivity::public_note(
        "https://bot.example.org/activities/synthetic-status-1",
        "https://bot.example.org/notes/synthetic-status-1",
        "https://bot.example.org/users/status-bot",
        "https://bot.example.org/users/status-bot/followers",
        "Synthetic status event: all services are in dry-run mode.",
        published,
    )?;
    println!("{}", serde_json::to_string_pretty(activity.json())?);
    Ok(())
}
