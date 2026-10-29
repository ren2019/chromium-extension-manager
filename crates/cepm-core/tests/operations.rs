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
