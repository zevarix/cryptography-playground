use super::activities::{ActivityInfo, ActivityResult, AppDocuments};
use std::{
    fs,
    time::{Duration, Instant},
};

fn finish(app: &AppDocuments, id: &str) -> ActivityInfo {
    let started = Instant::now();
    loop {
        let info = app.status(id).unwrap();
        if info.state != "running" {
            return info;
        }
        assert!(started.elapsed() < Duration::from_secs(15));
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn open_activity_returns_a_snapshot_and_rejects_invalid_bytes() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("file.md");
    fs::write(&path, "Café\r\n").unwrap();
    let app = AppDocuments::new().unwrap();
    let activity = app.open(path.clone()).unwrap();
    let result = finish(&app, &activity.id);
    assert_eq!(result.state, "complete");
    let Some(ActivityResult::Opened { document }) = result.result else {
        panic!("missing opened snapshot")
    };
    app.discard(&activity.id, true).unwrap();
    assert!(app.status(&activity.id).is_err());
    fs::write(&path, [0xff]).unwrap();
    let invalid = app.open(path).unwrap();
    assert_eq!(finish(&app, &invalid.id).state, "failed");
    assert_eq!(
        app.store().preview_document(&document.id, 0).unwrap().text,
        "Café\r\n"
    );
}

#[test]
fn abandoned_activity_releases_its_unadopted_document() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("file.txt");
    fs::write(&path, "text").unwrap();
    let app = AppDocuments::new().unwrap();
    let activity = app.open(path).unwrap();
    let Some(ActivityResult::Opened { document }) = finish(&app, &activity.id).result else {
        panic!("missing snapshot")
    };
    app.discard(&activity.id, false).unwrap();
    assert!(app.store().preview_document(&document.id, 0).is_err());
}

#[test]
fn native_drop_tokens_are_single_use_and_contain_no_path_authority() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("drop.txt");
    fs::write(&path, "hello").unwrap();
    let app = AppDocuments::new().unwrap();
    let notice = app.register_drop(vec![path.clone()]).unwrap();
    let data = serde_json::to_string(&notice).unwrap();
    assert!(!data.contains(root.path().to_str().unwrap()));
    let activity = app.open_drop(&notice.token).unwrap();
    assert!(app.open_drop(&notice.token).is_err());
    assert_eq!(finish(&app, &activity.id).state, "complete");
    assert!(app.register_drop(vec![path.clone(), path]).is_err());
}
