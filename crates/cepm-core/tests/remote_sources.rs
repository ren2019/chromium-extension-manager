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
