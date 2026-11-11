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
pub(super) fn ensure_root(root: &Path) -> Result<(), String> {
    if !root
        .try_exists()
        .map_err(|e| io_error("Cannot inspect experiment storage", e))?
    {
        fs::create_dir_all(root).map_err(|e| io_error("Cannot create experiment storage", e))?;
    }
    regular_directory(root)
}
pub(super) fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| io_error("Cannot create experiment metadata", e))?;
    output
        .write_all(bytes)
        .and_then(|()| output.sync_all())
        .map_err(|e| io_error("Cannot flush experiment metadata", e))
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
    let mut reader = ScalarReader::open(path)?;
    let mut hash = Sha256::new();
    let mut scalars = 0_u64;
    let mut buffer = [0_u8; 4];
    while let Some(scalar) = reader.next()? {
        hash.update(scalar.encode_utf8(&mut buffer).as_bytes());
        scalars = scalars.checked_add(1).ok_or("Artifact length overflow.")?;
        if scalars.is_multiple_of(4096) {
            control.check()?;
        }
    }
    control.check()?;
    Ok(ArtifactStats {
        bytes: reader.bytes,
        scalars,
        digest: digest_hex(&hash.finalize()),
    })
}
pub(super) fn read_manifest(path: &Path, id: &str, control: &Control) -> Result<Manifest, String> {
    control.check()?;
    regular_directory(path)?;
    let metadata = path.join("manifest.json");
    regular_file(&metadata)?;
    let mut bytes = Vec::new();
    File::open(&metadata)
        .map_err(|e| io_error("Cannot open experiment metadata", e))?
        .take(MAX_METADATA + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| io_error("Cannot read experiment metadata", e))?;
    if bytes.len() as u64 > MAX_METADATA {
        return Err("Experiment metadata exceeds 4 MiB.".into());
    }
    let manifest: Manifest =
        serde_json::from_slice(&bytes).map_err(|e| format!("Invalid experiment metadata: {e}"))?;
    if manifest.schema_version != 1 {
        return Err("Unsupported experiment schema version.".into());
    }
    if manifest.summary.id != id {
        return Err("Experiment ID does not match its directory.".into());
    }
    validate_name(&manifest.summary.name)?;
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
