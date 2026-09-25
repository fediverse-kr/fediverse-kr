use super::*;

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn visibility_and_session_revocation_during_io_are_rechecked() {
    let f = Fixture::new().await;
    let store = f.store();
    let fixture = &f;
    let storage = &store;
    let site = Target::Site(f.site.clone());
    let result = media::read_with(&f.db, &site, None, |reference| async move {
        let bytes = storage.read(&reference).await.unwrap();
        fixture
            .sql(&format!(
                "UPDATE directory_sites SET is_hidden=true WHERE id='{}'",
                fixture.site_id
            ))
            .await;
        Ok(bytes)
    })
    .await;
    assert!(result.unwrap().is_none());
    let result = media::read_with(
        &f.db,
        &Target::OwnAvatar,
        Some(&f.member.token),
        |reference| async move {
            let bytes = storage.read(&reference).await.unwrap();
            fixtures::ban(&fixture.db, fixture.member.member.id, true).await;
            Ok(bytes)
        },
    )
    .await;
    assert!(matches!(result, Err(Error::Unauthorized)));
    f.dispose().await;
}

#[tokio::test]
#[ignore = "requires isolated FEDKR_TEST_DATABASE_URL"]
async fn private_key_misreferences_are_not_exposed_as_public_logos_and_changed_logo_is_rechecked() {
    let f = Fixture::new().await;
    let store = f.store();
    let fixture = &f;
    let storage = &store;
    let logo = Target::Software(f.software.clone());
    let result = media::read_with(&f.db, &logo, None, |reference| async move {
        let bytes = storage.read(&reference).await.unwrap();
        let mut conn = fixture.db.pool.get().await.unwrap();
        diesel::sql_query("UPDATE catalog_software SET logo_key=$1 WHERE name=$2")
            .bind::<Text, _>(&fixture.keys[0])
            .bind::<Text, _>(&fixture.software)
            .execute(&mut conn)
            .await
            .unwrap();
        Ok(bytes)
    })
    .await;
    assert!(result.unwrap().is_none());
    assert!(
        !f.db
            .public_software(&f.software)
            .await
            .unwrap()
            .software
            .unwrap()
            .logo_available
    );
    assert!(media::read(&f.db, Some(&store), &logo, None)
        .await
        .unwrap()
        .is_none());
    f.dispose().await;
}
