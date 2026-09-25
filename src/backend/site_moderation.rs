use super::moderation::{self, Error};
use crate::{
    directory::management::SiteEdit,
    moderation::sites::{Action, DeleteRequest, Request},
};
use uuid::Uuid;

pub struct Command {
    pub id: Uuid,
    pub revision: i64,
    pub action: Action,
    pub note: String,
    pub tags: Option<Vec<String>>,
}
impl TryFrom<Request> for Command {
    type Error = Error;
    fn try_from(r: Request) -> Result<Self, Error> {
        let note = super::community::text(r.note, 1000, true).map_err(|_| Error::Invalid)?;
        if r.revision < 0 || note.len() > 4000 {
            return Err(Error::Invalid);
        }
        let tags = if let Action::Tags(tags) = &r.action {
            Some(
                super::site_management::validate_edit(SiteEdit {
                    tags: tags.clone(),
                    ..Default::default()
                })
                .map_err(|_| Error::Invalid)?
                .tags,
            )
        } else {
            None
        };
        Ok(Self {
            id: moderation::id(&r.id)?,
            revision: r.revision,
            action: r.action,
            note,
            tags,
        })
    }
}

/// Deliberately separate from ordinary site edits: the domain is an exact,
/// case-sensitive confirmation string, never a normalized lookup key.
pub struct DeleteCommand {
    pub id: Uuid,
    pub revision: i64,
    pub domain: String,
    pub note: String,
}

impl TryFrom<DeleteRequest> for DeleteCommand {
    type Error = Error;

    fn try_from(request: DeleteRequest) -> Result<Self, Error> {
        if !request.confirmed
            || request.revision < 0
            || request.domain.is_empty()
            || request.domain.len() > 253
            || request.domain.chars().any(char::is_control)
        {
            return Err(Error::Invalid);
        }
        let note = super::community::text(request.note, 1000, true).map_err(|_| Error::Invalid)?;
        if note.len() > 4000 {
            return Err(Error::Invalid);
        }
        Ok(Self {
            id: moderation::id(&request.id)?,
            revision: request.revision,
            domain: request.domain,
            note,
        })
    }
}
pub fn query(value: &str) -> Result<String, Error> {
    let value = value.trim();
    if value.len() > 512 || value.chars().count() > 128 || value.chars().any(char::is_control) {
        return Err(Error::Invalid);
    }
    Ok(value.to_owned())
}
pub fn filter(value: &str) -> Result<&str, Error> {
    match value {
        "all" | "visible" | "hidden" | "force_hidden" | "closed" => Ok(value),
        _ => Err(Error::Invalid),
    }
}
// Only these fixed expressions can enter SQL; never interpolate user input.
pub fn order(value: &str) -> Result<&'static str, Error> {
    match value {
        "domain" => Ok("s.domain,s.id"),
        "recent" => Ok("s.created_at DESC,s.id DESC"),
        "users" => Ok("o.user_count DESC NULLS LAST,s.domain,s.id"),
        _ => Err(Error::Invalid),
    }
}
