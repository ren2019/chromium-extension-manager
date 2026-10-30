use cepm_core::{Browser, Manager, Target};
use std::fs;

#[test]
fn installed_metadata_uses_owned_manifest_and_rejects_outside_icon() {
    let source = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    fs::write(source.path().join("manifest.json"), r#"{"manifest_version":3,"name":"Example","version":"1.0","description":"Author description"}"#).unwrap();
    let mut manager = Manager::open(data.path()).unwrap();
    let preview = manager.preview_local(source.path()).unwrap();
    let installed = manager
        .install_local(
            preview,
            Target {
                browser: Browser::Edge,
                profile: "personal".into(),
            },
        )
        .unwrap();
    let metadata = manager.presentation(&installed.id).unwrap();
    assert_eq!(metadata.description.as_deref(), Some("Author description"));
    assert!(metadata.icon_png.is_none());
    fs::write(installed.install_dir.join("manifest.json"), r#"{"manifest_version":3,"name":"Example","version":"1.0","description":"__MSG_description__","icons":{"128":"../../outside.png"}}"#).unwrap();
    let metadata = manager.presentation(&installed.id).unwrap();
    assert!(metadata.description.is_none());
    assert!(metadata.icon_png.is_none());
}

#[test]
fn same_name_and_guessed_release_selector_do_not_merge_artifacts() {
    use cepm_core::RemoteOrigin;
    use cepm_core::remote::GitHubRepo;
    let source = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    fs::write(
        source.path().join("manifest.json"),
        r#"{"manifest_version":3,"name":"Example","version":"1.0"}"#,
    )
    .unwrap();
    let mut manager = Manager::open(data.path()).unwrap();
    let mut ids = Vec::new();
    for asset in ["thing-v1.zip", "thing-v2.zip"] {
        let preview = manager.preview_local(source.path()).unwrap().with_origin(
            RemoteOrigin::GitHubRelease {
                repo: GitHubRepo::parse("owner/repo").unwrap(),
                tag: "latest".into(),
                asset_name: asset.into(),
                asset_selector: "thing-v*.zip".into(),
                root: "".into(),
            },
        );
        ids.push(
            manager
                .install_local(
                    preview,
                    Target {
                        browser: Browser::Edge,
                        profile: asset.into(),
                    },
                )
                .unwrap()
                .id,
        );
    }
    assert_eq!(manager.related_installations(&ids[0]).unwrap().len(), 1);
}

#[test]
fn artwork_decodes_owned_files_and_falls_back_after_corruption() {
    let source = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    fs::write(
        source.path().join("manifest.json"),
        r#"{"manifest_version":3,"name":"Example","version":"1.0","icons":{"128":"icon.png"}}"#,
    )
    .unwrap();
    image::RgbaImage::from_pixel(2, 2, image::Rgba([0, 0, 255, 255]))
        .save(source.path().join("icon.png"))
        .unwrap();
    let mut manager = Manager::open(data.path()).unwrap();
    let preview = manager.preview_local(source.path()).unwrap();
    let r = manager
        .install_local(
            preview,
            Target {
                browser: Browser::Edge,
                profile: "personal".into(),
            },
        )
        .unwrap();
    let png = manager.presentation(&r.id).unwrap().icon_png.unwrap();
    let decoded = image::load_from_memory(&png).unwrap().to_rgba8();
    assert_eq!(decoded.get_pixel(0, 0).0, [0, 0, 255, 255]);
    fs::write(r.install_dir.join("icon.png"), b"invalid image").unwrap();
    assert!(manager.presentation(&r.id).unwrap().icon_png.is_none());
    assert_eq!(manager.list().unwrap().len(), 1);
    #[cfg(unix)]
    {
        fs::remove_file(r.install_dir.join("icon.png")).unwrap();
        std::os::unix::fs::symlink(
            source.path().join("icon.png"),
            r.install_dir.join("icon.png"),
        )
        .unwrap();
        assert!(manager.presentation(&r.id).unwrap().icon_png.is_none());
    }
}

#[test]
fn related_local_installs_survive_reopen_without_mutating_other_profiles() {
    let source = tempfile::tempdir().unwrap();
    let unrelated = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    for path in [source.path(), unrelated.path()] {
        fs::write(
            path.join("manifest.json"),
            r#"{"manifest_version":3,"name":"Same name","version":"1.0"}"#,
        )
        .unwrap();
    }
    let mut manager = Manager::open(data.path()).unwrap();
    let mut records = Vec::new();
    for (path, profile) in [
        (source.path(), "personal"),
        (source.path(), "work"),
        (unrelated.path(), "other"),
    ] {
        let preview = manager.preview_local(path).unwrap();
        records.push(
            manager
                .install_local(
                    preview,
                    Target {
                        browser: Browser::Edge,
                        profile: profile.into(),
                    },
                )
                .unwrap(),
        );
    }
    let before = manager.list().unwrap();
    let related = manager.related_installations(&records[0].id).unwrap();
    assert_eq!(related.len(), 2);
    assert!(related.iter().all(|r| r.target.profile != "other"));
    assert_eq!(before, manager.list().unwrap());
    drop(manager);
    let manager = Manager::open(data.path()).unwrap();
    assert_eq!(
        manager.related_installations(&records[0].id).unwrap(),
        related
    );
}
