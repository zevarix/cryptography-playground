//! App-owned immutable captures. Manifests contain roles, never native paths.
use super::{
    Control,
    jobs::{Artifact, Direction, DocumentStore, Job, JobInfo, JobState, StageInfo},
    reconstruct::{self, DocChoice, Reading, ReadingInfo, ReadingWord},
    storage::{self, DocumentInfo},
    stream::{ArtifactStats, BUFFER, digest_hex, io_error},
};
use crate::{
    engine::WordChoice,
    recipes::{self, Recipe, RecipeResult, fingerprint},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, VecDeque},
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_METADATA: u64 = 4 * 1024 * 1024;
const MAX_ENTRIES: usize = 200;
const MAX_CAPTURES: usize = 64;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Summary {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub kind: String,
    pub recipe_name: String,
}
#[derive(Serialize)]
pub struct Listing {
    pub entries: Vec<Summary>,
    pub problems: Vec<String>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct TextCapture {
    recipe: Recipe,
    direction: String,
    source: String,
    result: RecipeResult,
    choices: Vec<WordChoice>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Evaluated {
    pub run_id: String,
    #[serde(flatten)]
    pub result: RecipeResult,
}
#[derive(Serialize)]
#[serde(
    tag = "kind",
    rename_all = "lowercase",
    rename_all_fields = "camelCase"
)]
pub enum Opened {
    Text {
        summary: Summary,
        recipe: Recipe,
        direction: String,
        source: String,
        result: RecipeResult,
        choices: Vec<WordChoice>,
        run_id: String,
    },
    File {
        summary: Summary,
        document: DocumentInfo,
        job: Box<JobInfo>,
        reading: Option<ReadingInfo>,
        choices: Vec<DocChoice>,
    },
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SavedDocument {
    name: String,
    bytes: String,
    digest: String,
    bom: bool,
    legacy_compatible: bool,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SavedReading {
    words: String,
    note: String,
    choices: Vec<DocChoice>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct FileCapture {
    document: SavedDocument,
    recipe: Recipe,
    direction: Direction,
    exact: bool,
    stages: Vec<StageInfo>,
    note: String,
    reading: Option<SavedReading>,
}
#[derive(Clone, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(tag = "role", rename_all = "camelCase", deny_unknown_fields)]
enum Role {
    Source,
    Stage { index: usize },
    Result,
    Reading,
    Candidates,
}
impl Role {
    fn filename(&self) -> String {
        match self {
            Self::Source => "source".into(),
            Self::Stage { index } => format!("stage-{index}"),
            Self::Result => "result".into(),
            Self::Reading => "reading".into(),
            Self::Candidates => "candidates".into(),
        }
    }
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SavedArtifact {
    role: Role,
    stats: ArtifactStats,
}
#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum Capture {
    Text { capture: TextCapture },
    File { capture: FileCapture },
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Manifest {
    schema_version: u8,
    metadata_digest: String,
    summary: Summary,
    capture: Capture,
    artifacts: Vec<SavedArtifact>,
}

pub struct Library {
    root: PathBuf,
    operations: Mutex<()>,
    captures: Mutex<VecDeque<(String, TextCapture)>>,
}
impl Library {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            operations: Mutex::new(()),
            captures: Mutex::new(VecDeque::new()),
        }
    }
    fn remember(&self, capture: TextCapture) -> Result<String, String> {
        let id = fresh_id("text-run-")?;
        let mut captures = self
            .captures
            .lock()
            .map_err(|_| "Text captures are unavailable.")?;
        if captures.len() >= MAX_CAPTURES {
            captures.pop_front();
        }
        captures.push_back((id.clone(), capture));
        Ok(id)
    }
    pub fn evaluate(
        &self,
        recipe: Recipe,
        source: String,
        key: String,
        direction: String,
        choices: Vec<WordChoice>,
    ) -> Result<Evaluated, String> {
        let result = match direction.as_str() {
            "encode" => {
                if !choices.is_empty() {
                    return Err("Cannot select plaintext candidates while encoding.".into());
                }
                recipes::encode(&recipe, &source, &key)?
            }
            "decode" => recipes::decode(&recipe, &source, &key, &choices)?,
            _ => return Err("Direction must be encode or decode.".into()),
        };
        let run_id = self.remember(TextCapture {
            recipe,
            direction,
            source,
            result: result.clone(),
            choices,
        })?;
        Ok(Evaluated { run_id, result })
    }
    pub fn revise(
        &self,
        run_id: &str,
        recipe: &Recipe,
        source: &str,
        key: &str,
        choices: &[WordChoice],
    ) -> Result<Evaluated, String> {
        let mut capture = self
            .captures
            .lock()
            .map_err(|_| "Text captures are unavailable.")?
            .iter()
            .find(|(id, _)| id == run_id)
            .map(|(_, capture)| capture.clone())
            .ok_or("This captured Text run is no longer available.")?;
        if capture.direction != "decode"
            || capture.source != source
            || fingerprint::fingerprint(&capture.recipe)?.value
                != fingerprint::fingerprint(recipe)?.value
        {
            return Err(
                "The submitted configuration does not match the captured Text decode.".into(),
            );
        }
        let result = recipes::decode(&capture.recipe, &capture.source, key, choices)?;
        let selected = result
            .reconstruction
            .as_ref()
            .and_then(|reading| reading.suggested.as_deref())
            .ok_or("No compatible English reconstruction was found.")?;
        let forward = recipes::encode(&capture.recipe, selected, key)?;
        if forward.output != capture.source {
            return Err("The selected reading does not re-encode to the complete captured ciphertext. The previous result is preserved.".into());
        }
        capture.result = result.clone();
        capture.choices = choices.to_vec();
        Ok(Evaluated {
            run_id: self.remember(capture)?,
            result,
        })
    }
    pub fn save_text(
        &self,
        run_id: &str,
        name: &str,
        control: &Control,
    ) -> Result<Summary, String> {
        let capture = self
            .captures
            .lock()
            .map_err(|_| "Text captures are unavailable.")?
            .iter()
            .find(|(id, _)| id == run_id)
            .map(|(_, capture)| capture.clone())
            .ok_or("This Text run is no longer available. Run it again before saving.")?;
        self.publish(name, Capture::Text { capture }, Vec::new(), control)
    }
    pub fn save_file(
        &self,
        store: &DocumentStore,
        job_id: &str,
        reading_id: Option<&str>,
        name: &str,
        control: &Control,
    ) -> Result<Summary, String> {
        let job = store.job(job_id)?;
        let result = job.result()?;
        // Retain the exact chosen reading before copying; a concurrent new reading
        // cannot silently substitute another completed reading.
        let reading = reading_id
            .map(|id| reconstruct::current(store, job_id, id))
            .transpose()?;
        let state = job
            .state
            .lock()
            .map_err(|_| "Document job state is unavailable.")?;
        let mut files = vec![
            (
                Role::Source,
                job.source.payload.clone(),
                job.source.stats.clone(),
            ),
            (Role::Result, result.path, result.stats),
        ];
        for (index, artifact) in state.artifacts.iter().enumerate() {
            files.push((
                Role::Stage { index },
                artifact.path.clone(),
                artifact.stats.clone(),
            ));
        }
        let saved_reading = if let Some(reading) = reading.as_ref() {
            files.push((
                Role::Reading,
                reading.artifact.path.clone(),
                reading.artifact.stats.clone(),
            ));
            files.push((
                Role::Candidates,
                reading.candidates.clone(),
                reading.candidates_stats.clone(),
            ));
            Some(SavedReading {
                words: reading.info.words.clone(),
                note: reading.info.note.clone(),
                choices: reading.choices.clone(),
            })
        } else {
            None
        };
        let doc = &job.source.info;
        let capture = FileCapture {
            document: SavedDocument {
                name: doc.name.clone(),
                bytes: doc.bytes.clone(),
                digest: doc.digest.clone(),
                bom: doc.bom,
                legacy_compatible: doc.legacy_compatible,
            },
            recipe: state.info.recipe.clone(),
            direction: state.info.direction,
            exact: state.info.exact,
            stages: state.info.stages.clone(),
            note: state.info.note.clone(),
            reading: saved_reading,
        };
        drop(state);
        self.publish(name, Capture::File { capture }, files, control)
    }
    fn publish(
        &self,
        name: &str,
        capture: Capture,
        files: Vec<(Role, PathBuf, ArtifactStats)>,
        control: &Control,
    ) -> Result<Summary, String> {
        let _operation = self
            .operations
            .lock()
            .map_err(|_| "Experiment storage is unavailable.")?;
        control.check()?;
        validate_name(name)?;
        ensure_root(&self.root)?;
        let mut count = 0_usize;
        for entry in fs::read_dir(&self.root).map_err(|e| io_error("Cannot list experiments", e))? {
            let entry = entry.map_err(|e| io_error("Cannot inspect experiment entry", e))?;
            let id = entry.file_name();
            if let Some(id) = id.to_str()
                && let Ok(path) = entry_path(&self.root, id)
                && read_manifest(&path, id, control).is_ok()
            {
                count += 1;
            }
            control.check()?;
            if count >= MAX_ENTRIES {
                break;
            }
        }
        if count >= MAX_ENTRIES {
            return Err(
                "The experiment library is full (200 entries). Delete an entry before saving."
                    .into(),
            );
        }
        let staged = super::private_tempdir(".staging-", Some(&self.root))
            .map_err(|e| io_error("Cannot stage experiment", e))?;
        let id = fresh_id("experiment-")?;
        let (kind, recipe_name) = match &capture {
            Capture::Text { capture } => ("text", capture.recipe.name.clone()),
            Capture::File { capture } => ("file", capture.recipe.name.clone()),
        };
        let summary = Summary {
            id: id.clone(),
            name: name.trim().into(),
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| "System clock is before the Unix epoch.")?
                .as_secs()
                .to_string(),
            kind: kind.into(),
            recipe_name,
        };
        let mut artifacts = Vec::new();
        for (role, source, expected) in files {
            let destination = staged.path().join(role.filename());
            copy_checked(&source, &destination, control)?;
            let actual = stats(&destination, control)?;
            if actual.bytes != expected.bytes
                || actual.scalars != expected.scalars
                || actual.digest != expected.digest
            {
                return Err(
                    "A captured artifact changed before saving. Run the experiment again.".into(),
                );
            }
            artifacts.push(SavedArtifact {
                role,
                stats: actual,
            });
        }
        let mut manifest = Manifest {
            schema_version: 2,
            metadata_digest: String::new(),
            summary: summary.clone(),
            capture,
            artifacts,
        };
        manifest.metadata_digest = metadata_digest(&manifest)?;
        let data =
            serde_json::to_vec(&manifest).map_err(|_| "Cannot serialize experiment metadata.")?;
        if u64::try_from(data.len()).map_err(|_| "Metadata length overflow.")? > MAX_METADATA {
            return Err("Experiment metadata exceeds 4 MiB.".into());
        }
        let metadata = stage_manifest(staged.path(), &id, &data, control)?;
        metadata.publish(control)?;
        let destination = entry_path(&self.root, &id)?;
        publish_directory(staged, &destination, control)?;
        Ok(summary)
    }

    pub fn list(&self) -> Result<Listing, String> {
        let _operation = self
            .operations
            .lock()
            .map_err(|_| "Experiment storage is unavailable.")?;
        let mut listing = Listing {
            entries: Vec::new(),
            problems: Vec::new(),
        };
        if !self
            .root
            .try_exists()
            .map_err(|e| io_error("Cannot inspect experiment library", e))?
        {
            return Ok(listing);
        }
        secure_directory(&self.root)?;
        for entry in fs::read_dir(&self.root).map_err(|e| io_error("Cannot list experiments", e))? {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    listing
                        .problems
                        .push(io_error("Cannot inspect experiment entry", error));
                    continue;
                }
            };
            let id = entry.file_name().to_string_lossy().into_owned();
            if id.starts_with(".staging-") {
                continue;
            }
            match entry_path(&self.root, &id)
                .and_then(|path| read_manifest(&path, &id, &Control::default()))
            {
                Ok(manifest) if listing.entries.len() < MAX_ENTRIES => {
                    listing.entries.push(manifest.summary)
                }
                Ok(_) => {
                    listing
                        .problems
                        .push("Only the first 200 valid experiments are listed.".into());
                    break;
                }
                Err(error) => listing.problems.push(format!("{id}: {error}")),
            }
        }
        listing.entries.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(listing)
    }
    pub fn delete(&self, id: &str) -> Result<(), String> {
        let _operation = self
            .operations
            .lock()
            .map_err(|_| "Experiment storage is unavailable.")?;
        secure_directory(&self.root)?;
        let path = entry_path(&self.root, id)?;
        regular_directory(&path)?;
        let identity = same_file::Handle::from_path(&path)
            .map_err(|e| io_error("Cannot retain experiment directory identity", e))?;
        read_metadata(&path, id, &path.join("manifest.json"), &Control::default())?;
        regular_directory(&path)?;
        let current = same_file::Handle::from_path(&path)
            .map_err(|e| io_error("Cannot recheck experiment directory identity", e))?;
        if identity != current {
            return Err(
                "The experiment directory changed before deletion. Nothing was removed.".into(),
            );
        }
        fs::remove_dir_all(path).map_err(|e| io_error("Cannot delete experiment", e))
    }
    pub fn open(
        &self,
        id: &str,
        store: &DocumentStore,
        control: &Control,
    ) -> Result<Opened, String> {
        let _operation = self
            .operations
            .lock()
            .map_err(|_| "Experiment storage is unavailable.")?;
        secure_directory(&self.root)?;
        let path = entry_path(&self.root, id)?;
        let manifest = read_manifest(&path, id, control)?;
        match manifest.capture {
            Capture::Text { capture } => {
                control.check()?;
                let run_id = self.remember(capture.clone())?;
                Ok(Opened::Text {
                    summary: manifest.summary,
                    recipe: capture.recipe,
                    direction: capture.direction,
                    source: capture.source,
                    result: capture.result,
                    choices: capture.choices,
                    run_id,
                })
            }
            Capture::File { capture } => restore_file(
                manifest.summary,
                capture,
                &path,
                &manifest.artifacts,
                store,
                control,
            ),
        }
    }
}

fn fresh_id(prefix: &str) -> Result<String, String> {
    let directory = super::private_tempdir(prefix, None)
        .map_err(|e| io_error("Cannot create fresh identifier", e))?;
    directory
        .path()
        .file_name()
        .and_then(|s| s.to_str())
        .map(str::to_owned)
        .ok_or_else(|| "Cannot identify temporary capture.".into())
}

pub(crate) mod commands;
mod persistence;
mod publication;
mod restore;
use persistence::*;
use restore::restore_file;
