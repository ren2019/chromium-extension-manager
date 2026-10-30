use cepm_core::{
    Browser,
    browser::{BrowserCancellation, EdgeSession},
    profiles::BrowserProfile,
};
use std::fs;
use tempfile::TempDir;

fn profile(root: &TempDir) -> BrowserProfile {
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
    BrowserProfile {
        id: "test-profile".into(),
        browser: Browser::Edge,
        name: "Test".into(),
        application: app,
        user_data_dir: data.canonicalize().unwrap(),
        profile_directory: "Default".into(),
    }
}

#[test]
fn shared_profile_root_is_rejected_without_contacting_a_browser() {
    let root = tempfile::tempdir().unwrap();
    let profile = profile(&root);
    fs::write(
        profile.user_data_dir.join("Local State"),
        r#"{"profile":{"info_cache":{"Default":{},"Profile 1":{}},"last_used":"Default"}}"#,
    )
    .unwrap();
    let error = EdgeSession::connect(&profile, BrowserCancellation::default())
        .err()
        .unwrap();
    assert!(error.contains("多个配置"), "{error}");
}

use cepm_core::{Manager, Verification};
use serde_json::{Value, json};
use std::{net::TcpListener, thread};

fn serve(
    profile: &BrowserProfile,
    entries: Value,
    path_override: Option<String>,
) -> thread::JoinHandle<()> {
    serve_delayed(
        profile,
        entries,
        path_override,
        std::time::Duration::ZERO,
        0,
    )
}
fn serve_delayed(
    profile: &BrowserProfile,
    entries: Value,
    path_override: Option<String>,
    authorization_delay: std::time::Duration,
    mut loading_reads: u32,
) -> thread::JoinHandle<()> {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    fs::write(
        profile.user_data_dir.join("DevToolsActivePort"),
        format!(
            "{}\n/devtools/browser/test-session",
            listener.local_addr().unwrap().port()
        ),
    )
    .unwrap();
    let path = path_override.unwrap_or_else(|| profile.profile_path().display().to_string());
    thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        thread::sleep(authorization_delay);
        let mut ws = tungstenite::accept(stream).unwrap();
        while let Ok(message) = ws.read() {
            if message.is_close() {
                break;
            }
            let Ok(text) = message.to_text() else {
                continue;
            };
            let request: Value = serde_json::from_str(text).unwrap();
            let result = match request["method"].as_str().unwrap() {
                "Browser.getVersion" => {
                    json!({"product":"Edg/156.0.4314.50","userAgent":"Edg/156.0.4314.50"})
                }
                "Target.createTarget" => json!({"targetId":"version-page"}),
                "Target.attachToTarget" => json!({"sessionId":"version-session"}),
                "Runtime.evaluate" if loading_reads > 0 => {
                    loading_reads -= 1;
                    json!({"result":{"value":{"url":"edge://version/"}}})
                }
                "Runtime.evaluate" => {
                    json!({"result":{"value":{"url":"edge://version/", "profilePath":path}}})
                }
                "Extensions.getExtensions" => json!({"extensions":entries}),
                "Target.closeTarget" => json!({"success":true}),
                other => panic!("Unexpected browser API {other}"),
            };
            let mut response = json!({"id":request["id"],"result":result});
            if let Some(session) = request.get("sessionId") {
                response["sessionId"] = session.clone();
            }
            if ws
                .send(tungstenite::Message::Text(response.to_string().into()))
                .is_err()
            {
                break;
            }
        }
    })
}
fn install(root: &TempDir, profile: &BrowserProfile) -> (Manager, cepm_core::Installation) {
    let source = root.path().join("source");
    fs::create_dir(&source).unwrap();
    fs::write(
        source.join("manifest.json"),
        r#"{"manifest_version":3,"name":"Probe","version":"1.0.0"}"#,
    )
    .unwrap();
    let mut manager = Manager::open(&root.path().join("cepm")).unwrap();
    let preview = manager.preview_local(&source).unwrap();
    let record = manager.install_local(preview, profile.target()).unwrap();
    (manager, record)
}
#[test]
fn authorized_browser_identification_fills_missing_id_and_persists_automatic_confirmation() {
    let root = tempfile::tempdir().unwrap();
    let profile = profile(&root);
    let (mut manager, record) = install(&root, &profile);
    manager
        .confirm_manual(&record.id, &profile.target(), record.generation, "1.0.0")
        .unwrap();
    let server = serve(
        &profile,
        json!([{"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","name":"Probe","path":record.install_dir,"version":"1.0.0","enabled":true}]),
        None,
    );
    let mut session = EdgeSession::connect(&profile, BrowserCancellation::default()).unwrap();
    let evidence = session.verify(&record).unwrap();
    drop(session);
    let confirmed = manager
        .confirm_browser(evidence, &profile.target())
        .unwrap();
    assert_eq!(confirmed.verification, Verification::Automatic);
    assert_eq!(
        confirmed.extension_id.as_deref(),
        Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
    );
    assert_eq!(confirmed.effective_version.as_deref(), Some("1.0.0"));
    assert!(!confirmed.waiting_identification());
    assert_eq!(confirmed.installed_at, record.installed_at);
    assert_eq!(confirmed.updated_at, record.updated_at);
    drop(manager);
    assert_eq!(
        Manager::open(&root.path().join("cepm"))
            .unwrap()
            .list()
            .unwrap()[0],
        confirmed
    );
    server.join().unwrap();
}

#[test]
fn a_profile_created_after_observation_invalidates_the_result() {
    let root = tempfile::tempdir().unwrap();
    let profile = profile(&root);
    let (mut manager, record) = install(&root, &profile);
    let server = serve(
        &profile,
        json!([{"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","path":record.install_dir,"version":"1.0.0","enabled":true}]),
        None,
    );
    let mut session = EdgeSession::connect(&profile, BrowserCancellation::default()).unwrap();
    let evidence = session.verify(&record).unwrap();
    drop(session);
    fs::create_dir(profile.user_data_dir.join("Profile 1")).unwrap();
    fs::write(profile.user_data_dir.join("Profile 1/Preferences"), "{}").unwrap();
    assert!(
        manager
            .confirm_browser(evidence, &profile.target())
            .is_err()
    );
    assert_eq!(manager.list().unwrap()[0], record);
    server.join().unwrap();
}

#[test]
fn a_cancelled_result_cannot_confirm_even_if_it_arrived_successfully() {
    let root = tempfile::tempdir().unwrap();
    let profile = profile(&root);
    let (mut manager, record) = install(&root, &profile);
    let server = serve(
        &profile,
        json!([{"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","path":record.install_dir,"version":"1.0.0","enabled":true}]),
        None,
    );
    let cancellation = BrowserCancellation::default();
    let mut session = EdgeSession::connect(&profile, cancellation.clone()).unwrap();
    let evidence = session.verify(&record).unwrap();
    drop(session);
    cancellation.cancel();
    assert!(
        manager
            .confirm_browser(evidence, &profile.target())
            .unwrap_err()
            .contains("取消")
    );
    assert_eq!(manager.list().unwrap()[0], record);
    server.join().unwrap();
}

#[test]
fn wrong_native_profile_path_never_confirms_the_installation() {
    let root = tempfile::tempdir().unwrap();
    let profile = profile(&root);
    let (manager, record) = install(&root, &profile);
    let server = serve(&profile, json!([]), Some(root.path().display().to_string()));
    let error = EdgeSession::connect(&profile, BrowserCancellation::default())
        .err()
        .unwrap();
    assert!(error.contains("不一致"), "{error}");
    assert_eq!(manager.list().unwrap()[0], record);
    server.join().unwrap();
}

#[test]
fn unavailable_browser_observations_preserve_the_users_previous_confirmation() {
    for entries_kind in [
        "missing",
        "disabled",
        "wrong-version",
        "ambiguous",
        "wrong-path",
        "wrong-id",
    ] {
        let root = tempfile::tempdir().unwrap();
        let profile = profile(&root);
        let (mut manager, record) = install(&root, &profile);
        let confirmed = manager
            .confirm_manual(&record.id, &profile.target(), record.generation, "1.0.0")
            .unwrap();
        let mut entry = json!({"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","path":record.install_dir,"version":"1.0.0","enabled":true});
        match entries_kind {
            "disabled" => entry["enabled"] = false.into(),
            "wrong-version" => entry["version"] = "2.0.0".into(),
            "wrong-path" => entry["path"] = root.path().display().to_string().into(),
            "wrong-id" => entry["id"] = "not-an-extension".into(),
            _ => {}
        }
        let entries = match entries_kind {
            "missing" => json!([]),
            "ambiguous" => json!([entry.clone(), entry]),
            _ => json!([entry]),
        };
        let server = serve(&profile, entries, None);
        let mut session = EdgeSession::connect(&profile, BrowserCancellation::default()).unwrap();
        assert!(session.verify(&confirmed).is_err(), "{entries_kind}");
        drop(session);
        assert_eq!(manager.list().unwrap()[0], confirmed, "{entries_kind}");
        server.join().unwrap();
    }
}

#[test]
fn another_selected_target_cannot_accept_the_result() {
    let root = tempfile::tempdir().unwrap();
    let profile = profile(&root);
    let (mut manager, record) = install(&root, &profile);
    let server = serve(
        &profile,
        json!([{"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","path":record.install_dir,"version":"1.0.0","enabled":true}]),
        None,
    );
    let mut session = EdgeSession::connect(&profile, BrowserCancellation::default()).unwrap();
    let evidence = session.verify(&record).unwrap();
    drop(session);
    let mut wrong = profile.target();
    wrong.profile = "other-profile".into();
    assert!(manager.confirm_browser(evidence, &wrong).is_err());
    assert_eq!(manager.list().unwrap()[0], record);
    server.join().unwrap();
}

#[test]
fn cancellation_interrupts_an_unresponsive_browser_without_changing_installation() {
    let root = tempfile::tempdir().unwrap();
    let profile = profile(&root);
    let (manager, record) = install(&root, &profile);
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    fs::write(
        profile.user_data_dir.join("DevToolsActivePort"),
        format!(
            "{}\n/devtools/browser/test-session",
            listener.local_addr().unwrap().port()
        ),
    )
    .unwrap();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(3)))
            .unwrap();
        let mut ws = tungstenite::accept(stream).unwrap();
        let _ = ws.read(); // deliberately no response: browser approval still pending
        while let Ok(message) = ws.read() {
            if message.is_close() {
                break;
            }
        }
    });
    let cancel = BrowserCancellation::default();
    let stop = cancel.clone();
    let canceller = thread::spawn(move || {
        thread::sleep(std::time::Duration::from_millis(150));
        stop.cancel();
    });
    let started = std::time::Instant::now();
    let error = EdgeSession::connect(&profile, cancel).err().unwrap();
    assert!(error.contains("取消"), "{error}");
    assert!(started.elapsed() < std::time::Duration::from_secs(2));
    assert_eq!(manager.list().unwrap()[0], record);
    canceller.join().unwrap();
    server.join().unwrap();
}

#[test]
fn an_old_deployment_observation_cannot_confirm_new_files() {
    let root = tempfile::tempdir().unwrap();
    let profile = profile(&root);
    let (mut manager, record) = install(&root, &profile);
    let server = serve(
        &profile,
        json!([{"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","path":record.install_dir,"version":"1.0.0","enabled":true}]),
        None,
    );
    let mut session = EdgeSession::connect(&profile, BrowserCancellation::default()).unwrap();
    let evidence = session.verify(&record).unwrap();
    drop(session);
    fs::write(
        record.source.join("manifest.json"),
        r#"{"manifest_version":3,"name":"Probe","version":"2.0.0"}"#,
    )
    .unwrap();
    let plan = manager.check_local(&record.id).unwrap();
    let next = manager
        .apply_update(plan, cepm_core::UpdateKind::Upgrade)
        .unwrap();
    assert!(
        manager
            .confirm_browser(evidence, &profile.target())
            .is_err()
    );
    assert_eq!(manager.list().unwrap()[0], next);
    server.join().unwrap();
}

#[test]
fn authorization_and_native_version_page_can_be_slow_without_flaking_connection() {
    let root = tempfile::tempdir().unwrap();
    let profile = profile(&root);
    let (mut manager, record) = install(&root, &profile);
    let server = serve_delayed(
        &profile,
        json!([{"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","path":record.install_dir,"version":"1.0.0","enabled":true}]),
        None,
        std::time::Duration::from_millis(2200),
        2,
    );
    let mut session = EdgeSession::connect(&profile, BrowserCancellation::default()).unwrap();
    let evidence = session.verify(&record).unwrap();
    drop(session);
    assert_eq!(
        manager
            .confirm_browser(evidence, &profile.target())
            .unwrap()
            .verification,
        Verification::Automatic
    );
    server.join().unwrap();
}

#[test]
fn automatic_recheck_clears_reported_failure_without_changing_files() {
    let root = tempfile::tempdir().unwrap();
    let profile = profile(&root);
    let (mut manager, record) = install(&root, &profile);
    let failed = manager
        .report_browser_failure(&record.id, &profile.target(), record.generation)
        .unwrap();
    assert!(failed.load_failed);
    let server = serve(
        &profile,
        json!([{"id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","path":record.install_dir,"version":"1.0.0","enabled":true}]),
        None,
    );
    let mut session = EdgeSession::connect(&profile, BrowserCancellation::default()).unwrap();
    let evidence = session.verify(&failed).unwrap();
    drop(session);
    let result = manager
        .confirm_browser(evidence, &profile.target())
        .unwrap();
    assert!(!result.load_failed);
    assert_eq!(result.installed_version, "1.0.0");
    assert_eq!(result.updated_at, record.updated_at);
    server.join().unwrap();
}

#[test]
fn refused_or_revoked_authorization_leaves_pending_files_and_record() {
    for revoked in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let profile = profile(&root);
        let (manager, record) = install(&root, &profile);
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        fs::write(
            profile.user_data_dir.join("DevToolsActivePort"),
            format!(
                "{}\n/devtools/browser/test-session",
                listener.local_addr().unwrap().port()
            ),
        )
        .unwrap();
        let server = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut ws = tungstenite::accept(stream).unwrap();
            let request: Value =
                serde_json::from_str(ws.read().unwrap().to_text().unwrap()).unwrap();
            if revoked {
                let _ = ws.close(None);
            } else {
                let _ = ws.send(tungstenite::Message::Text(
                    json!({"id":request["id"],"error":{"code":-32000,"message":"Access denied"}})
                        .to_string()
                        .into(),
                ));
            }
        });
        assert!(EdgeSession::connect(&profile, BrowserCancellation::default()).is_err());
        assert_eq!(manager.list().unwrap()[0], record);
        assert!(record.install_dir.join("manifest.json").exists());
        server.join().unwrap();
    }
}
