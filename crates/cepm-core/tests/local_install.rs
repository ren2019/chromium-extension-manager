use cepm_core::{Browser, Manager, Target, Verification};
use std::fs;
use tempfile::TempDir;

fn fixture(base: &std::path::Path) -> std::path::PathBuf {
    let source = base.join("来源目录");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("manifest.json"), r#"{"manifest_version":3,"name":"测试扩展","version":"1.2.3","background":{"service_worker":"worker.js"}}"#).unwrap();
    fs::write(source.join("worker.js"), "// original content").unwrap();
    source
}

fn target() -> Target {
    Target {
        browser: Browser::Chrome,
        profile: "测试配置".into(),
    }
}

#[test]
fn local_install_survives_source_removal_and_manager_restart_pending_confirmation() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let root = temp.path().join("managed");
    let mut manager = Manager::open(&root).unwrap();
    let preview = manager.preview_local(&source).unwrap();
    assert_eq!(preview.name(), "测试扩展");
    assert_eq!(preview.version(), "1.2.3");
    let record = manager.install_local(preview, target()).unwrap();
    assert_eq!(record.verification, Verification::Pending);
    assert!(record.extension_id.is_none());
    assert!(record.effective_version.is_none());
    assert_eq!(
        fs::read_to_string(source.join("worker.js")).unwrap(),
        "// original content"
    );
    fs::remove_dir_all(source).unwrap();
    assert_eq!(
        fs::read_to_string(record.install_dir.join("worker.js")).unwrap(),
        "// original content"
    );
    drop(manager);
    let manager = Manager::open(&root).unwrap();
    assert_eq!(manager.list().unwrap(), vec![record]);
}

#[test]
fn invalid_or_incomplete_artifacts_are_rejected_before_any_installation() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let manager = Manager::open(&temp.path().join("managed")).unwrap();
    let invalid = [
        r#"{"manifest_version":2,"name":"x","version":"1"}"#,
        r#"{"manifest_version":3,"name":" ","version":"1"}"#,
        r#"{"manifest_version":3,"name":"x","version":"1.02"}"#,
        r#"{"manifest_version":3,"name":"x","version":"65536"}"#,
        r#"{"manifest_version":3,"name":"x","version":"1.2.3.4.5"}"#,
        r#"{"manifest_version":3,"name":"x","version":"1","background":{"service_worker":"missing.js"}}"#,
        r#"{"manifest_version":3,"name":"x","version":"1","content_scripts":[{"matches":["https://example.com/*"],"js":["missing.js"]}]}"#,
        r#"{"manifest_version":3,"name":"x","version":"1","action":{"default_popup":"../outside.html"}}"#,
    ];
    for manifest in invalid {
        fs::write(source.join("manifest.json"), manifest).unwrap();
        assert!(
            manager.preview_local(&source).is_err(),
            "accepted {manifest}"
        );
        assert!(manager.list().unwrap().is_empty());
    }
}

#[test]
#[cfg(unix)]
fn unsafe_links_and_overlapping_managed_directories_are_rejected() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let root = temp.path().join("managed");
    let manager = Manager::open(&root).unwrap();
    let outside = temp.path().join("outside.txt");
    fs::write(&outside, "not extension data").unwrap();
    std::os::unix::fs::symlink(&outside, source.join("link.txt")).unwrap();
    assert!(manager.preview_local(&source).is_err());
    assert!(manager.preview_local(temp.path()).is_err());
    assert!(manager.preview_local(&root).is_err());
    assert!(manager.preview_local(&root.join("installations")).is_err());
    assert!(manager.list().unwrap().is_empty());
}

#[test]
fn only_matching_browser_identity_and_version_can_be_user_confirmed() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let root = temp.path().join("managed");
    let mut manager = Manager::open(&root).unwrap();
    let preview = manager.preview_local(&source).unwrap();
    let record = manager.install_local(preview, target()).unwrap();
    let extension_id = "abcdefghijklmnopabcdefghijklmnop";
    assert!(manager.confirm(&record.id, extension_id, "9.0.0").is_err());
    assert!(
        manager
            .confirm(&record.id, "not-an-extension-id", "1.2.3")
            .is_err()
    );
    assert_eq!(
        manager.list().unwrap()[0].verification,
        Verification::Pending
    );
    let confirmed = manager.confirm(&record.id, extension_id, "1.2.3").unwrap();
    assert_eq!(confirmed.verification, Verification::UserConfirmed);
    assert_eq!(confirmed.effective_version.as_deref(), Some("1.2.3"));
    assert_eq!(confirmed.extension_id.as_deref(), Some(extension_id));
    drop(manager);
    assert_eq!(
        Manager::open(&root).unwrap().list().unwrap(),
        vec![confirmed]
    );
}

#[test]
fn installing_requires_a_profile_and_metadata_failure_leaves_no_untracked_installation() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let root = temp.path().join("managed");
    let mut manager = Manager::open(&root).unwrap();
    let preview = manager.preview_local(&source).unwrap();
    assert!(
        manager
            .install_local(
                preview,
                Target {
                    browser: Browser::Chrome,
                    profile: " ".into()
                }
            )
            .is_err()
    );
    assert!(manager.list().unwrap().is_empty());
    let preview = manager.preview_local(&source).unwrap();
    fs::remove_dir(root.join("records")).unwrap();
    fs::write(
        root.join("records"),
        "simulate unavailable records directory",
    )
    .unwrap();
    assert!(manager.install_local(preview, target()).is_err());
    assert_eq!(fs::read_dir(root.join("installations")).unwrap().count(), 0);
}

#[test]
fn corrupted_record_cannot_redirect_confirmation_writes() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let root = temp.path().join("managed");
    let mut manager = Manager::open(&root).unwrap();
    let preview = manager.preview_local(&source).unwrap();
    let record = manager.install_local(preview, target()).unwrap();
    let record_path = root.join("records").join(format!("{}.json", record.id));
    let mut altered = serde_json::to_value(&record).unwrap();
    altered["id"] = "../../outside".into();
    fs::write(record_path, serde_json::to_vec(&altered).unwrap()).unwrap();
    assert!(manager.list().is_err());
    assert!(
        manager
            .confirm("../../outside", "abcdefghijklmnopabcdefghijklmnop", "1.2.3")
            .is_err()
    );
    assert!(!temp.path().join("outside.json").exists());
}

#[test]
fn declared_rule_sets_and_sandbox_pages_must_exist() {
    let temp = TempDir::new().unwrap();
    let source = fixture(temp.path());
    let manager = Manager::open(&temp.path().join("managed")).unwrap();
    for extra in [
        serde_json::json!({"declarative_net_request":{"rule_resources":[{"id":"rules","enabled":true,"path":"rules.json"}]}}),
        serde_json::json!({"sandbox":{"pages":["sandbox.html"]}}),
    ] {
        let mut manifest = serde_json::json!({"manifest_version":3,"name":"test","version":"1"});
        manifest
            .as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        fs::write(
            source.join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        assert!(manager.preview_local(&source).is_err());
    }
    fs::write(source.join("sandbox.html"), "<!doctype html>").unwrap();
    assert!(manager.preview_local(&source).is_ok());
}
