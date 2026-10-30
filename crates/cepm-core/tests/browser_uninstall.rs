use cepm_core::{
    Browser, Manager,
    browser::{AutomaticRemovalConfirmation, BrowserCancellation, EdgeSession},
    profiles::BrowserProfile,
};
use serde_json::{Value, json};
use std::{
    fs,
    net::TcpListener,
    sync::{Arc, Mutex},
    thread,
};
use tempfile::TempDir;

const ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
fn setup() -> (TempDir, BrowserProfile, Manager, cepm_core::Installation) {
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("edge");
    fs::create_dir_all(data.join("Default")).unwrap();
    fs::write(data.join("Default/Preferences"), "{}").unwrap();
    fs::write(
        data.join("Local State"),
        r#"{"profile":{"info_cache":{"Default":{}},"last_used":"Default"}}"#,
    )
    .unwrap();
    let app = root.path().join("edge-app");
    fs::write(&app, "").unwrap();
    let profile = BrowserProfile {
        id: "profile-one".into(),
        browser: Browser::Edge,
        name: "Disposable".into(),
        application: app,
        user_data_dir: data.canonicalize().unwrap(),
        profile_directory: "Default".into(),
    };
    let source = root.path().join("source");
    fs::create_dir(&source).unwrap();
    fs::write(
        source.join("manifest.json"),
        r#"{"manifest_version":3,"name":"Fixture","version":"1.0.0"}"#,
    )
    .unwrap();
    let mut manager = Manager::open(&root.path().join("cepm")).unwrap();
    let preview = manager.preview_local(&source).unwrap();
    let record = manager.install_local(preview, profile.target()).unwrap();
    let record = manager
        .confirm(&record.id, record.generation, ID, "1.0.0")
        .unwrap();
    (root, profile, manager, record)
}
fn serve(
    profile: &BrowserProfile,
    record: &cepm_core::Installation,
    mode: &str,
) -> (thread::JoinHandle<()>, Arc<Mutex<Value>>) {
    serve_with_cancel(profile, record, mode, None)
}
fn serve_with_cancel(
    profile: &BrowserProfile,
    record: &cepm_core::Installation,
    mode: &str,
    cancel_after_request: Option<BrowserCancellation>,
) -> (thread::JoinHandle<()>, Arc<Mutex<Value>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    fs::write(
        profile.user_data_dir.join("DevToolsActivePort"),
        format!(
            "{}\n/devtools/browser/test-session",
            listener.local_addr().unwrap().port()
        ),
    )
    .unwrap();
    let profile_path = profile.profile_path().display().to_string();
    let extensions = Arc::new(Mutex::new(
        json!([{"id":ID,"path":record.install_dir,"version":"1.0.0","enabled":true}]),
    ));
    let browser = extensions.clone();
    let mode = mode.to_string();
    let task = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut ws = tungstenite::accept(stream).unwrap();
        let mut removed = false;
        while let Ok(message) = ws.read() {
            if message.is_close() {
                break;
            }
            let Ok(text) = message.to_text() else {
                continue;
            };
            let request: Value = serde_json::from_str(text).unwrap();
            let mut response = json!({"id":request["id"]});
            if let Some(s) = request.get("sessionId") {
                response["sessionId"] = s.clone();
            }
            let result = match request["method"].as_str().unwrap() {
                "Browser.getVersion" => {
                    json!({"product":if mode=="unsupported"{"Edg/157.0.1.0"}else{"Edg/156.0.4314.50"},"userAgent":"Edg/156.0.4314.50"})
                }
                "Target.createTarget" => json!({"targetId":"version"}),
                "Target.attachToTarget" => json!({"sessionId":"version-session"}),
                "Runtime.evaluate" => {
                    json!({"result":{"value":{"url":"edge://version/","profilePath":profile_path}}})
                }
                "Extensions.getExtensions" => {
                    if removed && mode == "malformed-post" {
                        json!({"extensions":[{}]})
                    } else if removed && mode == "read-failed" {
                        response["error"] = json!({"code":-32000,"message":"Read failed"});
                        json!({})
                    } else {
                        json!({"extensions":*browser.lock().unwrap()})
                    }
                }
                "Extensions.uninstall" => {
                    assert_eq!(request["params"]["id"], ID);
                    if mode == "denied" {
                        response["error"] = json!({"code":-32000,"message":"denied"});
                    } else {
                        removed = true;
                        if let Some(cancel) = &cancel_after_request {
                            cancel.cancel();
                        }
                        if mode != "still-present" {
                            *browser.lock().unwrap() = json!([]);
                        }
                    }
                    json!({})
                }
                "Target.closeTarget" => json!({"success":true}),
                other => panic!("unexpected {other}"),
            };
            if response.get("error").is_none() {
                response["result"] = result;
            }
            if ws
                .send(tungstenite::Message::Text(response.to_string().into()))
                .is_err()
            {
                break;
            }
        }
    });
    (task, extensions)
}
#[cfg(feature = "edge-uninstall")]
#[test]
fn verified_removal_cleans_only_selected_installation_after_browser_absence() {
    let (_root, profile, mut manager, record) = setup();
    let other = manager
        .install_local(
            manager.preview_local(&record.source).unwrap(),
            cepm_core::Target {
                browser: Browser::Edge,
                profile: "another".into(),
            },
        )
        .unwrap();
    let (server, browser) = serve(&profile, &record, "success");
    let session = EdgeSession::connect(&profile, BrowserCancellation::default()).unwrap();
    manager
        .uninstall_edge(
            &record.id,
            &profile.target(),
            record.generation,
            session,
            AutomaticRemovalConfirmation::Confirmed,
        )
        .unwrap();
    assert_eq!(*browser.lock().unwrap(), json!([]));
    assert!(!record.install_dir.exists());
    assert!(record.source.join("manifest.json").exists());
    assert_eq!(manager.list().unwrap(), vec![other]);
    assert!(
        manager.operation_history(&profile.target()).unwrap()[0]
            .reason
            .contains("自动")
    );
    server.join().unwrap();
}

#[cfg(not(feature = "edge-uninstall"))]
#[test]
fn normal_build_cannot_issue_an_unvalidated_automatic_write() {
    let (_root, profile, mut manager, record) = setup();
    let (server, browser) = serve(&profile, &record, "success");
    let session = EdgeSession::connect(&profile, BrowserCancellation::default()).unwrap();
    assert!(!session.capabilities().uninstall);
    assert!(
        manager
            .uninstall_edge(
                &record.id,
                &profile.target(),
                record.generation,
                session,
                AutomaticRemovalConfirmation::Confirmed
            )
            .is_err()
    );
    assert_eq!(browser.lock().unwrap().as_array().unwrap().len(), 1);
    assert_eq!(manager.list().unwrap(), vec![record]);
    server.join().unwrap();
}

#[cfg(feature = "edge-uninstall")]
#[test]
fn denied_unknown_or_unchanged_browser_result_never_authorizes_cleanup() {
    for mode in [
        "denied",
        "read-failed",
        "still-present",
        "malformed-post",
        "unsupported",
    ] {
        let (_root, profile, mut manager, record) = setup();
        let (server, _browser) = serve(&profile, &record, mode);
        let session = EdgeSession::connect(&profile, BrowserCancellation::default()).unwrap();
        assert!(
            manager
                .uninstall_edge(
                    &record.id,
                    &profile.target(),
                    record.generation,
                    session,
                    AutomaticRemovalConfirmation::Confirmed
                )
                .is_err(),
            "{mode}"
        );
        assert_eq!(manager.list().unwrap(), vec![record.clone()], "{mode}");
        assert!(record.install_dir.join("manifest.json").exists());
        assert!(
            manager
                .operation_history(&profile.target())
                .unwrap()
                .is_empty()
        );
        server.join().unwrap();
    }
}

#[cfg(feature = "edge-uninstall")]
#[test]
fn missing_identity_wrong_target_stale_deployment_and_cancellation_never_write() {
    for mode in [
        "missing-id",
        "wrong-target",
        "stale",
        "cancelled",
        "not-confirmed",
        "shared-root",
        "wrong-browser-id",
    ] {
        let (_root, profile, mut manager, mut record) = setup();
        if mode == "missing-id" {
            record = manager
                .install_local(
                    manager.preview_local(&record.source).unwrap(),
                    profile.target(),
                )
                .unwrap();
        }
        let before = manager.list().unwrap();
        let (server, browser) = serve(&profile, &record, "success");
        if mode == "wrong-browser-id" {
            browser.lock().unwrap()[0]["id"] = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into();
        }
        let cancel = BrowserCancellation::default();
        let session = EdgeSession::connect(&profile, cancel.clone()).unwrap();
        if mode == "cancelled" {
            cancel.cancel();
        }
        if mode == "shared-root" {
            fs::create_dir(profile.user_data_dir.join("Profile 1")).unwrap();
            fs::write(profile.user_data_dir.join("Profile 1/Preferences"), "{}").unwrap();
        }
        let mut target = profile.target();
        if mode == "wrong-target" {
            target.profile = "other".into();
        }
        let generation = if mode == "stale" {
            record.generation + 1
        } else {
            record.generation
        };
        let confirmation = if mode == "not-confirmed" {
            AutomaticRemovalConfirmation::NotConfirmed
        } else {
            AutomaticRemovalConfirmation::Confirmed
        };
        assert!(
            manager
                .uninstall_edge(&record.id, &target, generation, session, confirmation)
                .is_err(),
            "{mode}"
        );
        assert_eq!(
            browser.lock().unwrap().as_array().unwrap().len(),
            1,
            "{mode}"
        );
        assert_eq!(manager.list().unwrap(), before, "{mode}");
        server.join().unwrap();
    }
}

#[cfg(feature = "edge-uninstall")]
#[test]
fn browser_removed_but_cleanup_failed_is_persisted_and_retried_without_browser() {
    let (root, profile, mut manager, record) = setup();
    let (server, browser) = serve(&profile, &record, "success");
    let session = EdgeSession::connect(&profile, BrowserCancellation::default()).unwrap();
    let parked = root.path().join("parked-installation");
    let original = record.install_dir.clone();
    let result = manager.uninstall_edge_with_progress(
        &record.id,
        &profile.target(),
        record.generation,
        session,
        AutomaticRemovalConfirmation::Confirmed,
        |phase| {
            if phase == cepm_core::RemovalPhase::BrowserRemoved {
                fs::rename(&original, &parked).unwrap();
                fs::write(&original, "cleanup conflict").unwrap();
            }
        },
    );
    assert!(result.is_err());
    assert_eq!(*browser.lock().unwrap(), json!([]));
    let retained = manager.list().unwrap().remove(0);
    assert_eq!(
        retained.removal.as_ref().unwrap().phase(),
        cepm_core::RemovalPhase::BrowserRemoved
    );
    assert!(retained.removal.as_ref().unwrap().error().is_some());
    drop(manager);
    server.join().unwrap();
    fs::remove_file(&original).unwrap();
    fs::rename(&parked, &original).unwrap();
    let mut manager = Manager::open(&root.path().join("cepm")).unwrap();
    manager
        .retry_removal(&record.id, &profile.target(), record.generation)
        .unwrap();
    assert!(manager.list().unwrap().is_empty());
    assert!(record.source.join("manifest.json").exists());
    let history = manager.operation_history(&profile.target()).unwrap();
    assert_eq!(history.len(), 1);
    assert!(history[0].reason.contains("自动"));
}

#[cfg(feature = "edge-uninstall")]
#[test]
fn cancelled_after_browser_request_does_not_claim_absence_or_clean_files() {
    let (_root, profile, mut manager, record) = setup();
    let cancel = BrowserCancellation::default();
    let (server, browser) = serve_with_cancel(&profile, &record, "success", Some(cancel.clone()));
    let session = EdgeSession::connect(&profile, cancel).unwrap();
    assert!(
        manager
            .uninstall_edge(
                &record.id,
                &profile.target(),
                record.generation,
                session,
                AutomaticRemovalConfirmation::Confirmed
            )
            .is_err()
    );
    assert_eq!(*browser.lock().unwrap(), json!([]));
    assert_eq!(manager.list().unwrap(), vec![record.clone()]);
    assert!(record.install_dir.exists());
    server.join().unwrap();
}
