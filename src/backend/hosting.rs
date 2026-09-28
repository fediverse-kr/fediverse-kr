use crate::backend::auth::AuthError;
use crate::hosting::HostingEdit;

#[derive(Debug)]
pub enum Error {
    Auth(AuthError),
    Invalid,
    Missing,
    Duplicate,
    Conflict,
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
            Self::Invalid => "입력값과 웹사이트·출처 주소를 확인해 주세요.",
            Self::Missing => "서비스 또는 버전을 찾을 수 없어요.",
            Self::Duplicate => "이미 등록된 주소예요.",
            Self::Conflict => {
                "다른 수정이 먼저 저장됐어요. 입력을 복사해 두고 최신 내용을 확인해 주세요."
            }
            Self::RateLimited => "짧은 시간에 수정이 많아요. 잠시 후 다시 시도해 주세요.",
            Self::Unavailable => "잠시 후 다시 시도해 주세요.",
        })
    }
}
impl std::error::Error for Error {}

pub fn slug(value: &str) -> Result<String, Error> {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty()
        || value.len() > 64
        || !value.as_bytes()[0].is_ascii_alphanumeric()
        || !value
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
    {
        return Err(Error::Invalid);
    }
    Ok(value)
}
pub fn validate(
    mut edit: HostingEdit,
    mut summary: String,
) -> Result<(HostingEdit, String), Error> {
    fn clean(value: &mut String, max: usize, multiline: bool) -> Result<(), Error> {
        *value = value.trim().to_owned();
        if value.chars().count() > max
            || value
                .chars()
                .any(|c| c.is_control() && !(multiline && matches!(c, '\n' | '\r' | '\t')))
        {
            return Err(Error::Invalid);
        }
        Ok(())
    }
    for (value, max) in [
        (&mut edit.name, 160),
        (&mut edit.website_url, 2048),
        (&mut edit.scope, 240),
        (&mut edit.software, 240),
        (&mut edit.source_url, 2048),
        (&mut edit.checked_on, 10),
        (&mut summary, 200),
    ] {
        clean(value, max, false)?;
    }
    for value in [
        &mut edit.provider_responsibilities,
        &mut edit.customer_responsibilities,
    ] {
        clean(value, 1000, true)?;
    }
    if edit.name.is_empty() || edit.website_url.is_empty() || summary.is_empty() {
        return Err(Error::Invalid);
    }
    for link in [&edit.website_url, &edit.source_url] {
        if link.is_empty() {
            continue;
        }
        let url = url::Url::parse(link).map_err(|_| Error::Invalid)?;
        if crate::directory::web_link(link).is_none()
            || url.host().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(Error::Invalid);
        }
    }
    if !edit.checked_on.is_empty() {
        let date = chrono::NaiveDate::parse_from_str(&edit.checked_on, "%Y-%m-%d")
            .map_err(|_| Error::Invalid)?;
        if edit.source_url.is_empty() || date > chrono::Utc::now().date_naive() {
            return Err(Error::Invalid);
        }
    }
    Ok((edit, summary))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn managed_hosting_multiline_responsibilities_keep_line_breaks() {
        let base = HostingEdit {
            name: "공간".into(),
            website_url: "https://example.org".into(),
            provider_responsibilities: "설치와 업데이트\n백업과 장애 대응".into(),
            customer_responsibilities: "가입 정책\n이용자 문의 대응".into(),
            ..Default::default()
        };
        let (saved, _) = validate(base.clone(), "등록".into()).unwrap();
        assert_eq!(saved, base);
        let mut invalid = base.clone();
        invalid.provider_responsibilities.push('\0');
        assert!(validate(invalid, "등록".into()).is_err());
        let mut invalid = base;
        invalid.name.push('\n');
        invalid.name.push('x');
        assert!(validate(invalid, "등록".into()).is_err());
    }

    #[test]
    fn managed_hosting_validation_rejects_unsafe_links_and_unsupported_dates() {
        let base = HostingEdit {
            name: "공간".into(),
            website_url: "https://example.org".into(),
            ..Default::default()
        };
        assert!(validate(base.clone(), "등록".into()).is_ok());
        for url in [
            "javascript:alert(1)",
            "https://u@example.org",
            "https://x\\evil",
        ] {
            assert!(validate(
                HostingEdit {
                    website_url: url.into(),
                    ..base.clone()
                },
                "등록".into()
            )
            .is_err());
        }
        assert!(validate(
            HostingEdit {
                checked_on: "2025-01-01".into(),
                ..base.clone()
            },
            "등록".into()
        )
        .is_err());
        assert!(validate(
            HostingEdit {
                checked_on: "2999-01-01".into(),
                source_url: "https://example.org/docs".into(),
                ..base
            },
            "등록".into()
        )
        .is_err());
        assert!(slug("../oops").is_err());
    }
}
