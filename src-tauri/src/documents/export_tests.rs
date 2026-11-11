use super::{
    Control,
    export::{self, Companion},
    jobs::{Direction, DocumentStore},
};
use crate::recipes::{Recipe, Rule, Step};
use std::{
    fs,
    path::Path,
    time::{Duration, Instant},
};

fn job(store: &DocumentStore, source: &Path) -> String {
    let document = store.capture(source, &Control::default()).unwrap();
    let recipe = Recipe {
        schema_version: 2,
        id: "user-export".into(),
        name: "Export".into(),
        description: String::new(),
        steps: vec![Step {
            enabled: true,
            rule: Rule::Caesar { shift: 3 },
        }],
    };
    let job = store
        .start(
            &document.id,
            recipe,
            "aPrivatePassphrase".into(),
            Direction::Encode,
        )
        .unwrap();
    let started = Instant::now();
    loop {
        let state = store.status(&job.id).unwrap();
        if state.state != "running" {
            assert_eq!(state.state, "complete");
            break;
        }
        assert!(started.elapsed() < Duration::from_secs(10));
        std::thread::sleep(Duration::from_millis(5));
    }
    job.id
}

#[test]
fn saves_result_and_one_portable_companion_preserving_original() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.md");
    fs::write(&source, "\u{feff}Café\r\n").unwrap();
    let store = DocumentStore::new().unwrap();
    let id = job(&store, &source);
    let result = root.path().join("result.txt");
    let receipt = export::save(&store, &id, &result, Companion::Json, &Control::default()).unwrap();
    assert_eq!(receipt.artifacts.len(), 2);
    assert_eq!(fs::read_to_string(&result).unwrap(), "\u{feff}Fdié\r\n");
    let json = fs::read_to_string(root.path().join("result.recipe.json")).unwrap();
    let parsed: Recipe = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.schema_version, 2);
    assert!(!json.contains("PrivatePassphrase"));
    assert!(!json.contains(root.path().to_str().unwrap()));
    assert_eq!(fs::read_to_string(&source).unwrap(), "\u{feff}Café\r\n");
    let fingerprint_result = root.path().join("fingerprint.txt");
    export::save(
        &store,
        &id,
        &fingerprint_result,
        Companion::Fingerprint,
        &Control::default(),
    )
    .unwrap();
    let fp = fs::read_to_string(root.path().join("fingerprint.fingerprint.txt")).unwrap();
    assert!(fp.starts_with("CRYPT-R2-"));
    assert!(fp.contains("\nSHA-256: "));
    assert!(!root.path().join("fingerprint.recipe.json").exists());
    export::save(
        &store,
        &id,
        &root.path().join("plain.txt"),
        Companion::None,
        &Control::default(),
    )
    .unwrap();
    assert!(!root.path().join("plain.recipe.json").exists());
}

#[test]
fn collisions_and_source_aliases_never_overwrite_any_file() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.txt");
    fs::write(&source, "abc").unwrap();
    let store = DocumentStore::new().unwrap();
    let id = job(&store, &source);
    assert!(export::save(&store, &id, &source, Companion::None, &Control::default()).is_err());
    let destination = root.path().join("existing.txt");
    fs::write(&destination, "human bytes").unwrap();
    assert!(
        export::save(
            &store,
            &id,
            &destination,
            Companion::Json,
            &Control::default()
        )
        .is_err()
    );
    assert_eq!(fs::read_to_string(&destination).unwrap(), "human bytes");
    assert_eq!(fs::read_to_string(&source).unwrap(), "abc");
    let destination = root.path().join("sibling.txt");
    let companion = root.path().join("sibling.recipe.json");
    fs::write(&companion, "existing companion").unwrap();
    assert!(
        export::save(
            &store,
            &id,
            &destination,
            Companion::Json,
            &Control::default()
        )
        .is_err()
    );
    assert!(!destination.exists());
    assert_eq!(fs::read_to_string(companion).unwrap(), "existing companion");
}

#[test]
fn corrupted_stage_and_cancel_do_not_publish_success() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.txt");
    fs::write(&source, "abc").unwrap();
    let store = DocumentStore::new().unwrap();
    let id = job(&store, &source);
    let destination = root.path().join("result.txt");
    let control = Control::default();
    control.cancel();
    assert!(export::save(&store, &id, &destination, Companion::Json, &control).is_err());
    assert!(!destination.exists());
    let artifact = store.job(&id).unwrap().result().unwrap();
    fs::write(&artifact.path, "corrupted").unwrap();
    assert!(
        export::save(
            &store,
            &id,
            &destination,
            Companion::Json,
            &Control::default()
        )
        .is_err()
    );
    assert!(!destination.exists());
    assert!(!root.path().join("result.recipe.json").exists());
}

#[test]
fn later_publication_failure_reports_the_verified_companion_and_keeps_job() {
    struct FailSecond(bool);
    impl export::Publisher for FailSecond {
        fn publish(
            &mut self,
            stage: tempfile::NamedTempFile,
            path: &Path,
        ) -> Result<fs::File, String> {
            if self.0 {
                return Err("Synthetic second publication failure.".into());
            }
            self.0 = true;
            export::Publisher::publish(&mut export::NativePublisher, stage, path)
        }
    }
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.txt");
    fs::write(&source, "abc").unwrap();
    let store = DocumentStore::new().unwrap();
    let id = job(&store, &source);
    let destination = root.path().join("partial.txt");
    let error = export::save_with(
        &store,
        &id,
        &destination,
        Companion::Json,
        &Control::default(),
        &mut FailSecond(false),
    )
    .unwrap_err();
    assert_eq!(error.published.len(), 1);
    assert_eq!(error.published[0].name, "partial.recipe.json");
    assert!(!destination.exists());
    assert!(
        fs::read_to_string(root.path().join("partial.recipe.json"))
            .unwrap()
            .contains("schemaVersion")
    );
    export::save(
        &store,
        &id,
        &root.path().join("retry.txt"),
        Companion::None,
        &Control::default(),
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(root.path().join("retry.txt")).unwrap(),
        "def"
    );
    assert_eq!(fs::read_to_string(source).unwrap(), "abc");
}

#[test]
fn lost_publish_acknowledgment_is_reconciled_from_actual_bytes() {
    struct LostAck;
    impl export::Publisher for LostAck {
        fn publish(
            &mut self,
            stage: tempfile::NamedTempFile,
            path: &Path,
        ) -> Result<fs::File, String> {
            export::Publisher::publish(&mut export::NativePublisher, stage, path)?;
            Err("Synthetic acknowledgment loss after publication.".into())
        }
    }
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.txt");
    fs::write(&source, "abc").unwrap();
    let store = DocumentStore::new().unwrap();
    let id = job(&store, &source);
    let destination = root.path().join("verified.txt");
    let receipt = export::save_with(
        &store,
        &id,
        &destination,
        Companion::None,
        &Control::default(),
        &mut LostAck,
    )
    .unwrap();
    assert_eq!(receipt.artifacts.len(), 1);
    assert_eq!(fs::read_to_string(destination).unwrap(), "def");
}

#[test]
fn cancellation_after_publication_reports_an_unverified_file() {
    struct CancelAfterPublish<'a>(&'a Control);
    impl export::Publisher for CancelAfterPublish<'_> {
        fn publish(
            &mut self,
            stage: tempfile::NamedTempFile,
            path: &Path,
        ) -> Result<fs::File, String> {
            let file = export::Publisher::publish(&mut export::NativePublisher, stage, path)?;
            assert_eq!(self.0.progress().unwrap().0, "Publishing new file");
            self.0.cancel();
            Ok(file)
        }
    }
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("source.txt");
    fs::write(&source, "abc").unwrap();
    let store = DocumentStore::new().unwrap();
    let id = job(&store, &source);
    let destination = root.path().join("interrupted.txt");
    let control = Control::default();
    let error = export::save_with(
        &store,
        &id,
        &destination,
        Companion::None,
        &control,
        &mut CancelAfterPublish(&control),
    )
    .unwrap_err();
    assert!(error.published.is_empty());
    assert_eq!(error.unverified, ["interrupted.txt"]);
    assert_eq!(fs::read_to_string(destination).unwrap(), "def");
    assert_eq!(fs::read_to_string(source).unwrap(), "abc");
}

#[test]
fn companion_import_validates_actual_bounds_and_leaves_recipes_session_local() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("recipe.json");
    fs::write(&path, r#"{"schemaVersion":2,"id":"user-file","name":"File","description":"","steps":[{"rule":{"type":"atbash"}}]}"#).unwrap();
    assert!(matches!(
        export::read_companion(&path).unwrap(),
        export::ImportedCompanion::Recipe { .. }
    ));
    for bad in [
        vec![0xff],
        b"{\"schemaVersion\":9000}".to_vec(),
        vec![b' '; 65537],
        b"CRYPT-R3-0000000000000000".to_vec(),
    ] {
        fs::write(&path, bad).unwrap();
        assert!(export::read_companion(&path).is_err());
    }
    let recipe: Recipe = serde_json::from_str(r#"{"schemaVersion":2,"id":"user-file","name":"File","description":"","steps":[{"rule":{"type":"atbash"}}]}"#).unwrap();
    let fp = crate::recipes::fingerprint::fingerprint(&recipe).unwrap();
    fs::write(&path, format!("{}\nSHA-256: {}\n", fp.display, fp.value)).unwrap();
    assert!(matches!(
        export::read_companion(&path).unwrap(),
        export::ImportedCompanion::Fingerprint { .. }
    ));
}
