//! Bounded manifests, fixed artifact roles and verified disk I/O.
use super::*;

pub(super) fn validate_name(name: &str) -> Result<(), String> {
    if name.trim().is_empty() || name.len() > 256 || name.chars().any(char::is_control) {
        return Err("Experiment names require 1–256 bytes without control characters.".into());
    }
    Ok(())
}
pub(super) fn entry_path(root: &Path, id: &str) -> Result<PathBuf, String> {
    if !id.starts_with("experiment-")
        || id.len() > 80
        || id.len() <= 11
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err("Invalid experiment ID.".into());
    }
    Ok(root.join(id))
}
pub(super) fn regular_directory(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|e| io_error("Cannot inspect experiment directory", e))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("Experiment directories must be regular directories, never symlinks.".into());
    }
    Ok(())
}
fn regular_file(path: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|e| io_error("Cannot inspect experiment artifact", e))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("Experiment artifacts must be regular files, never symlinks.".into());
    }
    Ok(())
}
pub(super) fn ensure_root(root: &Path) -> Result<PathBuf, String> {
    if !root
        .try_exists()
        .map_err(|e| io_error("Cannot inspect experiment storage", e))?
    {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(root)
            .map_err(|e| io_error("Cannot create experiment storage", e))?;
    }
    secure_directory(root)?;
    // Windows canonicalization supplies the extended absolute spelling needed
    // by both tempfile's manifest publication and the directory's MoveFileExW.
    #[cfg(windows)]
    {
        fs::canonicalize(root).map_err(|e| io_error("Cannot resolve experiment storage", e))
    }
    #[cfg(not(windows))]
    {
        Ok(root.to_owned())
    }
}

pub(super) fn lock_for_save(root: &Path) -> Result<File, String> {
    // Leave this empty app-owned file in place so all writers lock the same
    // object. Closing the handle (including process exit) releases the lock.
    let path = root.join(SAVE_LOCK);
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = match options.open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            regular_file(&path)?;
            OpenOptions::new()
                .write(true)
                .open(&path)
                .map_err(|e| io_error("Cannot open experiment save lock", e))?
        }
        Err(error) => return Err(io_error("Cannot create experiment save lock", error)),
    };
    file.lock()
        .map_err(|e| io_error("Cannot lock experiment library for saving", e))?;
    Ok(file)
}
pub(super) fn secure_directory(path: &Path) -> Result<(), String> {
    regular_directory(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .map_err(|e| io_error("Cannot restrict experiment directory access", e))?;
    }
    Ok(())
}
pub(super) struct PreparedManifest {
    file: tempfile::NamedTempFile,
    destination: PathBuf,
}
impl PreparedManifest {
    pub(super) fn publish(self, control: &Control) -> Result<(), String> {
        control.check()?;
        self.file
            .persist_noclobber(&self.destination)
            .map_err(|error| {
                io_error("Cannot publish completed experiment manifest", error.error)
            })?;
        Ok(())
    }
}
pub(super) fn publish_directory(
    staged: tempfile::TempDir,
    destination: &Path,
    control: &Control,
) -> Result<(), String> {
    control.check()?;
    super::publication::rename_exclusive(staged.path(), destination)
        .map_err(|e| io_error("Cannot publish completed experiment", e))?;
    // The atomic directory publication is the external commit point. No
    // fallible work or cancellation follows it; late cancellation is committed.
    let _ = staged.keep();
    Ok(())
}
pub(super) fn stage_manifest(
    path: &Path,
    id: &str,
    bytes: &[u8],
    control: &Control,
) -> Result<PreparedManifest, String> {
    control.check()?;
    let mut file = tempfile::Builder::new()
        .prefix(".manifest-")
        .tempfile_in(path)
        .map_err(|e| io_error("Cannot stage experiment manifest", e))?;
    file.write_all(bytes)
        .and_then(|()| file.as_file().sync_all())
        .map_err(|e| io_error("Cannot flush staged experiment manifest", e))?;
    read_manifest_at(path, id, file.path(), control)?;
    control.check()?;
    Ok(PreparedManifest {
        file,
        destination: path.join("manifest.json"),
    })
}
pub(super) fn copy_checked(
    source: &Path,
    destination: &Path,
    control: &Control,
) -> Result<(), String> {
    regular_file(source)?;
    let mut input = File::open(source).map_err(|e| io_error("Cannot open captured artifact", e))?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|e| io_error("Cannot create experiment artifact", e))?;
    let mut buffer = [0_u8; BUFFER];
    loop {
        control.check()?;
        let size = input
            .read(&mut buffer)
            .map_err(|e| io_error("Cannot read captured artifact", e))?;
        if size == 0 {
            break;
        }
        output
            .write_all(&buffer[..size])
            .map_err(|e| io_error("Cannot copy captured artifact", e))?;
    }
    output
        .sync_all()
        .map_err(|e| io_error("Cannot flush experiment artifact", e))
}
pub(super) fn stats(path: &Path, control: &Control) -> Result<ArtifactStats, String> {
    regular_file(path)?;
    crate::documents::stream::capture_stats(path, control)
}
pub(super) fn metadata_digest(manifest: &Manifest) -> Result<String, String> {
    // A deterministic JSON object representation binds all captured metadata.
    // This is corruption detection, not authentication against someone able to
    // rewrite both the library and its checksum.
    let mut value =
        serde_json::to_value(manifest).map_err(|_| "Cannot serialize experiment metadata.")?;
    value
        .as_object_mut()
        .ok_or("Invalid experiment metadata object.")?
        .remove("metadataDigest");
    let bytes =
        serde_json::to_vec(&value).map_err(|_| "Cannot serialize experiment checksum payload.")?;
    Ok(digest_hex(&Sha256::digest(&bytes)))
}

pub(super) fn read_metadata(
    path: &Path,
    id: &str,
    metadata: &Path,
    control: &Control,
) -> Result<Manifest, String> {
    control.check()?;
    regular_directory(path)?;
    regular_file(metadata)?;
    let mut bytes = Vec::new();
    File::open(metadata)
        .map_err(|e| io_error("Cannot open experiment metadata", e))?
        .take(MAX_METADATA + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| io_error("Cannot read experiment metadata", e))?;
    if bytes.len() as u64 > MAX_METADATA {
        return Err("Experiment metadata exceeds 4 MiB.".into());
    }
    let manifest: Manifest =
        serde_json::from_slice(&bytes).map_err(|e| format!("Invalid experiment metadata: {e}"))?;
    if manifest.schema_version != 2 {
        return Err("Unsupported experiment schema version.".into());
    }
    if manifest.summary.id != id {
        return Err("Experiment ID does not match its directory.".into());
    }
    if manifest.metadata_digest != metadata_digest(&manifest)? {
        return Err("Experiment metadata checksum does not match its captured contents.".into());
    }
    validate_name(&manifest.summary.name)?;
    secure_directory(path)?;
    Ok(manifest)
}
pub(super) fn read_manifest(path: &Path, id: &str, control: &Control) -> Result<Manifest, String> {
    read_manifest_at(path, id, &path.join("manifest.json"), control)
}
pub(super) fn read_manifest_at(
    path: &Path,
    id: &str,
    metadata: &Path,
    control: &Control,
) -> Result<Manifest, String> {
    let manifest = read_metadata(path, id, metadata, control)?;
    let mut expected = Vec::new();
    let (kind, recipe) = match &manifest.capture {
        Capture::Text { capture } => {
            if capture.source.len() > recipes::MAX_INPUT_BYTES
                || !matches!(capture.direction.as_str(), "encode" | "decode")
            {
                return Err("Invalid saved Text capture.".into());
            }
            ("text", &capture.recipe)
        }
        Capture::File { capture } => {
            let mut rules: Vec<_> = capture
                .recipe
                .steps
                .iter()
                .filter(|step| step.enabled)
                .map(|step| &step.rule)
                .collect();
            let exact = !rules.iter().any(|rule| rule.lossy());
            if matches!(capture.direction, Direction::Decode) {
                rules.reverse();
                rules = rules.into_iter().take_while(|rule| !rule.lossy()).collect();
            }
            if capture.stages.len() != rules.len()
                || capture.exact != exact
                || capture
                    .stages
                    .iter()
                    .zip(rules)
                    .any(|(stage, rule)| stage.lossy != rule.lossy())
            {
                return Err("Saved stages do not match the captured recipe pipeline.".into());
            }
            expected.extend([Role::Source, Role::Result]);
            for index in 0..capture.stages.len() {
                expected.push(Role::Stage { index });
            }
            if capture.reading.is_some() {
                expected.extend([Role::Reading, Role::Candidates]);
            }
            ("file", &capture.recipe)
        }
    };
    recipe.validate()?;
    if manifest.summary.kind != kind || manifest.summary.recipe_name != recipe.name {
        return Err("Experiment summary does not match its capture.".into());
    }
    if manifest.artifacts.len() != expected.len() {
        return Err("Experiment artifact roles are incomplete.".into());
    }
    for artifact in &manifest.artifacts {
        let index = expected
            .iter()
            .position(|role| role == &artifact.role)
            .ok_or("Unexpected or duplicate artifact role.")?;
        expected.swap_remove(index);
        let actual = stats(&path.join(artifact.role.filename()), control)?;
        if actual.bytes != artifact.stats.bytes
            || actual.scalars != artifact.stats.scalars
            || actual.digest != artifact.stats.digest
        {
            return Err("Experiment artifact is truncated or its digest does not match.".into());
        }
    }
    if let Capture::File { capture } = &manifest.capture {
        validate_file_capture(path, capture, &manifest.artifacts, control)?;
    }
    Ok(manifest)
}

fn validate_file_capture(
    path: &Path,
    capture: &FileCapture,
    artifacts: &[SavedArtifact],
    control: &Control,
) -> Result<(), String> {
    let find = |role: Role| {
        artifacts
            .iter()
            .find(|artifact| artifact.role == role)
            .map(|artifact| &artifact.stats)
            .ok_or("Missing saved artifact.")
    };
    let source_stats = find(Role::Source)?;
    let result_stats = find(Role::Result)?;
    let last_stats = if capture.stages.is_empty() {
        source_stats
    } else {
        find(Role::Stage {
            index: capture.stages.len() - 1,
        })?
    };
    if last_stats.bytes != result_stats.bytes
        || last_stats.digest != result_stats.digest
        || last_stats.scalars != result_stats.scalars
    {
        return Err("Saved result does not match the final pipeline stage.".into());
    }
    for (index, stage) in capture.stages.iter().enumerate() {
        if find(Role::Stage { index })?.bytes.to_string() != stage.bytes {
            return Err("Saved stage length is inconsistent.".into());
        }
    }
    let mut source =
        File::open(path.join("source")).map_err(|e| io_error("Cannot validate saved source", e))?;
    let mut hash = Sha256::new();
    if capture.document.bom {
        hash.update(b"\xef\xbb\xbf");
    }
    let mut ascii = true;
    let mut buffer = [0_u8; BUFFER];
    loop {
        control.check()?;
        let count = source
            .read(&mut buffer)
            .map_err(|e| io_error("Cannot validate saved source", e))?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
        ascii &= buffer[..count].is_ascii();
    }
    let raw_bytes = source_stats
        .bytes
        .checked_add(if capture.document.bom { 3 } else { 0 })
        .ok_or("Saved source length overflow.")?;
    let legacy =
        ascii && source_stats.bytes > 0 && source_stats.bytes <= recipes::MAX_INPUT_BYTES as u64;
    if capture.document.bytes != raw_bytes.to_string()
        || capture.document.digest != digest_hex(&hash.finalize())
        || capture.document.legacy_compatible != legacy
    {
        return Err("Saved source metadata does not match its contents.".into());
    }
    if let Some(reading) = &capture.reading {
        if capture.exact || !matches!(capture.direction, Direction::Decode) {
            return Err("Saved reading requires a lossy decode.".into());
        }
        let mut selected = HashMap::new();
        for choice in &reading.choices {
            if selected
                .insert(choice.word_id.clone(), choice.candidate.clone())
                .is_some()
            {
                return Err("Duplicate saved word choice.".into());
            }
        }
        let count = scan_candidates(&path.join("candidates"), control, |word, _| {
            if let Some(candidate) = selected.remove(&word.word_id)
                && candidate != word.suggested
            {
                return Err("Saved choice does not match its reading.".into());
            }
            Ok(())
        })?;
        if count == 0 || count.to_string() != reading.words || !selected.is_empty() {
            return Err("Saved reading word count or choices are inconsistent.".into());
        }
    }
    Ok(())
}

pub(super) fn scan_candidates(
    source: &Path,
    control: &Control,
    mut visit: impl FnMut(ReadingWord, u64) -> Result<(), String>,
) -> Result<u64, String> {
    let mut input = BufReader::new(
        File::open(source).map_err(|e| io_error("Cannot read saved candidates", e))?,
    );
    let mut count = 0_u64;
    let mut prefix = None;
    loop {
        control.check()?;
        let mut bytes = Vec::new();
        let size = (&mut input)
            .take(65537)
            .read_until(b'\n', &mut bytes)
            .map_err(|e| io_error("Cannot read candidate record", e))?;
        if size == 0 {
            break;
        }
        if size > 65536 {
            return Err("Candidate record exceeds metadata bounds.".into());
        }
        let word: ReadingWord =
            serde_json::from_slice(&bytes).map_err(|_| "Invalid saved candidate record.")?;
        let (job, index) = word
            .word_id
            .rsplit_once(':')
            .ok_or("Invalid saved word ID.")?;
        if job.is_empty()
            || index.parse::<u64>().ok() != Some(count)
            || prefix.as_deref().is_some_and(|prefix| prefix != job)
        {
            return Err("Saved candidate positions are inconsistent.".into());
        }
        if prefix.is_none() {
            prefix = Some(job.to_owned());
        }
        let valid_word = |word: &str| {
            !word.is_empty()
                && word.len() <= crate::engine::document_word_limit()
                && word.bytes().all(|byte| byte.is_ascii_lowercase())
        };
        if !valid_word(&word.suggested)
            || word.alternatives.len() > 5
            || !word.alternatives.iter().all(|word| valid_word(word))
            || word.compatible_count == 0
        {
            return Err("Invalid saved dictionary candidates.".into());
        }
        visit(word, count)?;
        count = count.checked_add(1).ok_or("Candidate count overflow.")?;
    }
    Ok(count)
}

#[cfg(test)]
mod publication_tests {
    use super::*;
    #[test]
    fn completed_manifest_is_published_only_after_verification_and_cancel_checkpoint() {
        let root = tempfile::tempdir().unwrap();
        let library = Library::new(root.path().join("experiments"));
        let recipe = Recipe {
            schema_version: 2,
            id: "user-stage".into(),
            name: "Stage".into(),
            description: String::new(),
            steps: vec![crate::recipes::Step {
                enabled: true,
                rule: crate::recipes::Rule::Caesar { shift: 3 },
            }],
        };
        let run = library
            .evaluate(recipe, "Hello".into(), "".into(), "encode".into(), vec![])
            .unwrap();
        let saved = library
            .save_text(&run.run_id, "Source", &Control::default())
            .unwrap();
        let mut manifest = read_manifest(
            &library.root.join(&saved.id),
            &saved.id,
            &Control::default(),
        )
        .unwrap();
        let id = "experiment-staging-fixture";
        let directory = root.path().join(id);
        fs::create_dir(&directory).unwrap();
        manifest.summary.id = id.into();
        manifest.metadata_digest = metadata_digest(&manifest).unwrap();
        let bytes = serde_json::to_vec(&manifest).unwrap();
        let control = Control::default();
        let prepared = stage_manifest(&directory, id, &bytes, &control).unwrap();
        assert!(!directory.join("manifest.json").exists());
        control.cancel();
        assert!(prepared.publish(&control).is_err());
        assert!(!directory.join("manifest.json").exists());
        let prepared = stage_manifest(&directory, id, &bytes, &Control::default()).unwrap();
        fs::write(directory.join("manifest.json"), "Keep existing").unwrap();
        assert!(prepared.publish(&Control::default()).is_err());
        assert_eq!(
            fs::read_to_string(directory.join("manifest.json")).unwrap(),
            "Keep existing"
        );
        fs::remove_file(directory.join("manifest.json")).unwrap();
        let control = Control::default();
        let prepared = stage_manifest(&directory, id, &bytes, &control).unwrap();
        prepared.publish(&control).unwrap();
        control.cancel();
        assert!(read_manifest(&directory, id, &Control::default()).is_ok());
    }
    #[test]
    fn independent_library_sees_only_the_atomically_committed_directory() {
        let root = tempfile::tempdir().unwrap();
        let library = Library::new(root.path().join("experiments"));
        let recipe = Recipe {
            schema_version: 2,
            id: "user-stage".into(),
            name: "Stage".into(),
            description: String::new(),
            steps: vec![crate::recipes::Step {
                enabled: true,
                rule: crate::recipes::Rule::Caesar { shift: 3 },
            }],
        };
        let run = library
            .evaluate(recipe, "Hello".into(), "".into(), "encode".into(), vec![])
            .unwrap();
        let saved = library
            .save_text(&run.run_id, "Source", &Control::default())
            .unwrap();
        let mut manifest = read_manifest(
            &library.root.join(&saved.id),
            &saved.id,
            &Control::default(),
        )
        .unwrap();
        library.delete(&saved.id).unwrap();
        let staged = crate::documents::private_tempdir(".staging-", Some(&library.root)).unwrap();
        let id = "experiment-atomic-fixture";
        manifest.summary.id = id.into();
        manifest.metadata_digest = metadata_digest(&manifest).unwrap();
        let bytes = serde_json::to_vec(&manifest).unwrap();
        stage_manifest(staged.path(), id, &bytes, &Control::default())
            .unwrap()
            .publish(&Control::default())
            .unwrap();
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        let other_barrier = barrier.clone();
        let other = Library::new(library.root.clone());
        let reader = std::thread::spawn(move || {
            other_barrier.wait();
            let before = other.list().unwrap();
            assert!(before.entries.is_empty());
            assert!(before.problems.is_empty());
            other_barrier.wait();
            other_barrier.wait();
            let after = other.list().unwrap();
            assert_eq!(after.entries.len(), 1);
            assert!(after.problems.is_empty());
        });
        barrier.wait();
        barrier.wait();
        publish_directory(staged, &library.root.join(id), &Control::default()).unwrap();
        barrier.wait();
        reader.join().unwrap();
        let cancelled =
            crate::documents::private_tempdir(".staging-", Some(&library.root)).unwrap();
        let control = Control::default();
        control.cancel();
        assert!(
            publish_directory(
                cancelled,
                &library.root.join("experiment-cancelled-fixture"),
                &control
            )
            .is_err()
        );
        assert_eq!(library.list().unwrap().entries.len(), 1);
    }
}
