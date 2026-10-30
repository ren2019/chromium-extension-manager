use cepm_core::{
    Browser, Manager, Target,
    operations::{OperationStage, OperationStatus},
    remote::Gh,
};
use std::fs;
struct NoNetwork;
impl Gh for NoNetwork {
    fn run(&self, _: &[String], _: usize) -> cepm_core::Result<Vec<u8>> {
        panic!("local operation must not use gh")
    }
}
#[test]
fn failed_update_check_remains_diagnosable_after_reopening_without_changing_installation() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("managed");
    let source = temp.path().join("source");
    fs::create_dir(&source).unwrap();
    fs::write(
        source.join("manifest.json"),
        r#"{"manifest_version":3,"name":"Example","version":"1"}"#,
    )
    .unwrap();
    let target = Target {
        browser: Browser::Chrome,
        profile: "work".into(),
    };
    let mut manager = Manager::open(&root).unwrap();
    let preview = manager.preview_local(&source).unwrap();
    let record = manager.install_local(preview, target.clone()).unwrap();
    fs::remove_dir_all(&source).unwrap();
    assert!(manager.check_update(&NoNetwork, &record.id).is_err());
    drop(manager);
    let manager = Manager::open(&root).unwrap();
    assert_eq!(manager.list().unwrap(), vec![record.clone()]);
    let history = manager.operation_history(&target).unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(
        history[0].installation_id.as_deref(),
        Some(record.id.as_str())
    );
    assert_eq!(history[0].stage, OperationStage::Check);
    assert_eq!(history[0].status, OperationStatus::Failed);
    assert!(history[0].at > 0);
    assert!(!history[0].reason.is_empty());
    assert!(!history[0].next_step.is_empty());
    assert!(
        manager
            .operation_history(&Target {
                browser: Browser::Chrome,
                profile: "personal".into()
            })
            .unwrap()
            .is_empty()
    );
}

#[test]
fn failed_initial_source_is_saved_without_raw_secrets_and_history_is_bounded() {
    let temp = tempfile::tempdir().unwrap();
    let target = Target {
        browser: Browser::Chrome,
        profile: "work".into(),
    };
    let manager = Manager::open(temp.path()).unwrap();
    manager
        .record_operation(
            &target,
            None,
            OperationStage::Source,
            Err("gh access failed Authorization: Bearer secret-value Cookie: sensitive-value"),
        )
        .unwrap();
    drop(manager);
    let manager = Manager::open(temp.path()).unwrap();
    let history = manager.operation_history(&target).unwrap();
    assert_eq!(history[0].installation_id, None);
    assert_eq!(history[0].reason, "来源获取或访问失败");
    assert_eq!(
        history[0].next_step,
        "检查网络、gh auth status 和来源访问权限后重试"
    );
    let serialized = fs::read_to_string(temp.path().join("operations.json")).unwrap();
    assert!(!serialized.contains("secret-value"));
    assert!(!serialized.contains("sensitive-value"));
    for _ in 0..105 {
        manager
            .record_operation(
                &target,
                None,
                OperationStage::Source,
                Err("invalid manifest resource"),
            )
            .unwrap();
    }
    assert_eq!(manager.operation_history(&target).unwrap().len(), 100);
}

#[test]
fn invalid_manifest_diagnostics_request_a_valid_artifact_after_reopening() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("managed");
    let source = temp.path().join("source");
    fs::create_dir(&source).unwrap();
    let target = Target {
        browser: Browser::Chrome,
        profile: "work".into(),
    };
    for manifest in [
        r#"{"manifest_version":2,"name":"Example","version":"1"}"#,
        r#"{"manifest_version":3,"name":"Example","version":"01"}"#,
        r#"{"manifest_version":3,"name":"Example","version":"1","background":{"service_worker":"missing-secret-path.js"}}"#,
    ] {
        fs::write(source.join("manifest.json"), manifest).unwrap();
        let manager = Manager::open(&root).unwrap();
        let error = manager
            .preview_local(&source)
            .err()
            .expect("invalid artifact must fail");
        manager
            .record_operation(&target, None, OperationStage::Source, Err(&error))
            .unwrap();
        drop(manager);
        let manager = Manager::open(&root).unwrap();
        let history = manager.operation_history(&target).unwrap();
        assert_eq!(history[0].reason, "扩展清单或必要资源无效");
        assert_eq!(
            history[0].next_step,
            "选择完整的 Manifest V3 预构建产物后重试"
        );
        assert!(
            !fs::read_to_string(root.join("operations.json"))
                .unwrap()
                .contains("missing-secret-path")
        );
    }
}

#[test]
fn successful_history_only_requests_browser_reload_after_file_changes() {
    let temp = tempfile::tempdir().unwrap();
    let target = Target {
        browser: Browser::Edge,
        profile: "personal".into(),
    };
    for (stage, expected) in [
        (OperationStage::Confirm, "浏览器生效版本已确认"),
        (OperationStage::Check, "可在扩展详情中查看检查结果"),
        (OperationStage::Deploy, "磁盘变更仍需在目标浏览器重载并核对"),
        (
            OperationStage::Recover,
            "磁盘变更仍需在目标浏览器重载并核对",
        ),
    ] {
        let manager = Manager::open(temp.path()).unwrap();
        manager
            .record_operation(&target, None, stage, Ok(()))
            .unwrap();
        drop(manager);
        let manager = Manager::open(temp.path()).unwrap();
        let history = manager.operation_history(&target).unwrap();
        let event = history.iter().find(|event| event.stage == stage).unwrap();
        assert_eq!(event.status, OperationStatus::Succeeded);
        assert_eq!(event.next_step, expected, "{stage:?}");
    }
}

#[test]
fn legacy_success_guidance_is_projected_without_rewriting_history_or_custom_messages() {
    let temp = tempfile::tempdir().unwrap();
    let target = Target {
        browser: Browser::Edge,
        profile: "personal".into(),
    };
    let old = "磁盘变更仍需在目标浏览器重载并核对";
    let cases = [
        ("Confirm", "Succeeded", old, "浏览器生效版本已确认"),
        ("Check", "Succeeded", old, "可在扩展详情中查看检查结果"),
        ("Confirm", "Failed", old, old),
        ("Check", "Skipped", old, old),
        ("Deploy", "Succeeded", old, old),
        ("Recover", "Succeeded", old, old),
        ("Confirm", "Succeeded", "保留自定义提示", "保留自定义提示"),
        ("Check", "Succeeded", "另一个提示", "另一个提示"),
    ];
    drop(Manager::open(temp.path()).unwrap());
    let stored: Vec<_> = cases.iter().enumerate().map(|(index, (stage, status, text, _))| {
        serde_json::json!({"target":{"browser":"Edge","profile":"personal"},"installation_id":null,"at":index as u64 + 1,"stage":stage,"status":status,"reason":"操作已完成","next_step":text})
    }).collect();
    let bytes = serde_json::to_vec(&stored).unwrap();
    fs::write(temp.path().join("operations.json"), &bytes).unwrap();
    let manager = Manager::open(temp.path()).unwrap();
    let history = manager.operation_history(&target).unwrap();
    for (index, (_, _, _, expected)) in cases.iter().enumerate() {
        let event = history
            .iter()
            .find(|event| event.at == index as u64 + 1)
            .unwrap();
        assert_eq!(&event.next_step, expected);
    }
    assert_eq!(
        fs::read(temp.path().join("operations.json")).unwrap(),
        bytes
    );
}
