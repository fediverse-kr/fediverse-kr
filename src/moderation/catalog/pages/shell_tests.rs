// Source-level route wiring gate; runtime authorization and mutations remain
// covered by the existing server and authenticated browser suites.
#[test]
fn directory_lists_name_page_results_and_details_separate_actions() {
    let sites = include_str!("../../sites/pages.rs");
    let catalog = include_str!("../pages.rs");
    for source in [sites, catalog] {
        assert!(source.contains("이 페이지에"));
        assert!(source.contains("backoffice-catalog.css"));
        assert!(source.contains("directory-current"));
        assert!(source.contains("directory-actions"));
    }
    let history = sites.find("History{key:").unwrap();
    let delete = sites.find("DeletePanel{site:current").unwrap();
    assert!(
        history < delete,
        "irreversible deletion follows ordinary actions and history"
    );
    assert!(sites.contains("위험 구역"));
    assert!(catalog.contains("h2{\"관리 설정 변경\"}"));
    assert!(catalog.contains("logo::LogoEditor{current,pending,generation}"));
    assert!(catalog.contains("AdminHistory{name:data.name"));
}

#[test]
fn catalog_routes_share_shell_and_reuse_editor_body() {
    let source = include_str!("../pages.rs");
    assert_eq!(source.matches("CatalogPage{").count(), 5);
    assert!(source.contains("BackofficePage"));
    assert!(source.contains("BackofficeSection::Catalog"));
    assert!(!source.contains("main{id:"));
    assert!(!source.contains("h1{"));
    assert!(!source.contains("AdminNav"));
    assert!(source.contains("EditorBody{name,administrator:true}"));
    assert!(source.contains("공개 등록 화면으로"));
    assert!(source.contains("aria_current:"));
    let editor = include_str!("../../../directory/catalog_editing/pages.rs");
    assert!(editor.contains("EditorBody{name,administrator:false}"));
    assert!(!editor.contains("AdminNav"));
    for invariant in [
        "if administrator {",
        "api::create(slug(),edit,summary())",
        "api::save(record.name.clone(),record.revision,edit,summary())",
        "key:\"{name}-{administrator}\"",
        "current.name==name",
        "base.peek().name!=record.name",
        "!conflicts().is_empty() && !confirmed()",
    ] {
        assert!(
            editor.contains(invariant),
            "lost editor invariant: {invariant}"
        );
    }
}

#[test]
fn site_routes_use_the_common_shell_without_duplicate_landmarks() {
    let source = include_str!("../../sites/pages.rs");
    let implementation = source.split("#[cfg(test)]").next().unwrap();
    assert_eq!(implementation.matches("BackofficePage{").count(), 2);
    assert!(!implementation.contains("main{id:"));
    assert!(!implementation.contains("h1{"));
    assert!(!implementation.contains("AdminNav"));
    assert!(implementation.contains("BackofficeSection::Sites"));
    assert!(implementation.contains("to:Route::ModerationSites{}"));
    assert!(implementation.contains("DeletePanel{site:current,deleted,pending}"));
    assert!(implementation.contains("History{key:"));
}
