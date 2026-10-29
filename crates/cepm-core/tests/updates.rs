use cepm_core::{Browser, Manager, Target, UpdateKind, Verification};
use std::{fs, path::Path};
use tempfile::TempDir;

const ID: &str = "abcdefghijklmnopabcdefghijklmnop";
fn artifact(path: &Path, version: &str, body: &str) {
    fs::create_dir_all(path).unwrap();
    fs::write(
        path.join("manifest.json"),
        serde_json::to_vec(&serde_json::json!({
            "manifest_version": 3, "name": "Update fixture", "version": version,
            "background": {"service_worker": "worker.js"}
        }))
        .unwrap(),
    )
    .unwrap();
    fs::write(path.join("worker.js"), body).unwrap();
}
fn target() -> Target {
    Target {
        browser: Browser::Chrome,
        profile: "test".into(),
    }
}

#[test]
fn upgrade_preserves_identity_and_requires_fresh_browser_confirmation_after_restart() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source");
    let root = temp.path().join("managed");
    artifact(&source, "1.9", "old");
    let mut manager = Manager::open(&root).unwrap();
    let old = manager
        .install_local(manager.preview_local(&source).unwrap(), target())
        .unwrap();
    manager.confirm(&old.id, old.generation, ID, "1.9").unwrap();
    artifact(&source, "1.10", "new");
    let plan = manager.check_local(&old.id).unwrap();
    assert_eq!(plan.kind(), UpdateKind::Upgrade);
    assert_eq!(plan.version(), "1.10");
    let new = manager.apply_update(plan, UpdateKind::Upgrade).unwrap();
    assert_eq!(new.id, old.id);
    assert_eq!(new.install_dir, old.install_dir);
    assert_eq!(new.extension_id.as_deref(), Some(ID));
    assert_eq!(new.verification, Verification::Pending);
    assert_eq!(new.effective_version, None);
    assert_eq!(new.historical_version.as_deref(), Some("1.9"));
    assert_eq!(
        fs::read_to_string(new.install_dir.join("worker.js")).unwrap(),
        "new"
    );
    assert!(
        manager
            .confirm(&new.id, old.generation, ID, "1.10")
            .is_err()
    );
    drop(manager);
    let mut manager = Manager::open(&root).unwrap();
    assert_eq!(manager.list().unwrap(), vec![new.clone()]);
    let confirmed = manager
        .confirm(&new.id, new.generation, ID, "1.10")
        .unwrap();
    assert_eq!(confirmed.verification, Verification::UserConfirmed);
}

#[test]
fn manual_file_changes_pause_deployment_and_confirmation_without_overwriting() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source");
    artifact(&source, "1", "old");
    let mut manager = Manager::open(&temp.path().join("managed")).unwrap();
    let old = manager
        .install_local(manager.preview_local(&source).unwrap(), target())
        .unwrap();
    artifact(&source, "2", "new");
    let plan = manager.check_local(&old.id).unwrap();
    fs::write(old.install_dir.join("worker.js"), "user edit").unwrap();
    assert!(
        manager
            .apply_update(plan, UpdateKind::Upgrade)
            .unwrap_err()
            .contains("冲突")
    );
    assert!(manager.confirm(&old.id, old.generation, ID, "1").is_err());
    assert_eq!(
        fs::read_to_string(old.install_dir.join("worker.js")).unwrap(),
        "user edit"
    );
    assert_eq!(manager.list().unwrap()[0].installed_version, "1");
    assert!(manager.check_local(&old.id).is_err());
}

#[test]
fn same_version_redeployment_and_downgrade_require_the_matching_explicit_choice() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source");
    artifact(&source, "2", "old");
    let mut manager = Manager::open(&temp.path().join("managed")).unwrap();
    let original = manager
        .install_local(manager.preview_local(&source).unwrap(), target())
        .unwrap();
    assert_eq!(
        manager.check_local(&original.id).unwrap().kind(),
        UpdateKind::Unchanged
    );
    artifact(&source, "2", "changed");
    assert_eq!(
        manager.check_local(&original.id).unwrap().kind(),
        UpdateKind::Redeploy
    );
    let plan = manager.check_local(&original.id).unwrap();
    assert!(manager.apply_update(plan, UpdateKind::Upgrade).is_err());
    let redeployed = manager
        .apply_update(
            manager.check_local(&original.id).unwrap(),
            UpdateKind::Redeploy,
        )
        .unwrap();
    assert!(
        manager
            .confirm(&original.id, original.generation, ID, "2")
            .is_err()
    );
    manager
        .confirm(&original.id, redeployed.generation, ID, "2")
        .unwrap();
    let old_source = temp.path().join("old-version");
    artifact(&old_source, "1", "downgraded");
    let plan = manager
        .prepare_update(&original.id, manager.preview_local(&old_source).unwrap())
        .unwrap();
    assert_eq!(plan.kind(), UpdateKind::Downgrade);
    assert!(manager.apply_update(plan, UpdateKind::Upgrade).is_err());
    let plan = manager
        .prepare_update(&original.id, manager.preview_local(&old_source).unwrap())
        .unwrap();
    let downgraded = manager.apply_update(plan, UpdateKind::Downgrade).unwrap();
    assert_eq!(downgraded.installed_version, "1");
    assert_eq!(downgraded.source, old_source.canonicalize().unwrap());
    assert_eq!(downgraded.verification, Verification::Pending);
    assert_eq!(
        fs::read_to_string(downgraded.install_dir.join("worker.js")).unwrap(),
        "downgraded"
    );
}

#[test]
fn reported_loading_failure_restores_previous_files_then_requires_another_confirmation() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source");
    let root = temp.path().join("managed");
    artifact(&source, "1", "old");
    let mut manager = Manager::open(&root).unwrap();
    let old = manager
        .install_local(manager.preview_local(&source).unwrap(), target())
        .unwrap();
    manager.confirm(&old.id, old.generation, ID, "1").unwrap();
    artifact(&source, "2", "broken in browser");
    let deployed = manager
        .apply_update(manager.check_local(&old.id).unwrap(), UpdateKind::Upgrade)
        .unwrap();
    drop(manager);
    let mut manager = Manager::open(&root).unwrap();
    assert!(
        manager
            .report_load_failure(&old.id, old.generation)
            .is_err()
    );
    let restored = manager
        .report_load_failure(&old.id, deployed.generation)
        .unwrap();
    assert_eq!(restored.install_dir, old.install_dir);
    assert_eq!(restored.installed_version, "1");
    assert_eq!(restored.extension_id.as_deref(), Some(ID));
    assert_eq!(restored.verification, Verification::Pending);
    assert_eq!(restored.effective_version, None);
    assert_eq!(
        fs::read_to_string(restored.install_dir.join("worker.js")).unwrap(),
        "old"
    );
    assert!(manager.confirm(&old.id, old.generation, ID, "1").is_err());
    manager
        .confirm(&old.id, restored.generation, ID, "1")
        .unwrap();
    drop(manager);
    let manager = Manager::open(&root).unwrap();
    assert_eq!(
        manager.list().unwrap()[0].verification,
        Verification::UserConfirmed
    );
}

#[test]
fn interrupted_deployments_recover_files_and_records_on_reopen() {
    use cepm_core::UpdateStage;
    for stop_at in [
        UpdateStage::Prepared,
        UpdateStage::BackedUp,
        UpdateStage::FilesDeployed,
        UpdateStage::RecordSaved,
    ] {
        let temp = TempDir::new().unwrap();
        let source = temp.path().join("source");
        let root = temp.path().join("managed");
        artifact(&source, "1", "old");
        let mut manager = Manager::open(&root).unwrap();
        let old = manager
            .install_local(manager.preview_local(&source).unwrap(), target())
            .unwrap();
        manager.confirm(&old.id, old.generation, ID, "1").unwrap();
        artifact(&source, "2", "new");
        let plan = manager.check_local(&old.id).unwrap();
        let stopped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            manager
                .apply_update_with_progress(plan, UpdateKind::Upgrade, |stage| {
                    if stage == stop_at {
                        panic!("simulated process interruption");
                    }
                })
                .unwrap();
        }));
        assert!(stopped.is_err());
        drop(manager);
        let mut manager = Manager::open(&root).unwrap();
        let restored = manager.list().unwrap().remove(0);
        assert_eq!(restored.installed_version, "1", "{stop_at:?}");
        assert_eq!(restored.verification, Verification::Pending);
        assert_eq!(restored.effective_version, None);
        assert_eq!(
            fs::read_to_string(restored.install_dir.join("worker.js")).unwrap(),
            "old"
        );
        assert!(manager.confirm(&old.id, old.generation, ID, "1").is_err());
        manager
            .confirm(&old.id, restored.generation, ID, "1")
            .unwrap();
        manager
            .apply_update(manager.check_local(&old.id).unwrap(), UpdateKind::Upgrade)
            .unwrap();
    }
}

#[test]
#[cfg(unix)]
fn record_write_failure_restores_files_and_reports_unfinished_recovery_until_repaired() {
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source");
    let root = temp.path().join("managed");
    artifact(&source, "1", "old");
    let mut manager = Manager::open(&root).unwrap();
    let old = manager
        .install_local(manager.preview_local(&source).unwrap(), target())
        .unwrap();
    manager.confirm(&old.id, old.generation, ID, "1").unwrap();
    artifact(&source, "2", "new");
    let plan = manager.check_local(&old.id).unwrap();
    let records_dir = root.join("records");
    fs::set_permissions(&records_dir, fs::Permissions::from_mode(0o500)).unwrap();
    let result = manager.apply_update(plan, UpdateKind::Upgrade);
    fs::set_permissions(&records_dir, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.unwrap_err().contains("恢复需要处理"));
    assert_eq!(
        fs::read_to_string(old.install_dir.join("worker.js")).unwrap(),
        "old"
    );
    assert!(manager.list().unwrap()[0].recovery_required);
    drop(manager);
    let manager = Manager::open(&root).unwrap();
    let restored = manager.list().unwrap().remove(0);
    assert_eq!(restored.installed_version, "1");
    assert_eq!(restored.verification, Verification::Pending);
    assert!(restored.last_operation.contains("恢复"));
}

#[test]
#[cfg(unix)]
fn replacing_installation_with_a_symlink_is_a_conflict_even_when_contents_match() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source");
    artifact(&source, "1", "old");
    let mut manager = Manager::open(&temp.path().join("managed")).unwrap();
    let old = manager
        .install_local(manager.preview_local(&source).unwrap(), target())
        .unwrap();
    let moved = temp.path().join("user-files");
    fs::rename(&old.install_dir, &moved).unwrap();
    std::os::unix::fs::symlink(&moved, &old.install_dir).unwrap();
    artifact(&source, "2", "new");
    assert!(manager.check_local(&old.id).is_err());
    assert!(manager.confirm(&old.id, old.generation, ID, "1").is_err());
    assert_eq!(fs::read_to_string(moved.join("worker.js")).unwrap(), "old");
}

#[test]
#[cfg(unix)]
fn failed_replacement_blocks_only_its_installation_and_can_recover_on_restart() {
    use cepm_core::UpdateStage;
    use std::os::unix::fs::PermissionsExt;
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source");
    let root = temp.path().join("managed");
    artifact(&source, "1", "old");
    let mut manager = Manager::open(&root).unwrap();
    let old = manager
        .install_local(manager.preview_local(&source).unwrap(), target())
        .unwrap();
    let independent = manager
        .install_local(manager.preview_local(&source).unwrap(), target())
        .unwrap();
    artifact(&source, "2", "new");
    let plan = manager.check_local(&old.id).unwrap();
    let installations_dir = root.join("installations");
    let error = manager
        .apply_update_with_progress(plan, UpdateKind::Upgrade, |stage| {
            if stage == UpdateStage::BackedUp {
                fs::set_permissions(&installations_dir, fs::Permissions::from_mode(0o500)).unwrap();
            }
        })
        .unwrap_err();
    fs::set_permissions(&installations_dir, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(error.contains("恢复需要处理"));
    let records = manager.list().unwrap();
    assert!(
        records
            .iter()
            .find(|r| r.id == old.id)
            .unwrap()
            .recovery_required
    );
    assert!(
        !records
            .iter()
            .find(|r| r.id == independent.id)
            .unwrap()
            .recovery_required
    );
    manager
        .confirm(&independent.id, independent.generation, ID, "1")
        .unwrap();
    assert!(manager.confirm(&old.id, old.generation, ID, "1").is_err());
    drop(manager);
    let manager = Manager::open(&root).unwrap();
    let records = manager.list().unwrap();
    let restored = records.iter().find(|r| r.id == old.id).unwrap();
    assert!(!restored.recovery_required);
    assert_eq!(restored.installed_version, "1");
    assert_eq!(restored.verification, Verification::Pending);
    assert_eq!(
        fs::read_to_string(restored.install_dir.join("worker.js")).unwrap(),
        "old"
    );
}

#[test]
#[cfg(unix)]
fn literal_backslash_names_cannot_hide_content_changes() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source");
    artifact(&source, "1", "old");
    fs::create_dir(source.join("a")).unwrap();
    fs::write(source.join("a\\b"), "payload").unwrap();
    let mut manager = Manager::open(&temp.path().join("managed")).unwrap();
    let installed = manager
        .install_local(manager.preview_local(&source).unwrap(), target())
        .unwrap();
    fs::remove_file(source.join("a\\b")).unwrap();
    fs::write(source.join("a/b"), "payload").unwrap();
    assert_eq!(
        manager.check_local(&installed.id).unwrap().kind(),
        UpdateKind::Redeploy
    );
}

#[test]
fn damaged_backup_does_not_replace_current_files_and_remains_visibly_unresolved() {
    let temp = TempDir::new().unwrap();
    let source = temp.path().join("source");
    let root = temp.path().join("managed");
    artifact(&source, "1", "old");
    let mut manager = Manager::open(&root).unwrap();
    let old = manager
        .install_local(manager.preview_local(&source).unwrap(), target())
        .unwrap();
    artifact(&source, "2", "new");
    let updated = manager
        .apply_update(manager.check_local(&old.id).unwrap(), UpdateKind::Upgrade)
        .unwrap();
    fs::write(
        root.join("transactions")
            .join(&old.id)
            .join("old/worker.js"),
        "damaged backup",
    )
    .unwrap();
    assert!(
        manager
            .report_load_failure(&old.id, updated.generation)
            .is_err()
    );
    assert_eq!(
        fs::read_to_string(old.install_dir.join("worker.js")).unwrap(),
        "new"
    );
    drop(manager);
    let manager = Manager::open(&root).unwrap();
    assert!(manager.list().unwrap()[0].recovery_required);
    assert_eq!(
        fs::read_to_string(old.install_dir.join("worker.js")).unwrap(),
        "new"
    );
}
