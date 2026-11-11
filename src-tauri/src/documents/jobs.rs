//! Immutable job snapshots, serialized admission, and terminal-state publication.
use super::{
    Control,
    storage::{self, DocumentInfo, Preview, Snapshot},
    stream::{ArtifactStats, io_error},
    transform,
};
use crate::recipes::{
    self, Recipe,
    fingerprint::{self, RecipeFingerprint},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, atomic::Ordering},
};
use tempfile::TempDir;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Encode,
    Decode,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StageInfo {
    pub label: String,
    pub bytes: String,
    pub lossy: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobInfo {
    pub id: String,
    pub document_id: String,
    pub state: String,
    pub phase: String,
    pub processed_bytes: String,
    pub total_bytes: String,
    pub recipe: Recipe,
    pub fingerprint: RecipeFingerprint,
    pub direction: Direction,
    pub exact: bool,
    pub restored: bool,
    pub stages: Vec<StageInfo>,
    pub note: String,
}

#[derive(Clone)]
pub(super) struct Artifact {
    pub path: PathBuf,
    pub stats: ArtifactStats,
    pub label: String,
    pub lossy: bool,
}

pub(super) struct JobState {
    pub info: JobInfo,
    pub artifacts: Vec<Artifact>,
    pub result: Option<Artifact>,
    pub reading: Option<Arc<super::reconstruct::Reading>>,
    pub reconstructing: bool,
}
pub(super) struct Job {
    pub root: TempDir,
    pub source: Arc<Snapshot>,
    pub control: Control,
    pub state: Mutex<JobState>,
    pub key: String,
}
impl Job {
    pub fn result(&self) -> Result<Artifact, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "Document job state is unavailable.")?;
        if state.info.state != "complete" {
            return Err("This job has no completed result.".into());
        }
        state
            .result
            .clone()
            .ok_or_else(|| "Missing completed artifact.".into())
    }
}

#[derive(Default)]
struct Registry {
    documents: HashMap<String, Arc<Snapshot>>,
    jobs: HashMap<String, Arc<Job>>,
}
struct Inner {
    session: TempDir,
    registry: Mutex<Registry>,
}
#[derive(Clone)]
pub struct DocumentStore {
    inner: Arc<Inner>,
}

impl DocumentStore {
    pub fn new() -> Result<Self, String> {
        let session = super::private_tempdir("cryptoplay-documents-", None)
            .map_err(|e| io_error("Cannot create private document session", e))?;
        Ok(Self {
            inner: Arc::new(Inner {
                session,
                registry: Mutex::new(Registry::default()),
            }),
        })
    }
    pub(super) fn adopt_restored(&self, job: Arc<Job>) -> Result<(), String> {
        let mut registry = self
            .inner
            .registry
            .lock()
            .map_err(|_| "Document registry is unavailable.")?;
        let id = job
            .state
            .lock()
            .map_err(|_| "Document job state is unavailable.")?
            .info
            .id
            .clone();
        registry
            .documents
            .insert(job.source.info.id.clone(), job.source.clone());
        registry.jobs.insert(id, job);
        Ok(())
    }
    pub fn capture(&self, source: &Path, control: &Control) -> Result<DocumentInfo, String> {
        let snapshot = storage::capture(source, self.inner.session.path(), control)?;
        let info = snapshot.info.clone();
        self.inner
            .registry
            .lock()
            .map_err(|_| "Document registry is unavailable.")?
            .documents
            .insert(info.id.clone(), Arc::new(snapshot));
        Ok(info)
    }
    fn document(&self, id: &str) -> Result<Arc<Snapshot>, String> {
        self.inner
            .registry
            .lock()
            .map_err(|_| "Document registry is unavailable.")?
            .documents
            .get(id)
            .cloned()
            .ok_or_else(|| "Unknown or discarded document.".into())
    }
    pub(super) fn job(&self, id: &str) -> Result<Arc<Job>, String> {
        self.inner
            .registry
            .lock()
            .map_err(|_| "Document registry is unavailable.")?
            .jobs
            .get(id)
            .cloned()
            .ok_or_else(|| "Unknown or discarded job.".into())
    }
    pub fn preview_document(&self, id: &str, offset: u64) -> Result<Preview, String> {
        storage::preview(&self.document(id)?.payload, offset)
    }
    pub fn preview_job(
        &self,
        id: &str,
        stage: Option<usize>,
        offset: u64,
    ) -> Result<Preview, String> {
        let job = self.job(id)?;
        let artifact = if let Some(index) = stage {
            job.state
                .lock()
                .map_err(|_| "Document job state is unavailable.")?
                .artifacts
                .get(index)
                .cloned()
                .ok_or("Unknown document stage.")?
        } else {
            job.result()?
        };
        storage::preview(&artifact.path, offset)
    }
    pub fn status(&self, id: &str) -> Result<JobInfo, String> {
        let job = self.job(id)?;
        let mut info = job
            .state
            .lock()
            .map_err(|_| "Document job state is unavailable.")?
            .info
            .clone();
        if info.state == "running"
            && let Some((phase, total)) = job.control.progress()
        {
            info.phase = format!("{} · {}", info.phase, phase);
            info.total_bytes = total.map_or_else(String::new, |value| value.to_string());
        }
        info.processed_bytes = job.control.bytes.load(Ordering::Relaxed).to_string();
        Ok(info)
    }
    pub fn cancel(&self, id: &str) -> Result<(), String> {
        let job = self.job(id)?;
        let state = job
            .state
            .lock()
            .map_err(|_| "Document job state is unavailable.")?;
        if state.info.state == "running" {
            job.control.cancel();
        }
        Ok(())
    }
    pub fn discard_document(&self, id: &str) -> Result<(), String> {
        self.inner
            .registry
            .lock()
            .map_err(|_| "Document registry is unavailable.")?
            .documents
            .remove(id);
        Ok(())
    }
    pub fn discard_job(&self, id: &str) -> Result<(), String> {
        if let Some(job) = self
            .inner
            .registry
            .lock()
            .map_err(|_| "Document registry is unavailable.")?
            .jobs
            .remove(id)
        {
            job.control.cancel();
        }
        Ok(())
    }
    pub fn start(
        &self,
        document_id: &str,
        recipe: Recipe,
        key: String,
        direction: Direction,
    ) -> Result<JobInfo, String> {
        recipe.validate()?;
        let source = self.document(document_id)?;
        let key = recipes::normalize_key(&recipe, &key)?;
        if recipe.schema_version == 1 {
            if source.stats.bytes
                > u64::try_from(recipes::MAX_INPUT_BYTES).map_err(|_| "Legacy size overflow.")?
            {
                return Err("Legacy recipes require nonempty ASCII up to 16 KiB. Create a document-compatible copy to use this file.".into());
            }
            let text = fs::read_to_string(&source.payload)
                .map_err(|e| io_error("Cannot validate legacy snapshot", e))?;
            recipes::validate_inputs(&recipe, &text, &key)?;
        }
        let fingerprint = fingerprint::fingerprint(&recipe)?;
        let root = super::private_tempdir("job-", Some(self.inner.session.path()))
            .map_err(|e| io_error("Cannot create document job", e))?;
        let id = root
            .path()
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("Cannot identify document job.")?
            .to_owned();
        let exact = !recipe.steps.iter().any(|s| s.enabled && s.rule.lossy());
        let info = JobInfo {
            id: id.clone(),
            document_id: document_id.into(),
            state: "running".into(),
            phase: "Starting".into(),
            processed_bytes: "0".into(),
            total_bytes: source.stats.bytes.to_string(),
            recipe,
            fingerprint,
            direction,
            exact,
            restored: false,
            stages: Vec::new(),
            note: String::new(),
        };
        let job = Arc::new(Job {
            key: key.clone(),
            root,
            source,
            control: Control::default(),
            state: Mutex::new(JobState {
                info: info.clone(),
                artifacts: Vec::new(),
                result: None,
                reading: None,
                reconstructing: false,
            }),
        });
        {
            let mut registry = self
                .inner
                .registry
                .lock()
                .map_err(|_| "Document registry is unavailable.")?;
            for existing in registry.jobs.values() {
                let state = existing
                    .state
                    .lock()
                    .map_err(|_| "Document job state is unavailable.")?;
                if state.info.document_id == document_id && state.info.state == "running" {
                    return Err("A job is already running for this document.".into());
                }
            }
            registry.jobs.insert(id, job.clone());
        }
        let worker = job.clone();
        if let Err(error) = std::thread::Builder::new()
            .name("document-transform".into())
            .spawn(move || execute(worker, key))
        {
            let mut state = job
                .state
                .lock()
                .map_err(|_| "Document job state is unavailable.")?;
            state.info.state = "failed".into();
            state.info.note = io_error("Cannot start document worker", error);
            return Err(state.info.note.clone());
        }
        Ok(info)
    }
}

fn execute(job: Arc<Job>, key: String) {
    let result = execute_pipeline(&job, &key);
    // Cancellation and terminal publication share this lock with cancel().
    if let Ok(mut state) = job.state.lock() {
        match result.and_then(|artifact| {
            job.control.check()?;
            Ok(artifact)
        }) {
            Ok(artifact) => {
                state.result = Some(artifact);
                state.info.state = "complete".into();
                state.info.phase = "Complete".into();
                state.info.note = if state.info.exact {
                    "Reversible document pipeline completed.".into()
                } else {
                    "Lossy pipeline: exact surviving text is retained. English reconstruction is a separate speculative operation.".into()
                };
            }
            Err(error) => {
                state.info.state = if job.control.check().is_err() {
                    "cancelled"
                } else {
                    "failed"
                }
                .into();
                state.info.phase = state.info.state.clone();
                state.info.note = error;
            }
        }
    }
}

fn execute_pipeline(job: &Job, key: &str) -> Result<Artifact, String> {
    let info = job
        .state
        .lock()
        .map_err(|_| "Document job state is unavailable.")?
        .info
        .clone();
    let mut current = Artifact {
        path: job.source.payload.clone(),
        stats: job.source.stats.clone(),
        label: "Source".into(),
        lossy: false,
    };
    let mut steps: Vec<_> = info
        .recipe
        .steps
        .iter()
        .enumerate()
        .filter(|(_, s)| s.enabled)
        .collect();
    let undo = matches!(info.direction, Direction::Decode);
    if undo {
        steps.reverse();
    }
    for (ordinal, (index, step)) in steps.into_iter().enumerate() {
        if undo && step.rule.lossy() {
            break;
        }
        job.control.check()?;
        {
            let mut state = job
                .state
                .lock()
                .map_err(|_| "Document job state is unavailable.")?;
            state.info.phase = format!(
                "{} {}",
                if undo { "Undo" } else { "Apply" },
                step.rule.name()
            );
            state.info.total_bytes = current.stats.bytes.to_string();
            job.control.bytes.store(0, Ordering::Relaxed);
        }
        let resolved = fingerprint::resolve(&info.recipe, index, &step.rule, key)?;
        let actual = resolved.as_ref().map_or(&step.rule, |(rule, _)| rule);
        let path = job.root.path().join(format!("stage-{ordinal}"));
        let stats = transform::transform_known(
            actual,
            &current.path,
            &path,
            key,
            undo,
            &job.control,
            Some(current.stats.scalars),
        )?;
        let label = resolved.map_or_else(|| step.rule.name().to_owned(), |(_, label)| label);
        current = Artifact {
            path,
            stats,
            label: if undo { format!("Undo {label}") } else { label },
            lossy: step.rule.lossy(),
        };
        let mut state = job
            .state
            .lock()
            .map_err(|_| "Document job state is unavailable.")?;
        state.info.stages.push(StageInfo {
            label: current.label.clone(),
            bytes: current.stats.bytes.to_string(),
            lossy: current.lossy,
        });
        state.artifacts.push(current.clone());
    }
    Ok(current)
}
