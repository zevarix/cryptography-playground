use super::{
    Control,
    jobs::{Direction, DocumentStore},
    reconstruct::{self, DocChoice},
};
use crate::recipes::{Recipe, Rule, Step};
use std::{
    fs,
    time::{Duration, Instant},
};

fn finish(store: &DocumentStore, id: &str) {
    let started = Instant::now();
    loop {
        let info = store.status(id).unwrap();
        if info.state != "running" {
            assert_eq!(info.state, "complete", "{}", info.note);
            return;
        }
        assert!(started.elapsed() < Duration::from_secs(20));
        std::thread::sleep(Duration::from_millis(5));
    }
}
fn decode_fixture(
    store: &DocumentStore,
    text: &str,
    steps: Vec<Rule>,
) -> (tempfile::TempDir, String) {
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("original.txt");
    fs::write(&source, text).unwrap();
    let recipe = Recipe {
        schema_version: 2,
        id: "user-reading".into(),
        name: "Reading".into(),
        description: String::new(),
        steps: steps
            .into_iter()
            .map(|rule| Step {
                enabled: true,
                rule,
            })
            .collect(),
    };
    let document = store.capture(&source, &Control::default()).unwrap();
    let encode = store
        .start(
            &document.id,
            recipe.clone(),
            "lemon".into(),
            Direction::Encode,
        )
        .unwrap();
    finish(store, &encode.id);
    let ciphertext = store.job(&encode.id).unwrap().result().unwrap();
    let document = store
        .capture(&ciphertext.path, &Control::default())
        .unwrap();
    let decoded = store
        .start(&document.id, recipe, "lemon".into(), Direction::Decode)
        .unwrap();
    finish(store, &decoded.id);
    (root, decoded.id)
}

#[test]
fn document_phonetic_search_has_global_state_and_paged_stable_word_ids() {
    let store = DocumentStore::new().unwrap();
    let (_root, job) = decode_fixture(
        &store,
        &"a ".repeat(50),
        vec![Rule::PhoneticRotate, Rule::Caesar { shift: 3 }],
    );
    let original = store.job(&job).unwrap().result().unwrap();
    let reading = reconstruct::run(&store, &job, &[], &Control::default()).unwrap();
    assert_eq!(reading.words, "50");
    let first = reconstruct::page(&store, &job, &reading.id, 0).unwrap();
    assert!(!first.eof);
    assert_eq!(first.words.len(), 32);
    let last = reconstruct::page(
        &store,
        &job,
        &reading.id,
        first.next_offset.parse().unwrap(),
    )
    .unwrap();
    assert!(last.eof);
    assert_eq!(last.words.last().unwrap().word_id, format!("{job}:49"));
    let pinned = reconstruct::run(
        &store,
        &job,
        &[DocChoice {
            word_id: format!("{job}:49"),
            candidate: "eye".into(),
        }],
        &Control::default(),
    )
    .unwrap();
    let preview = reconstruct::preview(&store, &job, &pinned.id, 0).unwrap();
    assert!(preview.text.ends_with("eye"));
    assert_eq!(
        store.job(&job).unwrap().result().unwrap().stats.digest,
        original.stats.digest
    );
    assert!(
        reconstruct::run(
            &store,
            &job,
            &[DocChoice {
                word_id: format!("{job}:49"),
                candidate: "fox".into()
            }],
            &Control::default()
        )
        .is_err()
    );
    assert!(reconstruct::preview(&store, &job, &pinned.id, 0).is_ok());
    assert!(reconstruct::preview(&store, &job, &reading.id, 0).is_err());
    let destination = tempfile::tempdir().unwrap();
    let output = destination.path().join("reading.txt");
    super::export::save_reading(
        &store,
        &job,
        Some(&pinned.id),
        &output,
        super::export::Companion::Json,
        &Control::default(),
    )
    .unwrap();
    assert!(fs::read_to_string(&output).unwrap().ends_with("eye"));
    assert_eq!(
        store.job(&job).unwrap().result().unwrap().stats.digest,
        original.stats.digest
    );
}

#[test]
fn skeleton_reading_preserves_surviving_format_for_forward_validation() {
    let store = DocumentStore::new().unwrap();
    let input = "# The quick brown fox.\r\n".repeat(16);
    let (_root, job) = decode_fixture(
        &store,
        &input,
        vec![Rule::RemoveVowels, Rule::CollapseDoubles, Rule::WordMirror],
    );
    let reading = reconstruct::run(&store, &job, &[], &Control::default()).unwrap();
    assert_eq!(reading.words, "64");
    let text = reconstruct::preview(&store, &job, &reading.id, 0)
        .unwrap()
        .text;
    assert!(text.starts_with("# T"));
    assert_eq!(text.matches(".\r\n").count(), 16);
    assert!(
        reconstruct::run(
            &store,
            &job,
            &[DocChoice {
                word_id: "different-job:3".into(),
                candidate: "fox".into()
            }],
            &Control::default()
        )
        .is_err()
    );
}

#[test]
fn cancel_or_unsupported_search_never_loses_the_exact_intermediate() {
    let store = DocumentStore::new().unwrap();
    let (_root, job) = decode_fixture(&store, "The fox", vec![Rule::RemoveVowels]);
    let before = store.preview_job(&job, None, 0).unwrap().text;
    assert!(reconstruct::run(&store, &job, &[], &Control::default()).is_err());
    assert_eq!(store.preview_job(&job, None, 0).unwrap().text, before);
    let cancelled = Control::default();
    cancelled.cancel();
    assert!(reconstruct::run(&store, &job, &[], &cancelled).is_err());
    assert_eq!(store.preview_job(&job, None, 0).unwrap().text, before);
}
