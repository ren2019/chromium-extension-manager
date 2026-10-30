use cepm_core::{
    Browser, Manager, Target, Verification,
    bulk::BulkStatus,
    remote::{Gh, ReleaseSelection},
};
use std::{
    fs,
    io::{Cursor, Write},
    path::Path,
};
use zip::{ZipWriter, write::SimpleFileOptions};

struct Sources {
    version: &'static str,
}
impl Gh for Sources {
    fn run(&self, args: &[String], _: usize) -> cepm_core::Result<Vec<u8>> {
        let endpoint = &args[3];
        if endpoint.contains("releases?per_page") {
            return Ok(serde_json::to_vec(&serde_json::json!([[{"tag_name":"latest-stable", "assets":[{"id":42,"name":"extension.zip","state":"uploaded"}]}]])).unwrap());
        }
        if endpoint.ends_with("releases/latest") {
            return Ok(br#"{"tag_name":"latest-stable"}"#.to_vec());
        }
        if endpoint.contains("commits/") {
            return Ok(format!(r#"{{"sha":"{}"}}"#, "a".repeat(40)).into_bytes());
        }
        if endpoint.contains("zipball/") {
            return Ok(archive(self.version, "repo/dist/"));
        }
        if endpoint.ends_with("releases/assets/42") {
            return Ok(archive(self.version, ""));
        }
        Err(format!("unexpected external request: {endpoint}"))
    }
}
fn archive(version: &str, prefix: &str) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file(
            format!("{prefix}manifest.json"),
            SimpleFileOptions::default(),
        )
        .unwrap();
    writer.write_all(manifest(version).as_bytes()).unwrap();
    writer.finish().unwrap().into_inner()
}
fn manifest(version: &str) -> String {
    format!(r#"{{"manifest_version":3,"name":"Example","version":"{version}"}}"#)
}
fn source(path: &Path, version: &str) {
    fs::create_dir_all(path).unwrap();
    fs::write(path.join("manifest.json"), manifest(version)).unwrap();
}
fn target(profile: &str) -> Target {
    Target {
        browser: Browser::Chrome,
        profile: profile.into(),
    }
}

#[test]
fn mixed_sources_are_scoped_and_each_failure_preserves_other_successes() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("managed");
    let mut manager = Manager::open(&root).unwrap();
    let local = temp.path().join("local");
    let missing = temp.path().join("missing");
    source(&local, "1");
    source(&missing, "1");
    let own = target("work");
    let other = target("personal");
    let p = manager.preview_local(&local).unwrap();
    let local_record = manager.install_local(p, own.clone()).unwrap();
    let p = manager.preview_local(&local).unwrap();
    let other_record = manager.install_local(p, other.clone()).unwrap();
    let p = manager.preview_local(&missing).unwrap();
    let missing_record = manager.install_local(p, own.clone()).unwrap();
    let gh = Sources { version: "1" };
    let p = manager
        .preview_repository(&gh, "owner/repo", "main", "dist")
        .unwrap();
    let repo_record = manager.install_local(p, own.clone()).unwrap();
    let p = manager
        .preview_release(&gh, "owner/repo", &ReleaseSelection::default())
        .unwrap()
        .select_root(None)
        .unwrap();
    let release_record = manager.install_local(p, own.clone()).unwrap();
    source(&local, "2");
    fs::remove_dir_all(&missing).unwrap();
    let checks = manager
        .check_target_updates(&Sources { version: "2" }, &own)
        .unwrap();
    assert_eq!(checks.len(), 4);
    assert!(checks.iter().all(|c| c.id() != other_record.id));
    assert!(
        checks
            .iter()
            .find(|c| c.id() == missing_record.id)
            .unwrap()
            .error()
            .is_some()
    );
    fs::write(
        repo_record.install_dir.join("manual.txt"),
        "preserve this conflict",
    )
    .unwrap();
    let results = manager.apply_target_upgrades(&own, checks);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r.status, BulkStatus::Deployed(_)))
            .count(),
        2
    );
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r.status, BulkStatus::Failed(_)))
            .count(),
        2
    );
    drop(manager);
    let manager = Manager::open(&root).unwrap();
    let history = manager.operation_history(&own).unwrap();
    for id in [
        &local_record.id,
        &release_record.id,
        &repo_record.id,
        &missing_record.id,
    ] {
        assert!(
            history
                .iter()
                .any(|r| r.installation_id.as_ref() == Some(id)
                    && r.stage == cepm_core::operations::OperationStage::Deploy)
        );
    }
    let records = manager.list().unwrap();
    for id in [local_record.id, release_record.id] {
        let record = records.iter().find(|r| r.id == id).unwrap();
        assert_eq!(record.installed_version, "2");
        assert_eq!(record.verification, Verification::Pending);
        assert_eq!(record.effective_version, None);
    }
    assert_eq!(
        records.iter().find(|r| r.id == other_record.id).unwrap(),
        &other_record
    );
    assert_eq!(
        fs::read_to_string(repo_record.install_dir.join("manual.txt")).unwrap(),
        "preserve this conflict"
    );
}

#[test]
fn batch_requires_same_target_and_skips_downgrades_and_same_version_redeployment() {
    let temp = tempfile::tempdir().unwrap();
    let mut manager = Manager::open(&temp.path().join("managed")).unwrap();
    let own = target("work");
    for (name, version) in [("upgrade", "1"), ("downgrade", "3"), ("redeploy", "2")] {
        let path = temp.path().join(name);
        source(&path, version);
        let preview = manager.preview_local(&path).unwrap();
        manager.install_local(preview, own.clone()).unwrap();
        source(&path, "2");
        fs::write(path.join("new.txt"), "new artifact").unwrap();
    }
    let checks = manager
        .check_target_updates(&Sources { version: "2" }, &own)
        .unwrap();
    let wrong_target = manager.apply_target_upgrades(&target("personal"), checks);
    assert!(
        wrong_target
            .iter()
            .all(|r| matches!(r.status, BulkStatus::Failed(_)))
    );
    let checks = manager
        .check_target_updates(&Sources { version: "2" }, &own)
        .unwrap();
    let results = manager.apply_target_upgrades(&own, checks);
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r.status, BulkStatus::Deployed(_)))
            .count(),
        1
    );
    assert_eq!(
        results
            .iter()
            .filter(|r| matches!(r.status, BulkStatus::Skipped(_)))
            .count(),
        2
    );
}
