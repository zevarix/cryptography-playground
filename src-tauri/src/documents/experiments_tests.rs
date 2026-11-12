use super::{
    Control,
    experiments::{Library, Opened},
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
        id: "user-capture".into(),
        name: "Original recipe".into(),
        description: String::new(),
        steps: vec![Step {
            enabled: true,
            rule: Rule::Caesar { shift: 3 },
        }],
    }
}
fn finish(store: &DocumentStore, id: &str) {
    let start = Instant::now();
    loop {
        let info = store.status(id).unwrap();
        if info.state != "running" {
            assert_eq!(info.state, "complete", "{}", info.note);
            break;
        }
        assert!(start.elapsed() < Duration::from_secs(20));
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn saved_text_survives_restart_and_delete_preserves_open_capture() {
    let root = tempfile::tempdir().unwrap();
    let library = Library::new(root.path().join("experiments"));
    let run = library
        .evaluate(
            recipe(),
            "Hello!".into(),
            "privatepassphrase".into(),
            "encode".into(),
            vec![],
        )
        .unwrap();
    let saved = library
        .save_text(&run.run_id, "My experiment", &Control::default())
        .unwrap();
    drop(library);
    let library = Library::new(root.path().join("experiments"));
    assert_eq!(library.list().unwrap().entries.len(), 1);
    let store = DocumentStore::new().unwrap();
    let Opened::Text {
        recipe,
        source,
        result,
        run_id,
        ..
    } = library
        .open(&saved.id, &store, &Control::default())
        .unwrap()
    else {
        panic!("expected text")
    };
    assert_eq!(source, "Hello!");
    assert_eq!(result.output, "Khoor!");
    assert_eq!(recipe.name, "Original recipe");
    assert_ne!(run_id, run.run_id);
    let bytes = fs::read(
        root.path()
            .join("experiments")
            .join(&saved.id)
            .join("manifest.json"),
    )
    .unwrap();
    assert!(
        !String::from_utf8(bytes)
            .unwrap()
            .contains("privatepassphrase")
    );
    library.delete(&saved.id).unwrap();
    assert!(library.list().unwrap().entries.is_empty());
    assert!(
        library
            .save_text(&run_id, "Independent", &Control::default())
            .is_ok()
    );
}

#[test]
fn saved_file_retains_large_bom_crlf_source_stages_without_original_path() {
    let root = tempfile::tempdir().unwrap();
    let original = root.path().join("original.txt");
    let text = format!("\u{feff}{}", "Héllo!\r\n".repeat(10000));
    fs::write(&original, &text).unwrap();
    let store = DocumentStore::new().unwrap();
    let document = store.capture(&original, &Control::default()).unwrap();
    let job = store
        .start(
            &document.id,
            recipe(),
            "privatepassphrase".into(),
            Direction::Encode,
        )
        .unwrap();
    finish(&store, &job.id);
    let library = Library::new(root.path().join("experiments"));
    let saved = library
        .save_file(&store, &job.id, None, "Document", &Control::default())
        .unwrap();
    fs::remove_file(&original).unwrap();
    drop(store);
    drop(library);
    let library = Library::new(root.path().join("experiments"));
    let store = DocumentStore::new().unwrap();
    let Opened::File {
        document: restored,
        job: restored_job,
        ..
    } = library
        .open(&saved.id, &store, &Control::default())
        .unwrap()
    else {
        panic!("expected file")
    };
    assert_ne!(restored.id, document.id);
    assert_ne!(restored_job.id, job.id);
    assert!(restored_job.restored);
    assert!(restored.bom);
    assert_eq!(restored.digest, document.digest);
    assert_eq!(restored_job.stages.len(), 1);
    assert!(!store.preview_document(&restored.id, 0).unwrap().eof);
    let restored_source = store.job(&restored_job.id).unwrap().source.clone();
    assert_eq!(
        fs::read_to_string(&restored_source.payload).unwrap(),
        text.trim_start_matches('\u{feff}')
    );
    let metadata = fs::read_to_string(
        root.path()
            .join("experiments")
            .join(&saved.id)
            .join("manifest.json"),
    )
    .unwrap();
    assert!(!metadata.contains(root.path().to_str().unwrap()));
    assert!(!metadata.contains("privatepassphrase"));
    library.delete(&saved.id).unwrap();
    assert!(store.preview_job(&restored_job.id, None, 0).is_ok());
    store.discard_job(&restored_job.id).unwrap();
    assert!(store.preview_document(&restored.id, 0).is_ok());
}

#[test]
fn invalid_entry_does_not_hide_healthy_and_cancelled_save_does_not_publish() {
    let root = tempfile::tempdir().unwrap();
    let library = Library::new(root.path().join("experiments"));
    let run = library
        .evaluate(recipe(), "Hello".into(), "".into(), "encode".into(), vec![])
        .unwrap();
    let saved = library
        .save_text(&run.run_id, "Healthy", &Control::default())
        .unwrap();
    let cancelled = Control::default();
    cancelled.cancel();
    assert!(
        library
            .save_text(&run.run_id, "Cancelled", &cancelled)
            .is_err()
    );
    let corrupt = root.path().join("experiments/experiment-corrupt");
    fs::create_dir(&corrupt).unwrap();
    fs::write(corrupt.join("manifest.json"), "{}").unwrap();
    let listed = library.list().unwrap();
    assert_eq!(listed.entries.len(), 1);
    assert_eq!(listed.entries[0].id, saved.id);
    assert_eq!(listed.problems.len(), 1);
    assert!(library.delete("../outside").is_err());
    assert!(
        library
            .open(
                "../outside",
                &DocumentStore::new().unwrap(),
                &Control::default()
            )
            .is_err()
    );
}

#[test]
fn reopened_reading_remaps_choices_and_requires_new_key_for_full_verification() {
    use super::reconstruct::{self, DocChoice};
    let root = tempfile::tempdir().unwrap();
    let source = root.path().join("words.txt");
    fs::write(&source, "a a").unwrap();
    let store = DocumentStore::new().unwrap();
    let document = store.capture(&source, &Control::default()).unwrap();
    let mut recipe = recipe();
    recipe.steps = vec![
        Step {
            enabled: true,
            rule: Rule::PhoneticRotate,
        },
        Step {
            enabled: true,
            rule: Rule::DerivedCaesar,
        },
    ];
    let encoded = store
        .start(
            &document.id,
            recipe.clone(),
            "lemon".into(),
            Direction::Encode,
        )
        .unwrap();
    finish(&store, &encoded.id);
    let ciphertext = store.job(&encoded.id).unwrap().result().unwrap();
    let document = store
        .capture(&ciphertext.path, &Control::default())
        .unwrap();
    let decoded = store
        .start(&document.id, recipe, "lemon".into(), Direction::Decode)
        .unwrap();
    finish(&store, &decoded.id);
    let reading = reconstruct::run(
        &store,
        &decoded.id,
        &[DocChoice {
            word_id: format!("{}:1", decoded.id),
            candidate: "eye".into(),
        }],
        &Control::default(),
    )
    .unwrap();
    let library = Library::new(root.path().join("experiments"));
    assert!(
        library
            .save_file(
                &store,
                &decoded.id,
                Some("unrelated-reading"),
                "Bad reading",
                &Control::default()
            )
            .is_err()
    );
    let saved = library
        .save_file(
            &store,
            &decoded.id,
            Some(&reading.id),
            "Reading",
            &Control::default(),
        )
        .unwrap();
    drop(store);
    drop(library);
    let store = DocumentStore::new().unwrap();
    let library = Library::new(root.path().join("experiments"));
    let Opened::File {
        job,
        reading: Some(restored),
        choices,
        ..
    } = library
        .open(&saved.id, &store, &Control::default())
        .unwrap()
    else {
        panic!("missing reading")
    };
    assert_ne!(restored.id, reading.id);
    assert_eq!(choices[0].word_id, format!("{}:1", job.id));
    assert_eq!(choices[0].candidate, "eye");
    let page = reconstruct::page(&store, &job.id, &restored.id, 0).unwrap();
    assert_eq!(page.words[1].word_id, format!("{}:1", job.id));
    assert!(
        reconstruct::preview(&store, &job.id, &restored.id, 0)
            .unwrap()
            .text
            .ends_with("eye")
    );
    assert!(reconstruct::run(&store, &job.id, &choices, &Control::default()).is_err());
    assert!(
        reconstruct::run_with_key(
            &store,
            &job.id,
            &choices,
            Some("orange"),
            &Control::default()
        )
        .is_err()
    );
    assert_eq!(
        reconstruct::current(&store, &job.id, &restored.id)
            .unwrap()
            .info
            .id,
        restored.id
    );
    let next = reconstruct::run_with_key(
        &store,
        &job.id,
        &choices,
        Some("lemon"),
        &Control::default(),
    )
    .unwrap();
    assert!(
        reconstruct::preview(&store, &job.id, &next.id, 0)
            .unwrap()
            .text
            .ends_with("eye")
    );
    let saved_again = library
        .save_file(
            &store,
            &job.id,
            Some(&next.id),
            "Again",
            &Control::default(),
        )
        .unwrap();
    assert_ne!(saved_again.id, saved.id);
    let manifest = root
        .path()
        .join("experiments")
        .join(&saved_again.id)
        .join("manifest.json");
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    value["capture"]["capture"]["reading"]["words"] = "999".into();
    fs::write(&manifest, serde_json::to_vec(&value).unwrap()).unwrap();
    let listing = library.list().unwrap();
    assert_eq!(listing.entries.len(), 1);
    assert_eq!(listing.problems.len(), 1);
}

#[test]
fn tampered_missing_oversized_and_unsupported_entries_are_reported_individually() {
    let root = tempfile::tempdir().unwrap();
    let library = Library::new(root.path().join("experiments"));
    let source = root.path().join("input.txt");
    fs::write(&source, "Hello").unwrap();
    let store = DocumentStore::new().unwrap();
    let document = store.capture(&source, &Control::default()).unwrap();
    let job = store
        .start(&document.id, recipe(), "".into(), Direction::Encode)
        .unwrap();
    finish(&store, &job.id);
    let healthy = library
        .save_file(&store, &job.id, None, "Healthy", &Control::default())
        .unwrap();
    for action in [
        "tampered",
        "missing",
        "unsupported",
        "oversized",
        "invalid-recipe",
    ] {
        let saved = library
            .save_file(&store, &job.id, None, action, &Control::default())
            .unwrap();
        let dir = root.path().join("experiments").join(&saved.id);
        match action {
            "tampered" => fs::write(dir.join("stage-0"), "Wrong").unwrap(),
            "missing" => fs::remove_file(dir.join("result")).unwrap(),
            "oversized" => {
                fs::write(dir.join("manifest.json"), vec![b' '; 4 * 1024 * 1024 + 1]).unwrap()
            }
            _ => {
                let path = dir.join("manifest.json");
                let mut value: serde_json::Value =
                    serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
                if action == "unsupported" {
                    value["schemaVersion"] = 999.into();
                } else {
                    value["capture"]["capture"]["recipe"]["schemaVersion"] = 999.into();
                }
                fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
            }
        }
        assert!(
            library
                .open(&saved.id, &store, &Control::default())
                .is_err(),
            "{action}"
        );
    }
    let listed = library.list().unwrap();
    assert_eq!(listed.entries.len(), 1);
    assert_eq!(listed.entries[0].id, healthy.id);
    assert_eq!(listed.problems.len(), 5);
    assert_eq!(fs::read_to_string(source).unwrap(), "Hello");
}

#[cfg(unix)]
#[test]
fn symlink_entries_artifacts_and_manifests_are_rejected_without_touching_targets() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let library = Library::new(root.path().join("experiments"));
    let source = root.path().join("input.txt");
    fs::write(&source, "Hello").unwrap();
    let store = DocumentStore::new().unwrap();
    let document = store.capture(&source, &Control::default()).unwrap();
    let job = store
        .start(&document.id, recipe(), "".into(), Direction::Encode)
        .unwrap();
    finish(&store, &job.id);
    let saved = library
        .save_file(&store, &job.id, None, "Artifact", &Control::default())
        .unwrap();
    let dir = root.path().join("experiments").join(&saved.id);
    fs::remove_file(dir.join("source")).unwrap();
    symlink(&source, dir.join("source")).unwrap();
    assert!(
        library
            .open(&saved.id, &store, &Control::default())
            .is_err()
    );
    let text = library
        .evaluate(recipe(), "Hello".into(), "".into(), "encode".into(), vec![])
        .unwrap();
    let saved_text = library
        .save_text(&text.run_id, "Manifest", &Control::default())
        .unwrap();
    let manifest = root
        .path()
        .join("experiments")
        .join(&saved_text.id)
        .join("manifest.json");
    fs::remove_file(&manifest).unwrap();
    symlink(&source, &manifest).unwrap();
    assert!(
        library
            .open(&saved_text.id, &store, &Control::default())
            .is_err()
    );
    symlink(
        root.path(),
        root.path().join("experiments/experiment-symlink"),
    )
    .unwrap();
    assert!(library.delete("experiment-symlink").is_err());
    assert_eq!(library.list().unwrap().problems.len(), 3);
    library.delete(&saved.id).unwrap();
    assert_eq!(fs::read_to_string(&source).unwrap(), "Hello");
}

#[test]
fn inconsistent_manifest_cannot_hide_lost_stages_or_forge_source_metadata() {
    let root = tempfile::tempdir().unwrap();
    let library = Library::new(root.path().join("experiments"));
    let source = root.path().join("input.txt");
    fs::write(&source, "Hello").unwrap();
    let store = DocumentStore::new().unwrap();
    let document = store.capture(&source, &Control::default()).unwrap();
    let job = store
        .start(&document.id, recipe(), "".into(), Direction::Encode)
        .unwrap();
    finish(&store, &job.id);
    for action in [
        "missing-stage",
        "source-metadata",
        "result-consistency",
        "exact-flag",
    ] {
        let saved = library
            .save_file(&store, &job.id, None, action, &Control::default())
            .unwrap();
        let path = root
            .path()
            .join("experiments")
            .join(&saved.id)
            .join("manifest.json");
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        match action {
            "missing-stage" => {
                value["capture"]["capture"]["stages"] = serde_json::json!([]);
                value["artifacts"]
                    .as_array_mut()
                    .unwrap()
                    .retain(|a| a["role"]["role"] != "stage");
            }
            "source-metadata" => {
                value["capture"]["capture"]["document"]["digest"] = "invalid".into()
            }
            "result-consistency" => {
                let source_stats = value["artifacts"][0]["stats"].clone();
                value["artifacts"][1]["stats"] = source_stats;
                fs::copy(&source, path.parent().unwrap().join("result")).unwrap();
            }
            _ => value["capture"]["capture"]["exact"] = false.into(),
        }
        fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        assert!(
            library
                .open(&saved.id, &store, &Control::default())
                .is_err(),
            "{action}"
        );
    }
    let listing = library.list().unwrap();
    assert!(listing.entries.is_empty());
    assert_eq!(listing.problems.len(), 4);
}

#[test]
fn changed_working_artifact_is_not_published_as_a_completed_save() {
    let root = tempfile::tempdir().unwrap();
    let library = Library::new(root.path().join("experiments"));
    let source = root.path().join("input.txt");
    fs::write(&source, "Hello").unwrap();
    let store = DocumentStore::new().unwrap();
    let document = store.capture(&source, &Control::default()).unwrap();
    let job = store
        .start(&document.id, recipe(), "".into(), Direction::Encode)
        .unwrap();
    finish(&store, &job.id);
    fs::write(store.job(&job.id).unwrap().result().unwrap().path, "Wrong").unwrap();
    assert!(
        library
            .save_file(&store, &job.id, None, "Corrupted", &Control::default())
            .is_err()
    );
    let listing = library.list().unwrap();
    assert!(listing.entries.is_empty());
    assert!(listing.problems.is_empty());
}

#[test]
fn incomplete_directories_do_not_consume_the_completed_entry_limit() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("experiments");
    fs::create_dir(&directory).unwrap();
    for index in 0..200 {
        fs::create_dir(directory.join(format!("experiment-incomplete-{index}"))).unwrap();
    }
    let library = Library::new(directory);
    let run = library
        .evaluate(recipe(), "Hello".into(), "".into(), "encode".into(), vec![])
        .unwrap();
    library
        .save_text(&run.run_id, "Healthy", &Control::default())
        .unwrap();
    let listing = library.list().unwrap();
    assert_eq!(listing.entries.len(), 1);
    assert_eq!(listing.problems.len(), 200);
}

#[test]
fn independent_libraries_compete_for_the_last_slot_without_losing_entries() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("experiments");
    let library = Library::new(directory.clone());
    let run = library
        .evaluate(recipe(), "Hello".into(), "".into(), "encode".into(), vec![])
        .unwrap();
    let first = library
        .save_text(&run.run_id, "Healthy", &Control::default())
        .unwrap();
    let bytes = fs::read(directory.join(&first.id).join("manifest.json")).unwrap();
    for index in 1..199 {
        let id = format!("experiment-fixture-{index}");
        let path = directory.join(&id);
        fs::create_dir(&path).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["summary"]["id"] = id.into();
        let mut payload = value.clone();
        payload.as_object_mut().unwrap().remove("metadataDigest");
        use sha2::Digest;
        value["metadataDigest"] =
            super::stream::digest_hex(&sha2::Sha256::digest(serde_json::to_vec(&payload).unwrap()))
                .into();
        fs::write(
            path.join("manifest.json"),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
    }
    let second_library = Library::new(directory.clone());
    let second = second_library
        .evaluate(recipe(), "World".into(), "".into(), "encode".into(), vec![])
        .unwrap();
    let start = std::sync::Barrier::new(2);
    let save = |library: &Library, run_id: &str| {
        start.wait();
        library.save_text(run_id, "Last slot", &Control::default())
    };
    let results = std::thread::scope(|scope| {
        let other = scope.spawn(|| save(&second_library, &second.run_id));
        [save(&library, &run.run_id), other.join().unwrap()]
    });
    let published: Vec<_> = results.iter().filter_map(|r| r.as_ref().ok()).collect();
    assert_eq!(
        published.len(),
        1,
        "Only one writer can publish: {results:?}"
    );
    assert!(
        results
            .iter()
            .any(|r| r.as_ref().is_err_and(|e| e.contains("library is full")))
    );
    assert!(
        library
            .save_text(&run.run_id, "Overflow", &Control::default())
            .is_err()
    );
    let listing = library.list().unwrap();
    assert_eq!(listing.entries.len(), 200);
    assert!(
        listing
            .entries
            .iter()
            .any(|entry| entry.id == published[0].id)
    );
    assert!(listing.problems.is_empty());
}

#[test]
fn restored_exact_text_decode_reruns_with_its_capture() {
    let root = tempfile::tempdir().unwrap();
    let library = Library::new(root.path().join("experiments"));
    let run = library
        .evaluate(
            recipe(),
            "Khoor!".into(),
            "".into(),
            "decode".into(),
            vec![],
        )
        .unwrap();
    let saved = library
        .save_text(&run.run_id, "Exact", &Control::default())
        .unwrap();
    let Opened::Text {
        source,
        run_id,
        recipe,
        ..
    } = library
        .open(
            &saved.id,
            &DocumentStore::new().unwrap(),
            &Control::default(),
        )
        .unwrap()
    else {
        panic!("expected text")
    };
    let repeated = library.revise(&run_id, &recipe, &source, "", &[]).unwrap();
    assert!(repeated.result.exact);
    assert_eq!(repeated.result.output, "Hello!");
    assert_ne!(repeated.run_id, run_id);
}

#[test]
fn restored_intermediate_only_schema_one_reruns() {
    check_intermediate_only_reruns(1);
}

#[test]
fn restored_intermediate_only_schema_two_reruns() {
    check_intermediate_only_reruns(2);
}

fn check_intermediate_only_reruns(schema: u32) {
    let root = tempfile::tempdir().unwrap();
    let library = Library::new(root.path().join("experiments"));
    let store = DocumentStore::new().unwrap();
    for (source, rules, has_dictionary, key) in [
        ("hll", vec![Rule::RemoveVowels], false, ""),
        (
            "zqzqzq",
            vec![Rule::RemoveVowels, Rule::CollapseDoubles, Rule::Vigenere],
            true,
            "a",
        ),
    ] {
        let mut recipe = recipe();
        recipe.schema_version = schema;
        recipe.steps = rules
            .into_iter()
            .map(|rule| Step {
                enabled: true,
                rule,
            })
            .collect();
        let run = library
            .evaluate(
                recipe.clone(),
                source.into(),
                key.into(),
                "decode".into(),
                vec![],
            )
            .unwrap();
        assert_eq!(run.result.intermediate, source);
        assert!(!run.result.exact);
        assert!(run.result.output.is_empty());
        assert_eq!(run.result.reconstruction.is_some(), has_dictionary);
        if let Some(reading) = &run.result.reconstruction {
            assert!(reading.suggested.is_none());
        }

        let saved = library
            .save_text(&run.run_id, "Surviving layer", &Control::default())
            .unwrap();
        let Opened::Text { run_id, .. } = library
            .open(&saved.id, &store, &Control::default())
            .unwrap()
        else {
            panic!("expected text")
        };
        if has_dictionary {
            let changed = crate::recipes::decode(&recipe, source, "b", &[]).unwrap();
            assert_ne!(changed.intermediate, source);
            assert!(changed.output.is_empty());
            assert!(library.revise(&run_id, &recipe, source, "b", &[]).is_err());
            let pin = crate::engine::WordChoice {
                index: 0,
                candidate: "the".into(),
            };
            assert!(
                library
                    .revise(&run_id, &recipe, source, key, &[pin])
                    .is_err()
            );
        }
        let repeated = library.revise(&run_id, &recipe, source, key, &[]).unwrap();
        assert_ne!(repeated.run_id, run_id);
        assert_eq!(repeated.result.intermediate, source);
        assert!(repeated.result.output.is_empty());
        assert_eq!(repeated.result.reconstruction.is_some(), has_dictionary);
    }
}

#[cfg(windows)]
#[test]
fn ordinary_windows_long_library_path_round_trips() {
    use std::{os::windows::ffi::OsStrExt, path::PathBuf};
    // Start relative to this fixture's parent so the library receives an
    // ordinary path even when the test runner uses a verbatim working directory.
    let fixture = tempfile::tempdir_in(".").unwrap();
    let mut directory = PathBuf::from(fixture.path().file_name().unwrap());
    while directory.as_os_str().encode_wide().count() <= 260 {
        directory.push("long-redirected-profile-component");
    }
    let library = Library::new(directory.clone());
    let run = library
        .evaluate(
            recipe(),
            "Hello!".into(),
            "".into(),
            "encode".into(),
            vec![],
        )
        .unwrap();
    let saved = library
        .save_text(&run.run_id, "Long path", &Control::default())
        .unwrap();
    let library = Library::new(directory);
    let listing = library.list().unwrap();
    assert!(listing.problems.is_empty());
    assert_eq!(listing.entries[0].id, saved.id);
    let Opened::Text { source, result, .. } = library
        .open(
            &saved.id,
            &DocumentStore::new().unwrap(),
            &Control::default(),
        )
        .unwrap()
    else {
        panic!("expected text")
    };
    assert_eq!(source, "Hello!");
    assert_eq!(result.output, "Khoor!");
    library.delete(&saved.id).unwrap();
    assert!(library.list().unwrap().entries.is_empty());
}

#[cfg(unix)]
#[test]
fn persisted_and_restored_content_directories_are_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let library = Library::new(root.path().join("experiments"));
    let source = root.path().join("source");
    fs::write(&source, "Private source").unwrap();
    let store = DocumentStore::new().unwrap();
    let document = store.capture(&source, &Control::default()).unwrap();
    let job = store
        .start(&document.id, recipe(), "".into(), Direction::Encode)
        .unwrap();
    finish(&store, &job.id);
    let saved = library
        .save_file(&store, &job.id, None, "Private", &Control::default())
        .unwrap();
    let Opened::File { job: restored, .. } = library
        .open(&saved.id, &store, &Control::default())
        .unwrap()
    else {
        panic!("expected file")
    };
    let working = store.job(&restored.id).unwrap();
    for path in [
        root.path().join("experiments"),
        root.path().join("experiments").join(&saved.id),
        working.root.path().to_path_buf(),
        working.source._root.path().to_path_buf(),
    ] {
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o700,
            "{}",
            path.display()
        );
    }
    let text = library
        .evaluate(
            recipe(),
            "Private text".into(),
            "".into(),
            "encode".into(),
            vec![],
        )
        .unwrap();
    let saved = library
        .save_text(&text.run_id, "Private text", &Control::default())
        .unwrap();
    assert_eq!(
        fs::metadata(root.path().join("experiments").join(saved.id))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
}

#[test]
fn parseable_text_capture_corruption_is_rejected_after_restart() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("experiments");
    let library = Library::new(directory.clone());
    let run = library
        .evaluate(
            recipe(),
            "Hello".into(),
            "privatepassphrase".into(),
            "encode".into(),
            vec![],
        )
        .unwrap();
    let healthy = library
        .save_text(&run.run_id, "Healthy", &Control::default())
        .unwrap();
    let mut corrupted = Vec::new();
    for field in [
        "output",
        "intermediate",
        "stage",
        "choices",
        "recipe",
        "source",
        "direction",
    ] {
        let saved = library
            .save_text(&run.run_id, field, &Control::default())
            .unwrap();
        let path = directory.join(&saved.id).join("manifest.json");
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        let capture = &mut value["capture"]["capture"];
        match field {
            "output" => capture["result"]["output"] = "Wrong".into(),
            "intermediate" => capture["result"]["intermediate"] = "Wrong".into(),
            "stage" => capture["result"]["stages"][0]["text"] = "Wrong".into(),
            "choices" => capture["choices"] = serde_json::json!([{"index":0,"candidate":"wrong"}]),
            "recipe" => capture["recipe"]["steps"][0]["rule"]["shift"] = 4.into(),
            "source" => capture["source"] = "Wrong".into(),
            _ => capture["direction"] = "decode".into(),
        }
        fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
        corrupted.push(saved.id);
    }
    drop(library);
    let library = Library::new(directory);
    let store = DocumentStore::new().unwrap();
    for id in &corrupted {
        assert!(
            library.open(id, &store, &Control::default()).is_err(),
            "{id}"
        );
    }
    let listed = library.list().unwrap();
    assert_eq!(listed.entries.len(), 1);
    assert_eq!(listed.entries[0].id, healthy.id);
    assert_eq!(listed.problems.len(), 7);
}

#[test]
fn selected_text_reading_must_reencode_to_the_complete_captured_ciphertext() {
    let root = tempfile::tempdir().unwrap();
    let library = Library::new(root.path().join("experiments"));
    let mut recipe = recipe();
    recipe.schema_version = 1;
    recipe.steps = vec![
        Step {
            enabled: true,
            rule: Rule::RemoveVowels,
        },
        Step {
            enabled: true,
            rule: Rule::CollapseDoubles,
        },
    ];
    let choices = vec![crate::engine::WordChoice {
        index: 0,
        candidate: "the".into(),
    }];
    let original = library
        .evaluate(
            recipe.clone(),
            "Th!".into(),
            "".into(),
            "decode".into(),
            choices.clone(),
        )
        .unwrap(); // Standalone evaluation keeps its prior semantics.
    let saved = library
        .save_text(&original.run_id, "Preserved", &Control::default())
        .unwrap();
    let Opened::Text { source, run_id, .. } = library
        .open(
            &saved.id,
            &DocumentStore::new().unwrap(),
            &Control::default(),
        )
        .unwrap()
    else {
        panic!("expected text")
    };
    assert!(
        library
            .revise(&run_id, &recipe, &source, "", &choices)
            .is_err()
    );
    assert!(library.revise(&run_id, &recipe, &source, "", &[]).is_err());
    assert!(
        library
            .revise(&run_id, &recipe, "th", "", &choices)
            .is_err()
    );
    let mut edited = recipe.clone();
    edited.steps.push(Step {
        enabled: true,
        rule: Rule::Caesar { shift: 3 },
    });
    assert!(
        library
            .revise(&run_id, &edited, &source, "", &choices)
            .is_err()
    );
    let preserved = library
        .save_text(&run_id, "Still preserved", &Control::default())
        .unwrap();
    let Opened::Text { source: still, .. } = library
        .open(
            &preserved.id,
            &DocumentStore::new().unwrap(),
            &Control::default(),
        )
        .unwrap()
    else {
        panic!("expected text")
    };
    assert_eq!(still, "Th!");
    let valid = library
        .evaluate(
            recipe.clone(),
            "th".into(),
            "".into(),
            "decode".into(),
            vec![],
        )
        .unwrap();
    let next = library
        .revise(&valid.run_id, &recipe, "th", "", &choices)
        .unwrap();
    assert_eq!(next.result.output, "the");
    let reset = library
        .revise(&next.run_id, &recipe, "th", "", &[])
        .unwrap();
    assert!(!reset.result.output.is_empty());
}

#[test]
fn candidate_records_changed_after_reconstruction_cannot_become_a_new_capture() {
    use super::reconstruct;
    let root = tempfile::tempdir().unwrap();
    let library = Library::new(root.path().join("experiments"));
    let source = root.path().join("source");
    fs::write(&source, "th").unwrap();
    let store = DocumentStore::new().unwrap();
    let document = store.capture(&source, &Control::default()).unwrap();
    let mut recipe = recipe();
    recipe.steps = vec![
        Step {
            enabled: true,
            rule: Rule::RemoveVowels,
        },
        Step {
            enabled: true,
            rule: Rule::CollapseDoubles,
        },
    ];
    let job = store
        .start(&document.id, recipe, "".into(), Direction::Decode)
        .unwrap();
    finish(&store, &job.id);
    let info = reconstruct::run(&store, &job.id, &[], &Control::default()).unwrap();
    let reading = reconstruct::current(&store, &job.id, &info.id).unwrap();
    let mut value: serde_json::Value =
        serde_json::from_slice(&fs::read(&reading.candidates).unwrap()).unwrap();
    value["alternatives"][0] = "synthetic".into();
    let mut bytes = serde_json::to_vec(&value).unwrap();
    bytes.push(b'\n');
    fs::write(&reading.candidates, bytes).unwrap();
    assert!(
        library
            .save_file(
                &store,
                &job.id,
                Some(&info.id),
                "Changed candidates",
                &Control::default()
            )
            .is_err()
    );
    assert!(library.list().unwrap().entries.is_empty());
}

#[test]
fn deletion_preserves_unknown_and_incomplete_directories() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("experiments");
    fs::create_dir(&directory).unwrap();
    let unknown = directory.join("experiment-unfamiliar");
    fs::create_dir(&unknown).unwrap();
    fs::write(unknown.join("human-notes.txt"), "Keep me").unwrap();
    let before = fs::metadata(&unknown).unwrap().permissions();
    let library = Library::new(directory);
    assert!(library.delete("experiment-unfamiliar").is_err());
    assert_eq!(fs::metadata(&unknown).unwrap().permissions(), before);
    assert_eq!(
        fs::read_to_string(unknown.join("human-notes.txt")).unwrap(),
        "Keep me"
    );
}

#[test]
fn deletion_requires_original_owned_metadata_but_allows_damaged_artifact_cleanup() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("experiments");
    let library = Library::new(directory.clone());
    let source = root.path().join("source");
    fs::write(&source, "Hello").unwrap();
    let store = DocumentStore::new().unwrap();
    let document = store.capture(&source, &Control::default()).unwrap();
    let job = store
        .start(&document.id, recipe(), "".into(), Direction::Encode)
        .unwrap();
    finish(&store, &job.id);
    let saved = library
        .save_file(&store, &job.id, None, "Owned", &Control::default())
        .unwrap();
    let entry = directory.join(&saved.id);
    let original = root.path().join("original-owned-entry");
    fs::rename(&entry, &original).unwrap();
    fs::create_dir(&entry).unwrap();
    fs::write(entry.join("human-notes"), "Keep me").unwrap();
    assert!(library.delete(&saved.id).is_err());
    assert_eq!(
        fs::read_to_string(entry.join("human-notes")).unwrap(),
        "Keep me"
    );
    fs::remove_file(entry.join("human-notes")).unwrap();
    fs::remove_dir(&entry).unwrap();
    fs::rename(&original, &entry).unwrap();
    fs::write(entry.join("result"), "damaged").unwrap();
    assert!(
        library
            .open(&saved.id, &store, &Control::default())
            .is_err()
    );
    library.delete(&saved.id).unwrap();
    assert!(!entry.exists());
    assert_eq!(fs::read_to_string(source).unwrap(), "Hello");
}
