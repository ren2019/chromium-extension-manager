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

#[test]
fn release_archive_can_be_prepared_and_installed_with_its_durable_source() {
    use cepm_core::remote::{Gh, ReleaseSelection};
    use std::io::{Cursor, Write};
    struct Cli(Vec<u8>);
    impl Gh for Cli {
        fn run(&self, args: &[String], _: usize) -> cepm_core::Result<Vec<u8>> {
            if args.iter().any(|s| s.ends_with("/latest")) {
                return Ok(br#"{"tag_name":"product-v7"}"#.to_vec());
            }
            if args.iter().any(|s| s.contains("assets/9")) {
                return Ok(self.0.clone());
            }
            Ok(br#"[[{"tag_name":"product-v7","prerelease":false,"draft":false,"assets":[{"id":9,"name":"bridge-extension-v1.zip","state":"uploaded","size":200}]}]]"#.to_vec())
        }
    }
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    zip.start_file("manifest.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    zip.write_all(br#"{"manifest_version":3,"name":"Bridge","version":"1"}"#)
        .unwrap();
    let gh = Cli(zip.finish().unwrap().into_inner());
    let storage = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(storage.path()).unwrap();
    let prepared = manager
        .preview_release(
            &gh,
            "private/extension",
            &ReleaseSelection {
                tag: Some("product-v7".into()),
                ..Default::default()
            },
        )
        .unwrap();
    let preview = prepared.select_root(None).unwrap();
    let record = manager
        .install_local(
            preview,
            Target {
                browser: Browser::Chrome,
                profile: "test".into(),
            },
        )
        .unwrap();
    assert_eq!(record.installed_version, "1");
    assert!(matches!(record.origin,Some(RemoteOrigin::GitHubRelease{tag,..}) if tag=="product-v7"));
    assert!(record.install_dir.join("manifest.json").exists());
}

use cepm_core::remote::{Gh, ReleaseSelection};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    io::{Cursor, Write},
};
struct FixtureGh {
    pages: Value,
    latest: String,
    archives: BTreeMap<u64, Vec<u8>>,
}
impl Gh for FixtureGh {
    fn run(&self, args: &[String], _: usize) -> cepm_core::Result<Vec<u8>> {
        let endpoint = &args[3];
        if endpoint.ends_with("/latest") {
            return Ok(serde_json::to_vec(&json!({"tag_name":self.latest})).unwrap());
        }
        if let Some(id) = endpoint.rsplit_once("assets/").map(|(_, id)| id) {
            return self
                .archives
                .get(&id.parse::<u64>().unwrap())
                .cloned()
                .ok_or_else(|| "获取失败：404；请核对 gh 访问权限".into());
        }
        Ok(serde_json::to_vec(&self.pages).unwrap())
    }
}
fn zip_files(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, data) in files {
        zip.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap().into_inner()
}
fn archive(version: &str) -> Vec<u8> {
    let manifest = json!({"manifest_version":3,"name":"Bridge","version":version}).to_string();
    zip_files(&[("manifest.json", manifest.as_bytes())])
}
fn release(tag: &str, pre: bool, id: u64, name: &str) -> Value {
    json!({"tag_name":tag,"prerelease":pre,"assets":[{"id":id,"name":name,"state":"uploaded"}]})
}
fn fixture(pages: Value, latest: &str, archives: &[(u64, Vec<u8>)]) -> FixtureGh {
    FixtureGh {
        pages,
        latest: latest.into(),
        archives: archives.iter().cloned().collect(),
    }
}
#[test]
fn all_pages_and_explicit_prerelease_are_available_but_github_latest_is_default() {
    let dir = tempfile::tempdir().unwrap();
    let manager = Manager::open(dir.path()).unwrap();
    let gh = fixture(
        json!([
            [
                release("preview", true, 1, "extension.zip"),
                release("old", false, 2, "extension.zip")
            ],
            [release("latest", false, 3, "extension.zip")]
        ]),
        "latest",
        &[(1, archive("3")), (3, archive("2"))],
    );
    let catalog = manager
        .list_releases(&gh, "https://github.com/private/bridge")
        .unwrap();
    assert_eq!(catalog.releases.len(), 3);
    assert_eq!(catalog.default_tag.as_deref(), Some("latest"));
    let preview = manager
        .preview_release(&gh, "private/bridge", &ReleaseSelection::default())
        .unwrap()
        .select_root(None)
        .unwrap();
    assert_eq!(preview.version(), "2");
    let preview = manager
        .preview_release(
            &gh,
            "https://github.com/private/bridge/releases/tag/preview",
            &ReleaseSelection::default(),
        )
        .unwrap()
        .select_root(None)
        .unwrap();
    assert_eq!(preview.version(), "3");
}
#[test]
fn checksum_and_declared_extension_version_failures_preserve_installed_files() {
    let dir = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(dir.path()).unwrap();
    let good = fixture(
        json!([[release("product-7", false, 1, "bridge-extension-v1.zip")]]),
        "product-7",
        &[(1, archive("1"))],
    );
    let record = manager
        .install_local(
            manager
                .preview_release(&good, "private/bridge", &ReleaseSelection::default())
                .unwrap()
                .select_root(None)
                .unwrap(),
            Target {
                browser: Browser::Chrome,
                profile: "one".into(),
            },
        )
        .unwrap();
    for (digest, name) in [
        (
            Some(format!("sha256:{}", "0".repeat(64))),
            "bridge-extension-v2.zip",
        ),
        (None, "bridge-extension-v8.zip"),
    ] {
        let mut next = release("product-8", false, 2, name);
        next["assets"][0]["digest"] = json!(digest);
        let bad = fixture(json!([[next]]), "product-8", &[(2, archive("2"))]);
        assert!(
            manager
                .preview_release(&bad, "private/bridge", &ReleaseSelection::default())
                .is_err()
        );
        assert_eq!(manager.list().unwrap(), vec![record.clone()]);
        let manifest: Value =
            serde_json::from_slice(&fs::read(record.install_dir.join("manifest.json")).unwrap())
                .unwrap();
        assert_eq!(manifest["version"], "1");
    }
}
#[test]
fn ambiguous_assets_and_roots_require_explicit_selection_and_never_guess() {
    let dir = tempfile::tempdir().unwrap();
    let manager = Manager::open(dir.path()).unwrap();
    let mut rel = release("v1", false, 1, "notextension.zip");
    let gh = fixture(json!([[rel.clone()]]), "v1", &[(1, archive("1"))]);
    assert_eq!(
        manager
            .list_releases(&gh, "owner/repo")
            .unwrap()
            .preselected_asset("v1"),
        None
    );
    assert!(
        manager
            .preview_release(&gh, "owner/repo", &ReleaseSelection::default())
            .is_err()
    );
    rel["assets"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":2,"name":"bridge-extension.zip","state":"uploaded"}));
    rel["assets"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":3,"name":"other-extension.zip","state":"uploaded"}));
    let manifest = br#"{"manifest_version":3,"name":"Bridge","version":"1"}"#;
    let gh = fixture(
        json!([[rel]]),
        "v1",
        &[(
            2,
            zip_files(&[("a/manifest.json", manifest), ("b/manifest.json", manifest)]),
        )],
    );
    assert!(
        manager
            .preview_release(&gh, "owner/repo", &ReleaseSelection::default())
            .is_err()
    );
    let artifact = manager
        .preview_release(
            &gh,
            "owner/repo",
            &ReleaseSelection {
                tag: None,
                asset_id: Some(2),
            },
        )
        .unwrap();
    assert_eq!(artifact.roots(), vec!["a", "b"]);
    assert!(artifact.select_root(None).is_err());
    let preview = manager
        .preview_release(
            &gh,
            "owner/repo",
            &ReleaseSelection {
                tag: None,
                asset_id: Some(2),
            },
        )
        .unwrap()
        .select_root(Some("b"))
        .unwrap();
    assert!(matches!(preview.origin(),Some(RemoteOrigin::GitHubRelease{root,..}) if root=="b"));
}
#[test]
fn dangerous_archives_are_rejected_before_touching_any_installation() {
    let dir = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(dir.path()).unwrap();
    let good = fixture(
        json!([[release("v1", false, 1, "extension.zip")]]),
        "v1",
        &[(1, archive("1"))],
    );
    let record = manager
        .install_local(
            manager
                .preview_release(&good, "owner/repo", &ReleaseSelection::default())
                .unwrap()
                .select_root(None)
                .unwrap(),
            Target {
                browser: Browser::Chrome,
                profile: "one".into(),
            },
        )
        .unwrap();
    let manifest = br#"{"manifest_version":3,"name":"Bridge","version":"2"}"#;
    for names in [
        vec!["../escape"],
        vec!["/absolute"],
        vec!["C:/windows"],
        vec!["a\\b"],
        vec!["a/./b"],
        vec!["aux.txt"],
        vec!["COM¹.txt"],
        vec!["trail."],
        vec!["a", "A"],
        vec!["café", "cafe\u{301}"],
        vec!["file", "file/child"],
        vec!["A/one", "a/two"],
        vec!["straße", "strasse"],
    ] {
        let mut files = vec![("manifest.json", manifest.as_slice())];
        files.extend(names.iter().map(|n| (*n, b"test".as_slice())));
        let bad = fixture(
            json!([[release("v2", false, 2, "extension.zip")]]),
            "v2",
            &[(2, zip_files(&files))],
        );
        assert!(
            manager
                .preview_release(&bad, "owner/repo", &ReleaseSelection::default())
                .is_err(),
            "accepted {names:?}"
        );
        assert_eq!(manager.list().unwrap(), vec![record.clone()]);
    }
}

#[test]
fn saved_asset_and_root_choices_follow_versions_and_explicit_downgrades_keep_identity() {
    let dir = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(dir.path()).unwrap();
    let initial = fixture(
        json!([[release("product-9", false, 1, "bridge-extension-v2.zip")]]),
        "product-9",
        &[(1, archive("2"))],
    );
    let record = manager
        .install_local(
            manager
                .preview_release(&initial, "owner/repo", &ReleaseSelection::default())
                .unwrap()
                .select_root(None)
                .unwrap(),
            Target {
                browser: Browser::Chrome,
                profile: "one".into(),
            },
        )
        .unwrap();
    let next = fixture(
        json!([[
            release("product-10", false, 2, "bridge-extension-v3.zip"),
            release("product-8", false, 3, "bridge-extension-v1.zip")
        ]]),
        "product-10",
        &[(2, archive("3")), (3, archive("1"))],
    );
    let plan = manager.check_release(&next, &record.id).unwrap();
    assert_eq!(plan.kind(), cepm_core::UpdateKind::Upgrade);
    let upgraded = manager
        .apply_update(plan, cepm_core::UpdateKind::Upgrade)
        .unwrap();
    assert_eq!(upgraded.install_dir, record.install_dir);
    assert_eq!(upgraded.installed_version, "3");
    assert_eq!(upgraded.verification, cepm_core::Verification::Pending);
    manager
        .confirm(&record.id, upgraded.generation, &"a".repeat(32), "3")
        .unwrap();
    let preview = manager
        .preview_release(
            &next,
            "owner/repo",
            &ReleaseSelection {
                tag: Some("product-8".into()),
                asset_id: None,
            },
        )
        .unwrap()
        .select_root(None)
        .unwrap();
    let plan = manager.prepare_update(&record.id, preview).unwrap();
    assert_eq!(plan.kind(), cepm_core::UpdateKind::Downgrade);
    let lower = manager
        .apply_update(plan, cepm_core::UpdateKind::Downgrade)
        .unwrap();
    assert_eq!(lower.install_dir, record.install_dir);
    assert_eq!(lower.installed_version, "1");
    assert_eq!(lower.extension_id.as_deref(), Some("a".repeat(32).as_str()));
    let recovered = manager
        .report_load_failure(&record.id, lower.generation)
        .unwrap();
    assert_eq!(recovered.installed_version, "3");
    assert!(
        matches!(recovered.origin,Some(RemoteOrigin::GitHubRelease{tag,..}) if tag=="product-10")
    );
}
#[test]
fn release_urls_preserve_encoded_tags_and_reject_foreign_or_credential_urls() {
    let dir = tempfile::tempdir().unwrap();
    let manager = Manager::open(dir.path()).unwrap();
    let gh = fixture(
        json!([[release("release/old", false, 1, "extension.zip")]]),
        "release/old",
        &[(1, archive("1"))],
    );
    let preview = manager
        .preview_release(
            &gh,
            "https://github.com/owner/releases/releases/tag/release%2Fold",
            &ReleaseSelection::default(),
        )
        .unwrap()
        .select_root(None)
        .unwrap();
    assert_eq!(preview.version(), "1");
    assert_eq!(
        manager
            .list_releases(&gh, "owner/releases")
            .unwrap()
            .repo
            .name,
        "releases"
    );
    for source in [
        "https://github.com.evil/owner/repo",
        "https://token@github.com/owner/repo",
        "https://gitlab.com/owner/repo",
        "https://github.com/owner/repo?token=secret",
        "https://github.com/owner/repo/releases/tag/x?token=secret",
        "--hostname/evil",
    ] {
        assert!(
            manager.list_releases(&gh, source).is_err(),
            "accepted {source}"
        );
    }
}
#[test]
fn duplicate_entries_links_corruption_and_oversized_entries_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let manager = Manager::open(dir.path()).unwrap();
    let manifest = br#"{"manifest_version":3,"name":"Bridge","version":"1"}"#;
    let mut duplicate = zip_files(&[
        ("manifest.json", manifest),
        ("first.txt", b"one"),
        ("other.txt", b"two"),
    ]);
    for i in 0..duplicate.len() - 9 {
        if &duplicate[i..i + 9] == b"other.txt" {
            duplicate[i..i + 9].copy_from_slice(b"first.txt");
        }
    }
    let mut symlink = zip::ZipWriter::new(Cursor::new(Vec::new()));
    symlink
        .start_file("manifest.json", zip::write::SimpleFileOptions::default())
        .unwrap();
    symlink.write_all(manifest).unwrap();
    symlink
        .add_symlink(
            "link",
            "../outside",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
    let mut oversized = archive("1");
    let index = oversized
        .windows(4)
        .position(|p| p == b"PK\x01\x02")
        .unwrap();
    oversized[index + 24..index + 28].copy_from_slice(&(65u32 * 1024 * 1024).to_le_bytes());
    for (label, bytes) in [
        ("duplicate", duplicate),
        ("symlink", symlink.finish().unwrap().into_inner()),
        ("size", oversized),
        ("corrupt", b"not a ZIP".to_vec()),
    ] {
        let gh = fixture(
            json!([[release("v1", false, 1, "extension.zip")]]),
            "v1",
            &[(1, bytes)],
        );
        assert!(
            manager
                .preview_release(&gh, "owner/repo", &ReleaseSelection::default())
                .is_err(),
            "accepted {label}"
        );
    }
}
#[test]
fn changed_selectors_or_cli_failures_pause_checks_without_changing_existing_installations() {
    let dir = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(dir.path()).unwrap();
    let manifest = br#"{"manifest_version":3,"name":"Bridge","version":"1"}"#;
    let initial = fixture(
        json!([[release("v1", false, 1, "bridge-extension-v1.zip")]]),
        "v1",
        &[(1, zip_files(&[("dist/manifest.json", manifest)]))],
    );
    let record = manager
        .install_local(
            manager
                .preview_release(&initial, "private/repo", &ReleaseSelection::default())
                .unwrap()
                .select_root(None)
                .unwrap(),
            Target {
                browser: Browser::Chrome,
                profile: "one".into(),
            },
        )
        .unwrap();
    let mut ambiguous = release("v2", false, 2, "bridge-extension-v2.zip");
    ambiguous["assets"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id":3,"name":"bridge-extension-v3.zip","state":"uploaded"}));
    for next in [
        release("v2", false, 2, "other-extension.zip"),
        ambiguous,
        release("v2", false, 2, "bridge-extension-v2.zip"),
    ] {
        let gh = fixture(json!([[next]]), "v2", &[(2, archive("2"))]);
        assert!(manager.check_release(&gh, &record.id).is_err());
        assert_eq!(manager.list().unwrap(), vec![record.clone()]);
    }
    struct Denied;
    impl Gh for Denied {
        fn run(&self, _: &[String], _: usize) -> cepm_core::Result<Vec<u8>> {
            Err("获取失败：404，来源不存在或无权访问；请检查 gh auth status".into())
        }
    }
    let error = manager.check_release(&Denied, &record.id).err().unwrap();
    assert!(error.contains("查询版本失败"));
    assert!(error.contains("gh auth status"));
    assert_eq!(manager.list().unwrap(), vec![record]);
    let local = tempfile::tempdir().unwrap();
    fs::write(local.path().join("manifest.json"), manifest).unwrap();
    assert!(manager.preview_local(local.path()).is_ok());
}

#[test]
fn reinstall_uses_saved_release_asset_even_when_newest_release_differs() {
    let temp = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(temp.path()).unwrap();
    let gh = FixtureGh {
        pages: json!([[{"tag_name":"old-product","assets":[{"id":1,"name":"extension-v1.zip","state":"uploaded"}]},{"tag_name":"new-product","assets":[{"id":2,"name":"extension-v2.zip","state":"uploaded"}]}]]),
        latest: "new-product".into(),
        archives: [(1, archive("1")), (2, archive("2"))].into(),
    };
    let preview = manager
        .preview_release(
            &gh,
            "private/extension",
            &ReleaseSelection {
                tag: Some("old-product".into()),
                asset_id: Some(1),
            },
        )
        .unwrap()
        .select_root(None)
        .unwrap();
    let target = Target {
        browser: Browser::Edge,
        profile: "test".into(),
    };
    let old = manager.install_local(preview, target.clone()).unwrap();
    let plan = manager
        .prepare_reinstall(&gh, &old.id, &target, old.generation)
        .unwrap();
    let repaired = manager
        .apply_update(plan, cepm_core::UpdateKind::Redeploy)
        .unwrap();
    assert_eq!(repaired.installed_version, "1");
    assert_eq!(repaired.origin, old.origin);
}
