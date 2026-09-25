//! Policy/validation only; DB and authorization remain in repository adapters.
use super::catalog_editing::{existing_name, new_name, Error};
use crate::moderation::catalog::{Action, CategoryRequest, Request};
pub fn query(value: &str, page: u32) -> Result<(), Error> {
    if value.len() > 256 || value.chars().any(char::is_control) || page > 10000 {
        Err(Error::Invalid)
    } else {
        Ok(())
    }
}
fn text(s: &mut String, max: usize, lines: bool) -> Result<(), Error> {
    *s = s.trim().replace("\r\n", "\n");
    if s.chars().count() > max
        || s.chars()
            .any(|c| c.is_control() && !(lines && matches!(c, '\n' | '\t')))
    {
        return Err(Error::Invalid);
    }
    Ok(())
}
fn note(s: &mut String) -> Result<(), Error> {
    text(s, 1000, true)?;
    if s.is_empty() || s.len() > 4000 {
        Err(Error::Invalid)
    } else {
        Ok(())
    }
}
pub fn software(mut v: Request) -> Result<Request, Error> {
    existing_name(&v.name)?;
    if v.revision < 0 {
        return Err(Error::Invalid);
    }
    note(&mut v.note)?;
    if let Action::BrandColor(ref mut color) = v.action {
        if let Some(s) = color {
            *s = s.trim().to_ascii_uppercase();
            if s.is_empty() {
                *color = None;
            } else if s.len() != 7
                || !s.starts_with('#')
                || !s.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
            {
                return Err(Error::Invalid);
            }
        }
    }
    Ok(v)
}
pub fn logo(
    mut v: crate::moderation::catalog::LogoRequest,
) -> Result<crate::moderation::catalog::LogoRequest, Error> {
    existing_name(&v.name)?;
    if v.revision < 0
        || v.data.as_ref().is_some_and(|s| {
            s.is_empty() || s.len() > crate::moderation::catalog::LOGO_MAX_BYTES.div_ceil(3) * 4
        })
    {
        return Err(Error::Invalid);
    }
    note(&mut v.note)?;
    Ok(v)
}
pub fn category(mut v: CategoryRequest) -> Result<CategoryRequest, Error> {
    if let Some(rev) = v.revision {
        existing_name(&v.name)?;
        if rev < 0 {
            return Err(Error::Invalid);
        }
    } else {
        v.name = new_name(&v.name)?;
    }
    text(&mut v.edit.label, 200, false)?;
    text(&mut v.edit.emoji, 32, false)?;
    note(&mut v.note)?;
    if v.edit.label.is_empty() {
        return Err(Error::Invalid);
    }
    Ok(v)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn admin_validation_preserves_names_and_rejects_css() {
        let make = |s: &str| Request {
            name: "legacy.name".into(),
            revision: 0,
            action: Action::BrandColor(Some(s.into())),
            note: " 수정 ".into(),
        };
        assert_eq!(
            software(make(" #abcdef ")).unwrap().action,
            Action::BrandColor(Some("#ABCDEF".into()))
        );
        for bad in [
            "red",
            "#fff",
            "#123456;display:none",
            "url(https://example.org)",
        ] {
            assert!(software(make(bad)).is_err());
        }
        assert!(query("%_\\", 0).is_ok());
        assert!(query("\n", 0).is_err());
        assert!(query("", 10001).is_err());
    }
}
