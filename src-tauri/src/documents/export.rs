//! Verified, non-overwriting Save As. Sibling files are not a transaction.
use super::{
    Control,
    jobs::{Artifact, DocumentStore},
    stream::{BUFFER, add, digest_hex, io_error},
};
use same_file::Handle;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ImportedCompanion {
    Recipe { recipe: crate::recipes::Recipe },
    Fingerprint { identifier: String },
}

/// Recipe metadata retains its existing 64 KiB storage bound. Enforce the
/// actual read bound before parsing; document payloads use a separate stream.
pub fn read_companion(path: &Path) -> Result<ImportedCompanion, String> {
    let file = File::open(path).map_err(|e| io_error("Cannot open recipe companion", e))?;
    if !file
        .metadata()
        .map_err(|e| io_error("Cannot inspect recipe companion", e))?
        .is_file()
    {
        return Err("Choose a regular companion file.".into());
    }
    let mut bytes = Vec::new();
    file.take(65537)
        .read_to_end(&mut bytes)
        .map_err(|e| io_error("Cannot read recipe companion", e))?;
    if bytes.len() > 65536 {
        return Err("Recipe companion exceeds the 64 KiB metadata limit.".into());
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| "Recipe companions must be UTF-8.")?
        .trim_start_matches('\u{feff}')
        .trim();
    if text.starts_with('{') {
        let recipe: crate::recipes::Recipe = serde_json::from_str(text).map_err(
            |_| "Invalid recipe JSON. Check the supported schema, fields and rule parameters.",
        )?;
        recipe.validate()?;
        Ok(ImportedCompanion::Recipe { recipe })
    } else {
        crate::recipes::lookup::lookup(text, &[])?;
        Ok(ImportedCompanion::Fingerprint {
            identifier: text.into(),
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Companion {
    Json,
    Fingerprint,
    None,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishedArtifact {
    pub name: String,
    pub bytes: String,
    pub digest: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct SaveReceipt {
    pub artifacts: Vec<PublishedArtifact>,
}
#[derive(Clone, Debug, Serialize)]
pub struct SaveFailure {
    pub message: String,
    pub published: Vec<PublishedArtifact>,
    pub unverified: Vec<String>,
}

pub(super) trait Publisher {
    fn publish(&mut self, stage: NamedTempFile, destination: &Path) -> Result<File, String>;
}
pub(super) struct NativePublisher;
impl Publisher for NativePublisher {
    fn publish(&mut self, stage: NamedTempFile, destination: &Path) -> Result<File, String> {
        stage.persist_noclobber(destination).map_err(|e| {
            io_error(
                "Cannot publish new file; existing files are retained",
                e.error,
            )
        })
    }
}

struct Staged {
    file: NamedTempFile,
    destination: PathBuf,
    bytes: u64,
    digest: String,
}

fn hash(file: &mut File, control: &Control) -> Result<(u64, String), String> {
    file.seek(SeekFrom::Start(0))
        .map_err(|e| io_error("Cannot rewind export for verification", e))?;
    let mut hash = Sha256::new();
    let mut bytes = 0;
    let mut buffer = [0; BUFFER];
    loop {
        control.check()?;
        let read = file
            .read(&mut buffer)
            .map_err(|e| io_error("Cannot verify exported bytes", e))?;
        if read == 0 {
            break;
        }
        bytes = add(
            bytes,
            u64::try_from(read).map_err(|_| "Export size overflow.")?,
        )?;
        hash.update(&buffer[..read]);
        control.observe(bytes)?;
    }
    Ok((bytes, digest_hex(&hash.finalize())))
}

fn finish_stage(
    mut file: NamedTempFile,
    destination: PathBuf,
    expected_bytes: u64,
    expected_digest: String,
    control: &Control,
) -> Result<Staged, String> {
    file.as_file()
        .sync_all()
        .map_err(|e| io_error("Cannot flush staged export", e))?;
    control.begin("Verifying staged bytes", Some(expected_bytes))?;
    let (bytes, digest) = hash(file.as_file_mut(), control)?;
    if bytes != expected_bytes || digest != expected_digest {
        return Err("Staged export failed byte verification.".into());
    }
    Ok(Staged {
        file,
        destination,
        bytes,
        digest,
    })
}

fn stage_result(
    artifact: &Artifact,
    bom: bool,
    root: &Path,
    destination: &Path,
    control: &Control,
) -> Result<Staged, String> {
    control.begin("Staging result", Some(artifact.stats.bytes))?;
    let mut source =
        File::open(&artifact.path).map_err(|e| io_error("Cannot read completed result", e))?;
    let mut stage = NamedTempFile::new_in(root).map_err(|e| io_error("Cannot stage result", e))?;
    let mut full_hash = Sha256::new();
    let mut payload_hash = Sha256::new();
    let mut bytes = 0;
    if bom {
        stage
            .write_all(b"\xef\xbb\xbf")
            .map_err(|e| io_error("Cannot write UTF-8 header", e))?;
        full_hash.update(b"\xef\xbb\xbf");
    }
    let mut buffer = [0; BUFFER];
    loop {
        control.observe(bytes)?;
        let read = source
            .read(&mut buffer)
            .map_err(|e| io_error("Cannot copy completed result", e))?;
        if read == 0 {
            break;
        }
        bytes = add(
            bytes,
            u64::try_from(read).map_err(|_| "Result size overflow.")?,
        )?;
        if bytes > artifact.stats.bytes {
            return Err("Completed result changed before export.".into());
        }
        stage
            .write_all(&buffer[..read])
            .map_err(|e| io_error("Cannot stage result bytes", e))?;
        payload_hash.update(&buffer[..read]);
        full_hash.update(&buffer[..read]);
    }
    if bytes != artifact.stats.bytes
        || digest_hex(&payload_hash.finalize()) != artifact.stats.digest
    {
        return Err("Completed result changed before export.".into());
    }
    finish_stage(
        stage,
        destination.to_owned(),
        add(bytes, if bom { 3 } else { 0 })?,
        digest_hex(&full_hash.finalize()),
        control,
    )
}

fn stage_bytes(
    bytes: &[u8],
    root: &Path,
    destination: PathBuf,
    control: &Control,
) -> Result<Staged, String> {
    let mut stage =
        NamedTempFile::new_in(root).map_err(|e| io_error("Cannot stage companion", e))?;
    stage
        .write_all(bytes)
        .map_err(|e| io_error("Cannot write companion", e))?;
    finish_stage(
        stage,
        destination,
        u64::try_from(bytes.len()).map_err(|_| "Companion length overflow.")?,
        digest_hex(&Sha256::digest(bytes)),
        control,
    )
}

pub fn companion_path(destination: &Path, companion: Companion) -> Option<PathBuf> {
    match companion {
        Companion::Json => Some(destination.with_extension("recipe.json")),
        Companion::Fingerprint => Some(destination.with_extension("fingerprint.txt")),
        Companion::None => None,
    }
}

fn absent(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(_) => Err(
            "A selected output or companion already exists. Choose another Save As name.".into(),
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error("Cannot inspect export destination", error)),
    }
}

fn verify_published(
    destination: &Path,
    identity: &Handle,
    bytes: u64,
    digest: &str,
    control: &Control,
) -> Result<PublishedArtifact, String> {
    let mut file =
        File::open(destination).map_err(|e| io_error("Cannot reopen published export", e))?;
    let opened = Handle::from_file(
        file.try_clone()
            .map_err(|e| io_error("Cannot retain published file", e))?,
    )
    .map_err(|e| io_error("Cannot inspect published identity", e))?;
    if opened != *identity {
        return Err("Published output identity changed; existing files are retained.".into());
    }
    // This read proves bytes under the published name, after external visibility.
    let actual = hash(&mut file, control)?;
    let current =
        Handle::from_path(destination).map_err(|e| io_error("Cannot recheck published name", e))?;
    if actual != (bytes, digest.to_owned()) || current != *identity {
        return Err("Published output failed identity/byte verification.".into());
    }
    Ok(PublishedArtifact {
        name: destination
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        bytes: bytes.to_string(),
        digest: digest.to_owned(),
    })
}

#[cfg(test)]
pub fn save(
    store: &DocumentStore,
    job_id: &str,
    destination: &Path,
    companion: Companion,
    control: &Control,
) -> Result<SaveReceipt, SaveFailure> {
    save_with(
        store,
        job_id,
        destination,
        companion,
        control,
        &mut NativePublisher,
    )
}

pub fn save_reading(
    store: &DocumentStore,
    job_id: &str,
    reading_id: Option<&str>,
    destination: &Path,
    companion: Companion,
    control: &Control,
) -> Result<SaveReceipt, SaveFailure> {
    save_selection_with(
        store,
        job_id,
        reading_id,
        destination,
        companion,
        control,
        &mut NativePublisher,
    )
}

#[cfg(test)]
pub(super) fn save_with(
    store: &DocumentStore,
    job_id: &str,
    destination: &Path,
    companion: Companion,
    control: &Control,
    publisher: &mut impl Publisher,
) -> Result<SaveReceipt, SaveFailure> {
    save_selection_with(
        store,
        job_id,
        None,
        destination,
        companion,
        control,
        publisher,
    )
}

fn save_selection_with(
    store: &DocumentStore,
    job_id: &str,
    reading_id: Option<&str>,
    destination: &Path,
    companion: Companion,
    control: &Control,
    publisher: &mut impl Publisher,
) -> Result<SaveReceipt, SaveFailure> {
    let mut published = Vec::new();
    let mut unverified = Vec::new();
    let outcome = (|| -> Result<(), String> {
        control.check()?;
        let job = store.job(job_id)?;
        let reading = reading_id
            .map(|id| super::reconstruct::current(store, job_id, id))
            .transpose()?;
        // Retaining this Arc keeps a selected reading alive if a later
        // candidate selection replaces the job's current reading during Save.
        let artifact = reading
            .as_ref()
            .map(|r| r.artifact.clone())
            .map_or_else(|| job.result(), Ok)?;
        let info = store.status(job_id)?;
        let parent = destination
            .parent()
            .ok_or("Choose a destination directory.")?
            .canonicalize()
            .map_err(|e| io_error("Cannot resolve destination directory", e))?;
        let name = destination.file_name().ok_or("Choose a result filename.")?;
        let destination = parent.join(name);
        if destination == job.source.source || destination == job.source.canonical_source {
            return Err("Save As cannot replace the source document.".into());
        }
        let parent_identity = Handle::from_path(&parent)
            .map_err(|e| io_error("Cannot retain destination directory identity", e))?;
        absent(&destination)?;
        let companion_destination = companion_path(&destination, companion);
        if let Some(path) = &companion_destination {
            if *path == destination
                || *path == job.source.source
                || *path == job.source.canonical_source
            {
                return Err(
                    "Companion and document destinations must be distinct new files.".into(),
                );
            }
            absent(path)?;
        }
        let staging = tempfile::Builder::new()
            .prefix(".cryptoplay-export-")
            .tempdir_in(&parent)
            .map_err(|e| io_error("Cannot create private export staging directory", e))?;
        let result = stage_result(
            &artifact,
            job.source.info.bom,
            staging.path(),
            &destination,
            control,
        )?;
        let mut stages = Vec::new();
        if let Some(path) = companion_destination {
            let bytes = match companion {
                Companion::Json => {
                    let mut bytes = serde_json::to_vec_pretty(&info.recipe)
                        .map_err(|_| "Cannot serialize recipe companion.")?;
                    bytes.push(b'\n');
                    bytes
                }
                Companion::Fingerprint => format!(
                    "{}\nSHA-256: {}\n",
                    info.fingerprint.display, info.fingerprint.value
                )
                .into_bytes(),
                Companion::None => return Err("Unexpected companion selection.".into()),
            };
            stages.push(stage_bytes(&bytes, staging.path(), path, control)?);
        }
        stages.push(result);
        for staged in stages {
            control.check()?;
            if Handle::from_path(&parent)
                .map_err(|e| io_error("Cannot recheck destination directory", e))?
                != parent_identity
            {
                return Err("Destination directory changed before publication.".into());
            }
            let identity = Handle::from_file(
                staged
                    .file
                    .as_file()
                    .try_clone()
                    .map_err(|e| io_error("Cannot retain staged file identity", e))?,
            )
            .map_err(|e| io_error("Cannot inspect staged file identity", e))?;
            control.begin("Publishing new file", None)?;
            let publication = publisher.publish(staged.file, &staged.destination);
            // Reconcile even when an acknowledgment failed: no error means
            // neither 'absent' nor permission to overwrite a possibly published file.
            // Even a cancelled publication must be reconciled below; begin must
            // not short-circuit after a file may already exist.
            let _ = control.begin("Verifying saved file", Some(staged.bytes));
            let receipt = verify_published(
                &staged.destination,
                &identity,
                staged.bytes,
                &staged.digest,
                control,
            );
            match (publication, receipt) {
                (_, Ok(receipt)) => published.push(receipt),
                (Err(error), Err(_)) => {
                    if fs::symlink_metadata(&staged.destination).is_ok() {
                        unverified.push(
                            staged
                                .destination
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned(),
                        );
                    }
                    return Err(error);
                }
                (Ok(_), Err(error)) => {
                    unverified.push(
                        staged
                            .destination
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                    );
                    return Err(error);
                }
            }
        }
        // Cancellation after verified publication cannot undo saved files.
        let _ = control.begin("Finalizing saved files", None);
        #[cfg(unix)]
        File::open(&parent)
            .and_then(|file| file.sync_all())
            .map_err(|e| io_error("Cannot flush published directory", e))?;
        Ok(())
    })();
    match outcome {
        Ok(()) => Ok(SaveReceipt {
            artifacts: published,
        }),
        Err(message) => Err(SaveFailure {
            message,
            published,
            unverified,
        }),
    }
}
