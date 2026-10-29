use cepm_core::{Browser, Manager, RemoteOrigin, Target, remote::GitHubRepo};
use std::fs;
#[test]
fn remote_identity_survives_install_reopen_and_update() {
    let storage = tempfile::tempdir().unwrap();
    let source = tempfile::tempdir().unwrap();
    fs::write(
        source.path().join("manifest.json"),
        r#"{"manifest_version":3,"name":"Bridge","version":"1"}"#,
    )
    .unwrap();
    let origin = RemoteOrigin::GitHubDirectory {
        repo: GitHubRepo::parse("private/extension").unwrap(),
        reference: "main".into(),
        commit: "a".repeat(40),
        subdirectory: "dist".into(),
    };
    let mut manager = Manager::open(storage.path()).unwrap();
    let preview = manager
        .preview_local(source.path())
        .unwrap()
        .with_origin(origin.clone());
    let record = manager
        .install_local(
            preview,
            Target {
                browser: Browser::Chrome,
                profile: "test".into(),
            },
        )
        .unwrap();
    assert_eq!(record.origin, Some(origin.clone()));
    fs::write(
        source.path().join("manifest.json"),
        r#"{"manifest_version":3,"name":"Bridge","version":"2"}"#,
    )
    .unwrap();
    let updated_origin = RemoteOrigin::GitHubDirectory {
        repo: GitHubRepo::parse("private/extension").unwrap(),
        reference: "main".into(),
        commit: "b".repeat(40),
        subdirectory: "dist".into(),
    };
    let plan = manager
        .prepare_update(
            &record.id,
            manager
                .preview_local(source.path())
                .unwrap()
                .with_origin(updated_origin.clone()),
        )
        .unwrap();
    let kind = plan.kind();
    manager.apply_update(plan, kind).unwrap();
    drop(manager);
    assert_eq!(
        Manager::open(storage.path()).unwrap().list().unwrap()[0].origin,
        Some(updated_origin)
    );
}
