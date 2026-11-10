use super::{
    Control,
    jobs::{Direction, DocumentStore},
};
use crate::recipes::{Recipe, Rule, Step};
use std::{
    fs,
    time::{Duration, Instant},
};

fn recipe() -> Recipe {
    Recipe {
        schema_version: 2,
        id: "user-document".into(),
        name: "Document".into(),
        description: String::new(),
        steps: vec![
            Step {
                enabled: true,
                rule: Rule::Caesar { shift: 3 },
            },
            Step {
                enabled: true,
                rule: Rule::RailFence { rails: 3 },
            },
        ],
    }
}
fn finish(store: &DocumentStore, id: &str) -> super::jobs::JobInfo {
    let started = Instant::now();
    loop {
        let info = store.status(id).unwrap();
        if info.state != "running" {
            return info;
        }
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "job did not finish"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn immutable_snapshot_preserves_bom_and_source_after_external_edits() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("notes.md");
    fs::write(&source, "\u{feff}Café\r\n世界!\n").unwrap();
    let store = DocumentStore::new().unwrap();
    let document = store.capture(&source, &Control::default()).unwrap();
    assert!(document.bom);
    assert_eq!(document.name, "notes.md");
    assert_eq!(
        store.preview_document(&document.id, 0).unwrap().text,
        "Café\r\n世界!\n"
    );
    fs::write(&source, "later human edit").unwrap();
    let job = store
        .start(&document.id, recipe(), String::new(), Direction::Encode)
        .unwrap();
    let result = finish(&store, &job.id);
    assert_eq!(result.state, "complete");
    assert!(result.exact);
    assert_eq!(result.recipe.schema_version, 2);
    assert_eq!(
        store.preview_job(&job.id, Some(0), 0).unwrap().text,
        "Fdié\r\n世界!\n"
    );
    assert_eq!(fs::read_to_string(&source).unwrap(), "later human edit");
}

#[test]
fn invalid_capture_and_unknown_ids_leave_previous_snapshot_usable() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("a.txt");
    let store = DocumentStore::new().unwrap();
    fs::write(&source, "keep this").unwrap();
    let document = store.capture(&source, &Control::default()).unwrap();
    fs::write(&source, [0xff]).unwrap();
    assert!(store.capture(&source, &Control::default()).is_err());
    assert_eq!(
        store.preview_document(&document.id, 0).unwrap().text,
        "keep this"
    );
    assert!(store.preview_document("missing", 0).is_err());
    assert!(store.status("missing").is_err());
    let cancelled = Control::default();
    cancelled.cancel();
    assert!(store.capture(&source, &cancelled).is_err());
}

#[test]
fn preview_pages_never_split_utf8_and_empty_documents_complete() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("unicode.txt");
    let store = DocumentStore::new().unwrap();
    let text = format!("{}🙂end", "é".repeat(10000));
    fs::write(&source, &text).unwrap();
    let document = store.capture(&source, &Control::default()).unwrap();
    let mut result = String::new();
    let mut offset = 0;
    loop {
        let page = store.preview_document(&document.id, offset).unwrap();
        assert!(page.text.len() <= 16387);
        result.push_str(&page.text);
        if page.eof {
            break;
        }
        offset = page.next_offset.parse().unwrap();
    }
    assert_eq!(result, text);
    assert!(store.preview_document(&document.id, 1).is_err());
    for input in ["", "\u{feff}"] {
        fs::write(&source, input).unwrap();
        let document = store.capture(&source, &Control::default()).unwrap();
        let job = store
            .start(&document.id, recipe(), String::new(), Direction::Encode)
            .unwrap();
        assert_eq!(finish(&store, &job.id).state, "complete");
        assert_eq!(store.preview_job(&job.id, None, 0).unwrap().text, "");
    }
}

#[test]
fn legacy_documents_keep_the_original_input_contract() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("legacy.txt");
    let store = DocumentStore::new().unwrap();
    let mut old = recipe();
    old.schema_version = 1;
    for input in [String::new(), "café".into(), "a".repeat(16385)] {
        fs::write(&source, input).unwrap();
        let document = store.capture(&source, &Control::default()).unwrap();
        assert!(
            store
                .start(&document.id, old.clone(), String::new(), Direction::Encode)
                .is_err()
        );
    }
}

#[test]
fn cancellation_never_reports_a_completed_job() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("large.txt");
    fs::write(&source, "Aé🙂 ".repeat(500000)).unwrap();
    let store = DocumentStore::new().unwrap();
    let document = store.capture(&source, &Control::default()).unwrap();
    let job = store
        .start(&document.id, recipe(), String::new(), Direction::Encode)
        .unwrap();
    store.cancel(&job.id).unwrap();
    let result = finish(&store, &job.id);
    assert_eq!(result.state, "cancelled");
    assert!(store.preview_job(&job.id, None, 0).is_err());
    assert!(store.preview_document(&document.id, 0).is_ok());
}

#[test]
fn one_document_admits_one_running_job_and_keeps_captured_recipe() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("large.txt");
    fs::write(&source, "abcé🙂 ".repeat(200000)).unwrap();
    let store = DocumentStore::new().unwrap();
    let document = store.capture(&source, &Control::default()).unwrap();
    let mut selected = recipe();
    let job = store
        .start(
            &document.id,
            selected.clone(),
            String::new(),
            Direction::Encode,
        )
        .unwrap();
    selected.steps[0].rule = Rule::Atbash;
    assert!(
        store
            .start(&document.id, selected, String::new(), Direction::Encode)
            .is_err()
    );
    assert!(matches!(
        store.status(&job.id).unwrap().recipe.steps[0].rule,
        Rule::Caesar { shift: 3 }
    ));
    store.cancel(&job.id).unwrap();
    assert_eq!(finish(&store, &job.id).state, "cancelled");
    store.discard_job(&job.id).unwrap();
    assert!(store.status(&job.id).is_err());
    store.discard_document(&document.id).unwrap();
    assert!(store.preview_document(&document.id, 0).is_err());
}

#[test]
fn lossy_decode_retains_the_exact_surviving_layer() {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("lossy.txt");
    fs::write(&source, "The quick brown fox").unwrap();
    let store = DocumentStore::new().unwrap();
    let document = store.capture(&source, &Control::default()).unwrap();
    let mut selected = recipe();
    selected.steps.insert(
        0,
        Step {
            enabled: true,
            rule: Rule::CollapseDoubles,
        },
    );
    selected.steps.insert(
        0,
        Step {
            enabled: true,
            rule: Rule::RemoveVowels,
        },
    );
    let encoded = store
        .start(
            &document.id,
            selected.clone(),
            String::new(),
            Direction::Encode,
        )
        .unwrap();
    assert_eq!(finish(&store, &encoded.id).state, "complete");
    let encoded = store.preview_job(&encoded.id, None, 0).unwrap();
    fs::write(&source, encoded.text).unwrap();
    let document = store.capture(&source, &Control::default()).unwrap();
    let decoded = store
        .start(&document.id, selected, String::new(), Direction::Decode)
        .unwrap();
    let info = finish(&store, &decoded.id);
    assert_eq!(info.state, "complete");
    assert!(!info.exact);
    assert_eq!(
        store.preview_job(&decoded.id, None, 0).unwrap().text,
        "Th qck brwn fx"
    );
}

#[test]
#[ignore = "local workload measurement; synthetic files only"]
fn measure_document_roundtrip() {
    use std::io::{BufWriter, Write};
    let blocks: u64 = std::env::var("CRYPTOPLAY_BENCH_BLOCKS")
        .unwrap_or_else(|_| "100000".into())
        .parse()
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("synthetic.txt");
    let mut file = BufWriter::new(fs::File::create(&source).unwrap());
    for _ in 0..blocks {
        file.write_all("# Café 世界 🙂\r\nThe quick brown fox.\n".as_bytes())
            .unwrap();
    }
    file.flush().unwrap();
    drop(file);
    let store = DocumentStore::new().unwrap();
    let started = Instant::now();
    let document = store.capture(&source, &Control::default()).unwrap();
    let encoded = store
        .start(&document.id, recipe(), String::new(), Direction::Encode)
        .unwrap();
    assert_eq!(finish(&store, &encoded.id).state, "complete");
    let result = store.job(&encoded.id).unwrap().result().unwrap();
    let ciphertext = store.capture(&result.path, &Control::default()).unwrap();
    let decoded = store
        .start(&ciphertext.id, recipe(), String::new(), Direction::Decode)
        .unwrap();
    assert_eq!(finish(&store, &decoded.id).state, "complete");
    let result = store.job(&decoded.id).unwrap().result().unwrap();
    assert_eq!(result.stats.digest, document.digest);
    assert_eq!(result.stats.bytes.to_string(), document.bytes);
    println!(
        "document_bytes={} roundtrip_elapsed_ms={}",
        document.bytes,
        started.elapsed().as_millis()
    );
}
