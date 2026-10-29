use cepm_core::{Browser, profiles::ProfileBook};
use std::fs;

#[test]
fn pins_and_last_target_survive_reopening_without_confusing_names() {
    let temp = tempfile::tempdir().unwrap();
    let app = temp.path().join("chrome");
    fs::write(&app, "executable fixture").unwrap();
    let data = temp.path().join("browser-data");
    fs::create_dir_all(data.join("Default")).unwrap();
    fs::create_dir(data.join("Profile 1")).unwrap();
    let root = temp.path().join("cepm");
    let mut book = ProfileBook::open(&root).unwrap();
    let personal = book
        .bind(Browser::Chrome, "个人", &app, &data.join("Default"))
        .unwrap();
    let work = book
        .bind(Browser::Chrome, "个人", &app, &data.join("Profile 1"))
        .unwrap();
    assert_ne!(personal.id, work.id);
    book.set_pinned(&personal.id, true).unwrap();
    book.set_pinned(&work.id, true).unwrap();
    book.select(&work.id).unwrap();
    drop(book);
    let mut book = ProfileBook::open(&root).unwrap();
    assert_eq!(book.selected().unwrap().id, work.id);
    assert_eq!(book.pinned_ids(), &[personal.id.clone(), work.id.clone()]);
    book.set_pinned(&personal.id, false).unwrap();
    assert_eq!(book.selected().unwrap().id, work.id);
    assert_eq!(book.pinned_ids(), &[work.id]);
}

#[test]
fn discovery_uses_profile_locations_and_missing_selection_never_falls_back() {
    use cepm_core::profiles::{BrowserLocation, discover_profiles};
    let temp = tempfile::tempdir().unwrap();
    let app = temp.path().join("chrome");
    fs::write(&app, "fixture").unwrap();
    let data = temp.path().join("user data");
    fs::create_dir_all(data.join("Default")).unwrap();
    fs::create_dir(data.join("Profile 1")).unwrap();
    fs::write(data.join("Local State"), r#"{"profile":{"info_cache":{"Default":{"name":"个人"},"Profile 1":{"name":"工作"},"../outside":{"name":"wrong"}}},"ignored":"not imported"}"#).unwrap();
    let found = discover_profiles(&[BrowserLocation {
        browser: Browser::Chrome,
        application: app.clone(),
        user_data_dir: data.clone(),
    }])
    .unwrap();
    assert_eq!(found.len(), 2);
    let work = found.iter().find(|p| p.name == "工作").unwrap();
    assert_eq!(work.profile_path, data.join("Profile 1"));
    let root = temp.path().join("cepm");
    let mut book = ProfileBook::open(&root).unwrap();
    let personal = book
        .bind(Browser::Chrome, "个人", &app, &data.join("Default"))
        .unwrap();
    let selected = book
        .bind(
            work.browser.clone(),
            &work.name,
            &work.application,
            &work.profile_path,
        )
        .unwrap();
    book.select(&selected.id).unwrap();
    fs::remove_dir(data.join("Profile 1")).unwrap();
    drop(book);
    let book = ProfileBook::open(&root).unwrap();
    assert_eq!(book.selected().unwrap().id, selected.id);
    assert!(book.selected().unwrap().validate_location().is_err());
    assert!(book.get(&personal.id).unwrap().validate_location().is_ok());
}

#[test]
fn native_management_handoff_keeps_application_and_profile_arguments_separate() {
    let temp = tempfile::tempdir().unwrap();
    let app = temp.path().join("Chrome executable");
    fs::write(&app, "fixture").unwrap();
    let profile = temp.path().join("browser data").join("Profile 1");
    fs::create_dir_all(&profile).unwrap();
    let mut book = ProfileBook::open(&temp.path().join("cepm")).unwrap();
    let p = book.bind(Browser::Chrome, "工作", &app, &profile).unwrap();
    let request = p.management_request().unwrap();
    assert_eq!(request.application, app.canonicalize().unwrap());
    assert!(
        request
            .arguments
            .contains(&"--profile-directory=Profile 1".into())
    );
    assert_eq!(request.arguments.last().unwrap(), "chrome://extensions");
    fs::remove_dir(&profile).unwrap();
    assert!(p.management_request().is_err());
}

#[test]
fn legacy_label_requires_explicit_binding_and_cannot_carry_old_confirmation() {
    use cepm_core::{Manager, Target, Verification};
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    fs::create_dir(&source).unwrap();
    fs::write(
        source.join("manifest.json"),
        r#"{"manifest_version":3,"name":"Example","version":"1.0"}"#,
    )
    .unwrap();
    let root = temp.path().join("cepm");
    let mut manager = Manager::open(&root).unwrap();
    let preview = manager.preview_local(&source).unwrap();
    let record = manager
        .install_local(
            preview,
            Target {
                browser: Browser::Chrome,
                profile: "工作".into(),
            },
        )
        .unwrap();
    manager
        .confirm(&record.id, "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "1.0")
        .unwrap();
    let app = temp.path().join("chrome");
    fs::write(&app, "fixture").unwrap();
    let path = temp.path().join("browser/Default");
    fs::create_dir_all(&path).unwrap();
    let mut book = ProfileBook::open(&root).unwrap();
    let profile = book.bind(Browser::Chrome, "工作", &app, &path).unwrap();
    let rebound = manager.bind_legacy_target(&record.id, &profile).unwrap();
    assert_eq!(rebound.target, profile.target());
    assert_eq!(rebound.install_dir, record.install_dir);
    assert_eq!(rebound.verification, Verification::Pending);
    assert_eq!(rebound.effective_version, None);
    assert_eq!(rebound.extension_id, None);
    assert!(manager.bind_legacy_target(&record.id, &profile).is_err());
}

#[test]
fn corrupt_preferences_fail_visibly_instead_of_falling_back_or_losing_selection() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("cepm");
    fs::create_dir(&root).unwrap();
    fs::write(
        root.join("profiles.json"),
        r#"{"profiles":[],"pinned":[],"selected":"missing-profile"}"#,
    )
    .unwrap();
    assert!(ProfileBook::open(&root).is_err());
}
