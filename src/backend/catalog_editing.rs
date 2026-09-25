//! Contribution validation, separate from persistence and member authentication.
use super::auth::AuthError;
use crate::directory::catalog_editing::SoftwareEdit;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Auth(AuthError),
    Invalid,
    Missing,
    Duplicate,
    Conflict,
    Locked,
    Forbidden,
    RateLimited,
    Unavailable,
}
impl From<AuthError> for Error {
    fn from(e: AuthError) -> Self {
        Self::Auth(e)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Auth(e) => return e.fmt(f),
            Self::Invalid => "입력 길이·종류·웹사이트 주소를 확인해 주세요.",
            Self::Missing => "해당 소프트웨어·분류·버전을 찾을 수 없어요.",
            Self::Duplicate => "이미 등록된 식별자예요. 기존 정보를 수정해 주세요.",
            Self::Conflict => {
                "다른 수정이 먼저 저장됐어요. 입력은 남겨두었으니 최신 내용과 비교해 주세요."
            }
            Self::Locked => "이 항목은 현재 편집이 잠겨 있어요.",
            Self::Forbidden => "관리자만 사용할 수 있어요.",
            Self::RateLimited => "짧은 시간에 수정이 많아요. 잠시 후 다시 저장해 주세요.",
            Self::Unavailable => "정보를 처리하지 못했어요. 잠시 후 다시 시도해 주세요.",
        })
    }
}
impl std::error::Error for Error {}
pub struct ValidatedEdit {
    pub(crate) edit: SoftwareEdit,
    pub(crate) summary: String,
}
pub fn new_name(input: &str) -> Result<String, Error> {
    let name = input.trim().to_ascii_lowercase();
    if name.is_empty()
        || name.len() > 64
        || !name
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-' || c == b'_')
        || !name.as_bytes()[0].is_ascii_alphanumeric()
    {
        return Err(Error::Invalid);
    }
    Ok(name)
}
pub fn existing_name(name: &str) -> Result<(), Error> {
    if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
        Err(Error::Invalid)
    } else {
        Ok(())
    }
}
fn text(value: &mut String, max: usize, multiline: bool) -> Result<(), Error> {
    *value = value.trim().replace("\r\n", "\n");
    if value.chars().count() > max
        || value
            .chars()
            .any(|c| c.is_control() && !(multiline && matches!(c, '\n' | '\t')))
    {
        return Err(Error::Invalid);
    }
    Ok(())
}
pub fn validate(mut edit: SoftwareEdit, mut summary: String) -> Result<ValidatedEdit, Error> {
    text(&mut edit.display_name, 200, false)?;
    text(&mut edit.family, 128, false)?;
    text(&mut edit.description, 4000, true)?;
    text(&mut edit.tech_stack, 1000, true)?;
    text(&mut edit.website_url, 2048, false)?;
    text(&mut summary, 200, false)?;
    if edit.display_name.is_empty()
        || summary.is_empty()
        || edit.categories.is_empty()
        || edit.categories.len() > 32
        || edit.features.len() > 32
    {
        return Err(Error::Invalid);
    }
    for c in &mut edit.categories {
        text(c, 128, false)?;
        if c.is_empty() {
            return Err(Error::Invalid);
        }
    }
    edit.categories.sort();
    edit.categories.dedup();
    for feature in &mut edit.features {
        text(feature, 300, false)?;
    }
    edit.features.retain(|f| !f.is_empty());
    edit.features.dedup();
    if !edit.website_url.is_empty() {
        let u = url::Url::parse(&edit.website_url).map_err(|_| Error::Invalid)?;
        if crate::directory::web_link(&edit.website_url).is_none()
            || u.host().is_none()
            || !u.username().is_empty()
            || u.password().is_some()
        {
            return Err(Error::Invalid);
        }
    }
    Ok(ValidatedEdit { edit, summary })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contributions_are_bounded_plain_text_and_safe_navigation() {
        let base = SoftwareEdit {
            display_name: "새 도구".into(),
            categories: vec!["image".into()],
            ..Default::default()
        };
        assert_eq!(new_name(" New_App ").unwrap(), "new_app");
        for name in ["", "../x", "a/b", "-x", "새도구"] {
            assert!(new_name(name).is_err());
        }
        assert!(validate(base.clone(), "소개 추가".into()).is_ok());
        for bad in [
            "javascript:alert(1)",
            "https://",
            "https://user@site.example.org",
            "https://x\\evil",
        ] {
            assert!(validate(
                SoftwareEdit {
                    website_url: bad.into(),
                    ..base.clone()
                },
                "수정".into()
            )
            .is_err());
        }
        assert!(validate(base.clone(), "".into()).is_err());
        assert!(validate(
            SoftwareEdit {
                description: "가".repeat(4001),
                ..base.clone()
            },
            "수정".into()
        )
        .is_err());
        assert!(validate(
            SoftwareEdit {
                description: "<script>not markup</script>".into(),
                ..base
            },
            "수정".into()
        )
        .is_ok());
    }
}
