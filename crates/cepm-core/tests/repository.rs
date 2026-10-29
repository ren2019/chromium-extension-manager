use cepm_core::{
    Browser, Manager, Target, Verification,
    remote::{Gh, GitHubRepo, RemoteOrigin},
};
use std::{
    fs,
    io::{Cursor, Write},
};
use tempfile::TempDir;
use zip::{ZipWriter, write::SimpleFileOptions};

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

struct RepositoryGh {
    reference: String,
    commit: String,
    archive: Vec<u8>,
}
impl Gh for RepositoryGh {
    fn run(&self, args: &[String], _max_bytes: usize) -> cepm_core::Result<Vec<u8>> {
        if args
            == [
                "api",
                "--hostname",
                "github.com",
                &format!("repos/private-owner/extension/commits/{}", self.reference),
            ]
        {
            return Ok(serde_json::to_vec(&serde_json::json!({"sha": self.commit})).unwrap());
        }
        if args
            == [
                "api",
                "--hostname",
                "github.com",
                &format!("repos/private-owner/extension/zipball/{}", self.commit),
            ]
        {
            return Ok(self.archive.clone());
        }
        Err("此引用不可访问，请检查 gh 授权或引用".into())
    }
}
fn archive(entries: &[(&str, &str)]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, content) in entries {
        writer
            .start_file(*name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(content.as_bytes()).unwrap();
    }
    writer.finish().unwrap().into_inner()
}
fn artifact(version: &str, body: &str) -> Vec<u8> {
    let manifest = serde_json::json!({
        "manifest_version": 3, "name": "Private extension", "version": version,
        "background": {"service_worker": "worker.js"}
    })
    .to_string();
    archive(&[
        (
            "generated-wrapper/packages/extension/manifest.json",
            &manifest,
        ),
        ("generated-wrapper/packages/extension/worker.js", body),
        (
            "generated-wrapper/package.json",
            "{\"scripts\":{\"build\":\"never execute\"}}",
        ),
    ])
}
fn target() -> Target {
    Target {
        browser: Browser::Chrome,
        profile: "isolated-profile".into(),
    }
}
fn origin(reference: &str, commit: &str) -> RemoteOrigin {
    RemoteOrigin::GitHubDirectory {
        repo: GitHubRepo::parse("private-owner/extension").unwrap(),
        reference: reference.into(),
        commit: commit.into(),
        subdirectory: "packages/extension".into(),
    }
}

#[test]
fn private_repository_installs_only_selected_complete_subdirectory_and_retains_ref_and_commit() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("managed");
    let mut manager = Manager::open(&root).unwrap();
    let gh = RepositoryGh {
        reference: "feature%2Fready".into(),
        commit: COMMIT.into(),
        archive: artifact("1.2", "ready"),
    };
    let preview = manager
        .preview_repository(
            &gh,
            "private-owner/extension",
            "feature/ready",
            "packages/extension",
        )
        .unwrap();
    assert_eq!(preview.name(), "Private extension");
    assert_eq!(preview.version(), "1.2");
    let record = manager.install_local(preview, target()).unwrap();
    assert_eq!(record.origin, Some(origin("feature/ready", COMMIT)));
    assert_eq!(
        fs::read_to_string(record.install_dir.join("worker.js")).unwrap(),
        "ready"
    );
    assert!(!record.install_dir.join("package.json").exists());
    assert_eq!(record.verification, Verification::Pending);
    drop(manager);
    assert_eq!(Manager::open(&root).unwrap().list().unwrap(), vec![record]);
}

#[test]
fn subdirectory_cannot_escape_downloaded_repository_even_to_another_valid_extension() {
    let temp = TempDir::new().unwrap();
    let outside = temp.path().join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(
        outside.join("manifest.json"),
        r#"{"manifest_version":3,"name":"Outside","version":"9"}"#,
    )
    .unwrap();
    let manager = Manager::open(&temp.path().join("managed")).unwrap();
    let gh = RepositoryGh {
        reference: "main".into(),
        commit: COMMIT.into(),
        archive: artifact("1", "safe"),
    };
    for path in [
        outside.to_str().unwrap(),
        "../packages/extension",
        "packages/../extension",
        "packages\\extension",
        "C:/extension",
        "packages//extension",
    ] {
        let error = manager
            .preview_repository(&gh, "private-owner/extension", "main", path)
            .err()
            .expect("unsafe subdirectory rejected");
        assert!(error.contains("子目录"), "{path}: {error}");
    }
    assert!(manager.list().unwrap().is_empty());
}

#[test]
fn moving_branch_requires_explicit_redeploy_and_load_failure_restores_commit_metadata() {
    use cepm_core::UpdateKind;
    let temp = TempDir::new().unwrap();
    let mut manager = Manager::open(&temp.path().join("managed")).unwrap();
    let mut gh = RepositoryGh {
        reference: "main".into(),
        commit: COMMIT.into(),
        archive: artifact("1", "original"),
    };
    let preview = manager
        .preview_repository(&gh, "private-owner/extension", "main", "packages/extension")
        .unwrap();
    let old = manager.install_local(preview, target()).unwrap();
    gh.commit = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into();
    gh.archive = artifact("1", "changed on branch");
    let plan = manager.check_repository(&gh, &old.id).unwrap();
    assert_eq!(plan.kind(), UpdateKind::Redeploy);
    assert_eq!(plan.origin(), Some(&origin("main", &gh.commit)));
    assert!(manager.apply_update(plan, UpdateKind::Upgrade).is_err());
    assert_eq!(
        fs::read_to_string(old.install_dir.join("worker.js")).unwrap(),
        "original"
    );
    let plan = manager.check_repository(&gh, &old.id).unwrap();
    let new = manager.apply_update(plan, UpdateKind::Redeploy).unwrap();
    assert_eq!(new.origin, Some(origin("main", &gh.commit)));
    assert_eq!(new.install_dir, old.install_dir);
    assert_eq!(new.verification, Verification::Pending);
    assert_eq!(
        fs::read_to_string(new.install_dir.join("worker.js")).unwrap(),
        "changed on branch"
    );
    let restored = manager
        .report_load_failure(&new.id, new.generation)
        .unwrap();
    assert_eq!(restored.origin, old.origin);
    assert_eq!(
        fs::read_to_string(restored.install_dir.join("worker.js")).unwrap(),
        "original"
    );
}

#[test]
fn unusable_repository_artifacts_leave_old_files_and_records_unchanged() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("managed");
    let mut manager = Manager::open(&root).unwrap();
    let mut gh = RepositoryGh {
        reference: "main".into(),
        commit: COMMIT.into(),
        archive: artifact("1", "keep"),
    };
    let old = manager
        .install_local(
            manager
                .preview_repository(&gh, "private-owner/extension", "main", "packages/extension")
                .unwrap(),
            target(),
        )
        .unwrap();
    let missing_worker = r#"{"manifest_version":3,"name":"Broken","version":"2","background":{"service_worker":"missing.js"}}"#;
    for bytes in [
        archive(&[("wrapper/src/index.ts", "source only")]),
        archive(&[("wrapper/packages/extension/manifest.json", missing_worker)]),
        archive(&[(
            "wrapper/other/manifest.json",
            r#"{"manifest_version":3,"name":"Wrong location","version":"2"}"#,
        )]),
        archive(&[("first/file", "one"), ("second/file", "two")]),
        archive(&[("manifest.json", "{}")]),
        archive(&[("wrapper/../escape", "danger")]),
        b"not a ZIP".to_vec(),
    ] {
        gh.archive = bytes;
        assert!(manager.check_repository(&gh, &old.id).is_err());
        assert_eq!(
            fs::read_to_string(old.install_dir.join("worker.js")).unwrap(),
            "keep"
        );
        assert_eq!(manager.list().unwrap(), vec![old.clone()]);
    }
    gh.reference = "not-the-saved-ref".into();
    assert!(
        manager
            .check_repository(&gh, &old.id)
            .err()
            .unwrap()
            .contains("访问权限")
    );
    drop(manager);
    assert_eq!(Manager::open(&root).unwrap().list().unwrap(), vec![old]);
}

#[test]
fn explicit_old_tag_can_be_selected_as_downgrade_and_its_reference_is_retained() {
    use cepm_core::UpdateKind;
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("managed");
    let mut manager = Manager::open(&root).unwrap();
    let mut gh = RepositoryGh {
        reference: "main".into(),
        commit: COMMIT.into(),
        archive: artifact("2", "latest"),
    };
    let old = manager
        .install_local(
            manager
                .preview_repository(&gh, "private-owner/extension", "main", "packages/extension")
                .unwrap(),
            target(),
        )
        .unwrap();
    gh.reference = "tags%2Fv1".into();
    gh.commit = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into();
    gh.archive = artifact("1", "older release");
    let preview = manager
        .preview_repository(
            &gh,
            "https://github.com/private-owner/extension",
            "tags/v1",
            "packages/extension",
        )
        .unwrap();
    let plan = manager.prepare_update(&old.id, preview).unwrap();
    assert_eq!(plan.kind(), UpdateKind::Downgrade);
    let new = manager.apply_update(plan, UpdateKind::Downgrade).unwrap();
    assert_eq!(new.origin, Some(origin("tags/v1", &gh.commit)));
    assert_eq!(new.installed_version, "1");
    assert_eq!(new.install_dir, old.install_dir);
    manager
        .confirm(
            &new.id,
            new.generation,
            "abcdefghijklmnopabcdefghijklmnop",
            "1",
        )
        .unwrap();
    drop(manager);
    let manager = Manager::open(&root).unwrap();
    let check = manager.check_repository(&gh, &new.id).unwrap();
    assert_eq!(check.kind(), UpdateKind::Unchanged);
    assert_eq!(check.origin(), Some(&origin("tags/v1", &gh.commit)));
}

#[test]
fn repository_root_and_full_commit_input_are_supported_without_guessing_an_extension_root() {
    let temp = TempDir::new().unwrap();
    let mut manager = Manager::open(&temp.path().join("managed")).unwrap();
    let manifest = r#"{"manifest_version":3,"name":"Root extension","version":"3"}"#;
    let gh = RepositoryGh {
        reference: COMMIT.into(),
        commit: COMMIT.into(),
        archive: archive(&[("wrapper/manifest.json", manifest)]),
    };
    let preview = manager
        .preview_repository(&gh, "private-owner/extension", COMMIT, ".")
        .unwrap();
    let record = manager.install_local(preview, target()).unwrap();
    assert_eq!(
        record.origin,
        Some(RemoteOrigin::GitHubDirectory {
            repo: GitHubRepo::parse("private-owner/extension").unwrap(),
            reference: COMMIT.into(),
            commit: COMMIT.into(),
            subdirectory: "".into()
        })
    );
    assert_eq!(
        manager.check_repository(&gh, &record.id).unwrap().kind(),
        cepm_core::UpdateKind::Unchanged
    );
}

#[test]
fn invalid_resolved_commits_and_input_refs_are_rejected_before_installation() {
    let temp = TempDir::new().unwrap();
    let manager = Manager::open(&temp.path().join("managed")).unwrap();
    let mut gh = RepositoryGh {
        reference: "main".into(),
        commit: "short-sha".into(),
        archive: artifact("1", "unused"),
    };
    for commit in [
        "short-sha",
        "gggggggggggggggggggggggggggggggggggggggg",
        "../../another",
    ] {
        gh.commit = commit.into();
        let error = manager
            .preview_repository(&gh, "private-owner/extension", "main", "packages/extension")
            .err()
            .unwrap();
        assert!(error.contains("提交标识"));
    }
    for reference in ["", "main\nsecret"] {
        assert!(
            manager
                .preview_repository(
                    &gh,
                    "private-owner/extension",
                    reference,
                    "packages/extension"
                )
                .is_err()
        );
    }
    assert!(manager.list().unwrap().is_empty());
}
