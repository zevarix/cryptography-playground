//! Independent temporary document and reading reconstruction.
use super::*;

pub(super) fn restore_file(
    summary: Summary,
    capture: FileCapture,
    path: &Path,
    artifacts: &[SavedArtifact],
    store: &DocumentStore,
    control: &Control,
) -> Result<Opened, String> {
    let snapshot_root = tempfile::Builder::new()
        .prefix("restored-source-")
        .tempdir()
        .map_err(|e| io_error("Cannot create restored source", e))?;
    let raw_source = snapshot_root.path().join("source");
    let mut raw = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&raw_source)
        .map_err(|e| io_error("Cannot create restored source", e))?;
    if capture.document.bom {
        raw.write_all(b"\xef\xbb\xbf")
            .map_err(|e| io_error("Cannot restore source BOM", e))?;
    }
    let mut source =
        File::open(path.join("source")).map_err(|e| io_error("Cannot open saved source", e))?;
    let mut buffer = [0_u8; BUFFER];
    loop {
        control.check()?;
        let count = source
            .read(&mut buffer)
            .map_err(|e| io_error("Cannot read saved source", e))?;
        if count == 0 {
            break;
        }
        raw.write_all(&buffer[..count])
            .map_err(|e| io_error("Cannot restore source", e))?;
    }
    raw.sync_all()
        .map_err(|e| io_error("Cannot flush restored source", e))?;
    drop(raw);
    let mut snapshot = storage::capture(&raw_source, &std::env::temp_dir(), control)?;
    if snapshot.info.digest != capture.document.digest
        || snapshot.info.bytes != capture.document.bytes
        || snapshot.info.bom != capture.document.bom
        || snapshot.info.legacy_compatible != capture.document.legacy_compatible
    {
        return Err("Saved source metadata does not match its contents.".into());
    }
    snapshot.info.name = capture.document.name;
    // The snapshot owns its payload. Move the complete source into that same
    // owned directory so export protection never retains a library pathname.
    let owned_source = snapshot._root.path().join("source");
    fs::rename(&raw_source, &owned_source)
        .map_err(|e| io_error("Cannot retain restored source", e))?;
    snapshot.source = owned_source.clone();
    snapshot.canonical_source = owned_source
        .canonicalize()
        .map_err(|e| io_error("Cannot resolve restored source", e))?;
    // Snapshot and job directories have independent ownership: discarding a
    // job must leave the adopted source document available for another run.
    let root = snapshot_root;
    let id = fresh_id("job-")?;
    let lookup: HashMap<_, _> = artifacts.iter().map(|a| (&a.role, &a.stats)).collect();
    let artifact = |role: Role, label: String, lossy: bool| -> Result<Artifact, String> {
        let destination = root.path().join(format!("working-{}", role.filename()));
        copy_checked(&path.join(role.filename()), &destination, control)?;
        let expected = lookup.get(&role).ok_or("Missing artifact role.")?;
        let actual = stats(&destination, control)?;
        if actual.bytes != expected.bytes
            || actual.scalars != expected.scalars
            || actual.digest != expected.digest
        {
            return Err("An experiment artifact changed while it was being opened. The current notebook is preserved.".into());
        }
        Ok(Artifact {
            path: destination,
            stats: actual,
            label,
            lossy,
        })
    };
    let mut stages = Vec::new();
    for (index, stage) in capture.stages.iter().enumerate() {
        let item = artifact(Role::Stage { index }, stage.label.clone(), stage.lossy)?;
        if item.stats.bytes.to_string() != stage.bytes {
            return Err("Saved stage length is inconsistent.".into());
        }
        stages.push(item);
    }
    let result = artifact(Role::Result, "Exact result".into(), !capture.exact)?;
    let (reading, choices) = if let Some(saved) = capture.reading {
        let reading_root = tempfile::Builder::new()
            .prefix("reading-")
            .tempdir_in(root.path())
            .map_err(|e| io_error("Cannot restore reading", e))?;
        let reading_id = reading_root
            .path()
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("Cannot identify restored reading.")?
            .to_owned();
        let reading_artifact = artifact(Role::Reading, "Speculative English reading".into(), true)?;
        let candidates = reading_root.path().join("candidates.jsonl");
        let saved_candidates = artifact(Role::Candidates, String::new(), false)?;
        let count = remap_candidates(&saved_candidates.path, &candidates, &id, control)?;
        if count.to_string() != saved.words {
            return Err("Saved reading word count is inconsistent.".into());
        }
        let mut choices = Vec::new();
        for choice in saved.choices {
            let (_, index) = choice
                .word_id
                .rsplit_once(':')
                .ok_or("Invalid saved choice ID.")?;
            let index: u64 = index
                .parse()
                .map_err(|_| "Invalid saved choice position.")?;
            if index >= count {
                return Err("Saved choice is outside the reading.".into());
            }
            choices.push(DocChoice {
                word_id: format!("{id}:{index}"),
                candidate: choice.candidate,
            });
        }
        let info = ReadingInfo {
            id: reading_id,
            job_id: id.clone(),
            words: saved.words,
            note: saved.note,
        };
        (
            Some(Arc::new(Reading {
                _root: reading_root,
                info,
                artifact: reading_artifact,
                candidates,
                choices: choices.clone(),
            })),
            choices,
        )
    } else {
        (None, Vec::new())
    };
    let document = snapshot.info.clone();
    let info = JobInfo {
        id: id.clone(),
        document_id: document.id.clone(),
        state: "complete".into(),
        phase: "Complete".into(),
        processed_bytes: result.stats.bytes.to_string(),
        total_bytes: result.stats.bytes.to_string(),
        fingerprint: fingerprint::fingerprint(&capture.recipe)?,
        recipe: capture.recipe,
        direction: capture.direction,
        exact: capture.exact,
        restored: true,
        stages: capture.stages,
        note: capture.note,
    };
    let reading_info = reading.as_ref().map(|r| r.info.clone());
    control.check()?;
    store.adopt_restored(Arc::new(Job {
        root,
        source: Arc::new(snapshot),
        control: Control::default(),
        key: String::new(),
        state: Mutex::new(JobState {
            info: info.clone(),
            artifacts: stages,
            result: Some(result),
            reading,
            reconstructing: false,
        }),
    }))?;
    Ok(Opened::File {
        summary,
        document,
        job: Box::new(info),
        reading: reading_info,
        choices,
    })
}
fn remap_candidates(
    source: &Path,
    destination: &Path,
    job_id: &str,
    control: &Control,
) -> Result<u64, String> {
    let mut output = BufWriter::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
            .map_err(|e| io_error("Cannot create restored candidates", e))?,
    );
    let count = scan_candidates(source, control, |mut word, index| {
        word.word_id = format!("{job_id}:{index}");
        serde_json::to_writer(&mut output, &word)
            .map_err(|_| "Cannot write restored candidates.")?;
        output
            .write_all(b"\n")
            .map_err(|e| io_error("Cannot finish restored candidate", e))
    })?;
    output
        .flush()
        .and_then(|()| output.get_ref().sync_all())
        .map_err(|e| io_error("Cannot flush restored candidates", e))?;
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restored_working_artifacts_are_verified_after_library_validation() {
        let root = tempfile::tempdir().unwrap();
        let library = Library::new(root.path().join("experiments"));
        let store = DocumentStore::new().unwrap();
        let source = root.path().join("source");
        fs::write(&source, "Hello").unwrap();
        let document = store.capture(&source, &Control::default()).unwrap();
        let recipe = Recipe {
            schema_version: 2,
            id: "user-restore".into(),
            name: "Restore".into(),
            description: String::new(),
            steps: vec![crate::recipes::Step {
                enabled: true,
                rule: crate::recipes::Rule::Caesar { shift: 3 },
            }],
        };
        let job = store
            .start(&document.id, recipe, "".into(), Direction::Encode)
            .unwrap();
        let start = std::time::Instant::now();
        while store.status(&job.id).unwrap().state == "running" {
            assert!(start.elapsed() < std::time::Duration::from_secs(20));
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let saved = library
            .save_file(&store, &job.id, None, "Test", &Control::default())
            .unwrap();
        let path = library.root.join(&saved.id);
        let manifest = read_manifest(&path, &saved.id, &Control::default()).unwrap();
        fs::write(path.join("stage-0"), "Wrong").unwrap();
        let Capture::File { capture } = manifest.capture else {
            panic!("expected file")
        };
        assert!(
            restore_file(
                manifest.summary,
                capture,
                &path,
                &manifest.artifacts,
                &store,
                &Control::default()
            )
            .is_err()
        );
    }
}
