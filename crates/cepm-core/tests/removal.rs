use cepm_core::{Browser, Manager, ManualRemovalConfirmation, Target};
use std::{fs, path::Path};
fn artifact(path: &Path, version: &str) {
    fs::create_dir_all(path).unwrap();
    fs::write(
        path.join("manifest.json"),
        format!(r#"{{"manifest_version":3,"name":"Shared fixture","version":"{version}"}}"#),
    )
    .unwrap();
}
fn target(profile: &str) -> Target {
    Target {
        browser: Browser::Edge,
        profile: profile.into(),
    }
}

#[test]
fn manual_removal_requires_explicit_confirmation_and_only_cleans_the_selected_installation() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let root = temp.path().join("managed");
    artifact(&source, "1");
    let mut manager = Manager::open(&root).unwrap();
    let first = manager
        .install_local(manager.preview_local(&source).unwrap(), target("one"))
        .unwrap();
    let other = manager
        .install_local(manager.preview_local(&source).unwrap(), target("two"))
        .unwrap();
    manager
        .confirm_manual(&first.id, &first.target, first.generation, "1")
        .unwrap();
    assert!(
        manager
            .confirm_manual_removal(
                &first.id,
                &first.target,
                first.generation,
                ManualRemovalConfirmation::NotRemoved
            )
            .is_err()
    );
    assert!(first.install_dir.exists());
    assert_eq!(manager.list().unwrap().len(), 2);
    assert!(
        manager
            .confirm_manual_removal(
                &first.id,
                &other.target,
                first.generation,
                ManualRemovalConfirmation::Removed
            )
            .is_err()
    );
    assert!(
        manager
            .confirm_manual_removal(
                &first.id,
                &first.target,
                first.generation + 1,
                ManualRemovalConfirmation::Removed
            )
            .is_err()
    );
    manager
        .confirm_manual_removal(
            &first.id,
            &first.target,
            first.generation,
            ManualRemovalConfirmation::Removed,
        )
        .unwrap();
    assert!(!first.install_dir.exists());
    assert!(source.join("manifest.json").exists());
    assert_eq!(manager.list().unwrap(), vec![other.clone()]);
    assert!(other.install_dir.join("manifest.json").exists());
    drop(manager);
    let manager = Manager::open(&root).unwrap();
    let history = manager.operation_history(&first.target).unwrap();
    assert_eq!(history.len(), 1);
    assert!(history[0].reason.contains("用户确认"));
    assert_eq!(
        history[0].installation_id.as_deref(),
        Some(first.id.as_str())
    );
}

#[test]
#[cfg(unix)]
fn cleanup_rejects_symlink_ancestors_before_deleting_any_external_files() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let root = temp.path().join("managed");
    artifact(&source, "1");
    let mut manager = Manager::open(&root).unwrap();
    let record = manager
        .install_local(manager.preview_local(&source).unwrap(), target("one"))
        .unwrap();
    let outside = temp.path().join("outside");
    fs::rename(root.join("installations"), &outside).unwrap();
    std::os::unix::fs::symlink(&outside, root.join("installations")).unwrap();
    assert!(
        manager
            .confirm_manual_removal(
                &record.id,
                &record.target,
                record.generation,
                ManualRemovalConfirmation::Removed
            )
            .is_err()
    );
    assert!(outside.join(&record.id).join("manifest.json").exists());
    assert!(
        root.join("records")
            .join(format!("{}.json", record.id))
            .exists()
    );
}

#[test]
fn removal_interruption_never_resurrects_files_and_retries_without_browser_confirmation() {
    use cepm_core::{RemovalPhase, UpdateKind};
    for stop_at in [
        RemovalPhase::BrowserRemoved,
        RemovalPhase::FilesRemoved,
        RemovalPhase::BackupsRemoved,
        RemovalPhase::HistoryRecorded,
    ] {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let root = temp.path().join("managed");
        artifact(&source, "1");
        let mut manager = Manager::open(&root).unwrap();
        let first = manager
            .install_local(manager.preview_local(&source).unwrap(), target("one"))
            .unwrap();
        artifact(&source, "2");
        let updated = manager
            .apply_update(manager.check_local(&first.id).unwrap(), UpdateKind::Upgrade)
            .unwrap();
        artifact(&source, "3");
        let pending = manager
            .apply_update(manager.check_local(&first.id).unwrap(), UpdateKind::Upgrade)
            .unwrap();
        assert!(pending.generation > updated.generation);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                manager
                    .confirm_manual_removal_with_progress(
                        &pending.id,
                        &pending.target,
                        pending.generation,
                        ManualRemovalConfirmation::Removed,
                        |phase| {
                            if phase == stop_at {
                                panic!("simulated interruption");
                            }
                        },
                    )
                    .unwrap();
            }))
            .is_err()
        );
        let had_files = pending.install_dir.exists();
        drop(manager);
        let mut manager = Manager::open(&root).unwrap();
        let remaining = manager.list().unwrap().remove(0);
        assert_eq!(remaining.removal.as_ref().unwrap().phase(), stop_at);
        assert_eq!(
            remaining.install_dir.exists(),
            had_files,
            "update recovery resurrected deleted files"
        );
        assert!(!remaining.recovery_required);
        assert!(
            manager
                .confirm_manual(&remaining.id, &remaining.target, remaining.generation, "3")
                .is_err()
        );
        assert!(manager.check_local(&remaining.id).is_err());
        manager
            .retry_removal(&remaining.id, &remaining.target, remaining.generation)
            .unwrap();
        assert!(manager.list().unwrap().is_empty());
        assert!(!root.join("transactions").join(&remaining.id).exists());
        assert!(source.join("manifest.json").exists());
        assert_eq!(
            manager.operation_history(&remaining.target).unwrap().len(),
            1
        );
    }
}

#[test]
#[cfg(unix)]
fn partial_cleanup_failure_is_persisted_and_can_resume_after_permissions_are_repaired() {
    use cepm_core::{RemovalPhase, UpdateKind};
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let root = temp.path().join("managed");
    artifact(&source, "1");
    let mut manager = Manager::open(&root).unwrap();
    let first = manager
        .install_local(manager.preview_local(&source).unwrap(), target("one"))
        .unwrap();
    artifact(&source, "2");
    let pending = manager
        .apply_update(manager.check_local(&first.id).unwrap(), UpdateKind::Upgrade)
        .unwrap();
    let transaction = root.join("transactions");
    fs::set_permissions(&transaction, fs::Permissions::from_mode(0o500)).unwrap();
    let result = manager.confirm_manual_removal(
        &pending.id,
        &pending.target,
        pending.generation,
        ManualRemovalConfirmation::Removed,
    );
    fs::set_permissions(&transaction, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.is_err());
    let remaining = manager.list().unwrap().remove(0);
    let removal = remaining.removal.as_ref().unwrap();
    assert_eq!(removal.phase(), RemovalPhase::FilesRemoved);
    assert!(removal.error().is_some());
    assert!(!remaining.install_dir.exists());
    drop(manager);
    let mut manager = Manager::open(&root).unwrap();
    assert!(manager.list().unwrap()[0].removal.is_some());
    manager
        .retry_removal(&remaining.id, &remaining.target, remaining.generation)
        .unwrap();
    assert!(manager.list().unwrap().is_empty());
}

#[test]
#[cfg(unix)]
fn history_is_not_duplicated_if_record_checkpoint_fails_after_cleanup() {
    use cepm_core::RemovalPhase;
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let root = temp.path().join("managed");
    artifact(&source, "1");
    let mut manager = Manager::open(&root).unwrap();
    let record = manager
        .install_local(manager.preview_local(&source).unwrap(), target("one"))
        .unwrap();
    let records = root.join("records");
    let result = manager.confirm_manual_removal_with_progress(
        &record.id,
        &record.target,
        record.generation,
        ManualRemovalConfirmation::Removed,
        |phase| {
            if phase == RemovalPhase::BackupsRemoved {
                fs::set_permissions(&records, fs::Permissions::from_mode(0o500)).unwrap();
            }
        },
    );
    fs::set_permissions(&records, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(result.is_err());
    assert_eq!(manager.operation_history(&record.target).unwrap().len(), 1);
    drop(manager);
    let mut manager = Manager::open(&root).unwrap();
    assert_eq!(
        manager.list().unwrap()[0].removal.as_ref().unwrap().phase(),
        RemovalPhase::BackupsRemoved
    );
    manager
        .retry_removal(&record.id, &record.target, record.generation)
        .unwrap();
    assert!(manager.list().unwrap().is_empty());
    assert_eq!(manager.operation_history(&record.target).unwrap().len(), 1);
}
