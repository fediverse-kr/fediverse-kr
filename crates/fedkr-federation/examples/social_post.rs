use chrono::{DateTime, Utc};
use fedkr_federation::publication::PreparedActivity;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let published = DateTime::parse_from_rfc3339("2026-10-23T01:02:03Z")?.with_timezone(&Utc);
    let activity = PreparedActivity::public_note(
        "https://social.example.org/activities/demo-1",
        "https://social.example.org/notes/demo-1",
        "https://social.example.org/users/demo",
        "https://social.example.org/users/demo/followers",
        "Synthetic SNS post preview; this example never sends it.",
        published,
    )?;
    println!("{}", serde_json::to_string_pretty(activity.json())?);
    Ok(())
}
