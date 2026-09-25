//! Public contribution DTOs; never serialize member IDs or linked identities.
pub mod api;
pub mod pages;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Kinds {
    pub items: Vec<super::Category>,
    pub truncated: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SoftwareEdit {
    pub display_name: String,
    pub family: String,
    pub description: String,
    pub categories: Vec<String>,
    pub features: Vec<String>,
    pub website_url: String,
    pub tech_stack: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EditableSoftware {
    pub name: String,
    pub revision: i64,
    pub locked: bool,
    pub edit: SoftwareEdit,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Revision {
    pub revision: i64,
    pub action: String,
    pub summary: String,
    pub created_at: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct History {
    pub name: String,
    pub current_revision: i64,
    pub locked: bool,
    pub entries: Vec<Revision>,
    pub page: u32,
    pub has_next: bool,
}

/// A three-way merge keeps remote-only changes and never silently chooses an
/// overlapping field. The UI must compare and acknowledge conflicts before save.
pub fn merge(
    base: &SoftwareEdit,
    mine: &SoftwareEdit,
    remote: &SoftwareEdit,
) -> (SoftwareEdit, Vec<String>) {
    fn field<T: Clone + PartialEq>(
        base: &T,
        mine: &T,
        remote: &T,
        label: &str,
        conflicts: &mut Vec<String>,
    ) -> T {
        if mine == base {
            return remote.clone();
        }
        if remote != base && remote != mine {
            conflicts.push(label.into());
        }
        mine.clone()
    }
    let mut conflicts = vec![];
    let edit = SoftwareEdit {
        display_name: field(
            &base.display_name,
            &mine.display_name,
            &remote.display_name,
            "이름",
            &mut conflicts,
        ),
        family: field(
            &base.family,
            &mine.family,
            &remote.family,
            "계열",
            &mut conflicts,
        ),
        description: field(
            &base.description,
            &mine.description,
            &remote.description,
            "소개",
            &mut conflicts,
        ),
        categories: field(
            &base.categories,
            &mine.categories,
            &remote.categories,
            "종류",
            &mut conflicts,
        ),
        features: field(
            &base.features,
            &mine.features,
            &remote.features,
            "기능",
            &mut conflicts,
        ),
        website_url: field(
            &base.website_url,
            &mine.website_url,
            &remote.website_url,
            "웹사이트",
            &mut conflicts,
        ),
        tech_stack: field(
            &base.tech_stack,
            &mine.tech_stack,
            &remote.tech_stack,
            "기술",
            &mut conflicts,
        ),
    };
    (edit, conflicts)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn merges_only_non_overlapping_changes() {
        let base = SoftwareEdit {
            display_name: "처음".into(),
            description: "기존".into(),
            ..Default::default()
        };
        let mine = SoftwareEdit {
            description: "내 소개".into(),
            ..base.clone()
        };
        let mut remote = SoftwareEdit {
            display_name: "새 이름".into(),
            ..base.clone()
        };
        let (merged, conflicts) = merge(&base, &mine, &remote);
        assert_eq!(merged.display_name, "새 이름");
        assert_eq!(merged.description, "내 소개");
        assert!(conflicts.is_empty());
        remote.description = "다른 소개".into();
        let (merged, conflicts) = merge(&base, &mine, &remote);
        assert_eq!(merged.description, "내 소개");
        assert_eq!(conflicts, ["소개"]);
    }
}
