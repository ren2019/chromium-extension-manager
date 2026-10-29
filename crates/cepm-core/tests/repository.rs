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
