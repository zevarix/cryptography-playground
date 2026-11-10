//! Background document I/O and native drop-token ownership.
use super::{
    Control,
    export::{self, Companion, ImportedCompanion, PublishedArtifact, SaveFailure, SaveReceipt},
    jobs::DocumentStore,
    storage::DocumentInfo,
};
use serde::Serialize;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ActivityResult {
    Opened { document: DocumentInfo },
    Imported { companion: ImportedCompanion },
    Saved { receipt: SaveReceipt },
}
#[derive(Clone, Debug, Serialize)]
pub struct ActivityError {
    pub message: String,
    pub published: Vec<PublishedArtifact>,
    pub unverified: Vec<String>,
}
impl From<String> for ActivityError {
    fn from(message: String) -> Self {
        Self {
            message,
            published: Vec::new(),
            unverified: Vec::new(),
        }
    }
}
impl From<SaveFailure> for ActivityError {
    fn from(error: SaveFailure) -> Self {
        Self {
            message: error.message,
            published: error.published,
            unverified: error.unverified,
        }
    }
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityInfo {
    pub id: String,
    pub state: String,
    pub phase: String,
    pub processed_bytes: String,
    pub total_bytes: Option<String>,
    pub result: Option<ActivityResult>,
    pub error: Option<ActivityError>,
}
struct Activity {
    info: Mutex<ActivityInfo>,
    control: Control,
}
struct Inner {
    store: DocumentStore,
    next: AtomicU64,
    activities: Mutex<HashMap<String, Arc<Activity>>>,
    drop: Mutex<Option<(String, PathBuf)>>,
}
#[derive(Clone)]
pub struct AppDocuments {
    inner: Arc<Inner>,
}
#[derive(Clone, Debug, Serialize)]
pub struct DropNotice {
    pub token: String,
    pub name: String,
}

impl AppDocuments {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            inner: Arc::new(Inner {
                store: DocumentStore::new()?,
                next: AtomicU64::new(0),
                activities: Mutex::new(HashMap::new()),
                drop: Mutex::new(None),
            }),
        })
    }
    pub fn store(&self) -> &DocumentStore {
        &self.inner.store
    }
    fn id(&self, prefix: &str) -> Result<String, String> {
        let next = self
            .inner
            .next
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| "Document operation identifier overflow.")?;
        Ok(format!("{prefix}-{next}"))
    }
    fn activity(&self, id: &str) -> Result<Arc<Activity>, String> {
        self.inner
            .activities
            .lock()
            .map_err(|_| "Document activity state is unavailable.")?
            .get(id)
            .cloned()
            .ok_or_else(|| "Unknown or discarded document activity.".into())
    }
    pub fn status(&self, id: &str) -> Result<ActivityInfo, String> {
        let activity = self.activity(id)?;
        let mut info = activity
            .info
            .lock()
            .map_err(|_| "Document activity state is unavailable.")?
            .clone();
        info.processed_bytes = activity.control.bytes.load(Ordering::Relaxed).to_string();
        Ok(info)
    }
    pub fn cancel(&self, id: &str) -> Result<(), String> {
        let activity = self.activity(id)?;
        let info = activity
            .info
            .lock()
            .map_err(|_| "Document activity state is unavailable.")?;
        if info.state == "running" {
            activity.control.cancel();
        }
        Ok(())
    }
    pub fn discard(&self, id: &str, keep_document: bool) -> Result<(), String> {
        let activity = self
            .inner
            .activities
            .lock()
            .map_err(|_| "Document activity state is unavailable.")?
            .remove(id);
        if let Some(activity) = activity {
            let info = activity
                .info
                .lock()
                .map_err(|_| "Document activity state is unavailable.")?;
            activity.control.cancel();
            if !keep_document {
                if let Some(ActivityResult::Opened { document }) = &info.result {
                    self.store().discard_document(&document.id)?;
                }
            }
        }
        Ok(())
    }
    pub fn spawn(
        &self,
        phase: &str,
        total_bytes: Option<String>,
        work: impl FnOnce(&Control) -> Result<ActivityResult, ActivityError> + Send + 'static,
    ) -> Result<ActivityInfo, String> {
        let id = self.id("activity")?;
        let info = ActivityInfo {
            id: id.clone(),
            state: "running".into(),
            phase: phase.into(),
            processed_bytes: "0".into(),
            total_bytes,
            result: None,
            error: None,
        };
        let activity = Arc::new(Activity {
            info: Mutex::new(info.clone()),
            control: Control::default(),
        });
        self.inner
            .activities
            .lock()
            .map_err(|_| "Document activity state is unavailable.")?
            .insert(id.clone(), activity.clone());
        let controller = self.clone();
        let worker = activity.clone();
        if let Err(error) = std::thread::Builder::new()
            .name("document-io".into())
            .spawn(move || {
                let outcome = work(&worker.control);
                if let Ok(mut info) = worker.info.lock() {
                    match outcome {
                        Ok(result) => {
                            // A verified save remains a completed effect even if a
                            // stop request arrives after its final verification.
                            if worker.control.check().is_err()
                                && !matches!(result, ActivityResult::Saved { .. })
                            {
                                if let ActivityResult::Opened { document } = result {
                                    let _ = controller.store().discard_document(&document.id);
                                }
                                info.state = "cancelled".into();
                                info.phase = "Cancelled".into();
                            } else {
                                info.state = "complete".into();
                                info.phase = "Complete".into();
                                info.result = Some(result);
                            }
                        }
                        Err(error) => {
                            info.state = if worker.control.check().is_err() {
                                "cancelled"
                            } else {
                                "failed"
                            }
                            .into();
                            info.phase = info.state.clone();
                            info.error = Some(error);
                        }
                    }
                }
            })
        {
            self.inner
                .activities
                .lock()
                .map_err(|_| "Document activity state is unavailable.")?
                .remove(&id);
            return Err(super::stream::io_error(
                "Cannot start document I/O worker",
                error,
            ));
        }
        Ok(info)
    }
    pub fn open(&self, path: PathBuf) -> Result<ActivityInfo, String> {
        let total = std::fs::metadata(&path)
            .ok()
            .map(|metadata| metadata.len().to_string());
        let store = self.store().clone();
        self.spawn("Opening and validating UTF-8", total, move |control| {
            let document = store.capture(&path, control)?;
            Ok(ActivityResult::Opened { document })
        })
    }
    pub fn import(&self, path: PathBuf, expected: String) -> Result<ActivityInfo, String> {
        self.spawn("Reading recipe companion", None, move |control| {
            control.check()?;
            let companion = export::read_companion(&path)?;
            if let ImportedCompanion::Recipe { recipe } = &companion {
                if !expected.trim().is_empty()
                    && crate::recipes::lookup::lookup(&expected, std::slice::from_ref(recipe))?
                        .is_none()
                {
                    return Err("Imported JSON does not match the supplied fingerprint."
                        .to_owned()
                        .into());
                }
            }
            control.check()?;
            Ok(ActivityResult::Imported { companion })
        })
    }
    pub fn save(
        &self,
        job_id: String,
        destination: PathBuf,
        companion: Companion,
    ) -> Result<ActivityInfo, String> {
        let store = self.store().clone();
        let result = store.job(&job_id)?.result()?;
        self.spawn(
            "Staging, publishing and verifying new files",
            Some(result.stats.bytes.to_string()),
            move |control| {
                Ok(ActivityResult::Saved {
                    receipt: export::save(&store, &job_id, &destination, companion, control)?,
                })
            },
        )
    }
    pub fn register_drop(&self, paths: Vec<PathBuf>) -> Result<DropNotice, String> {
        if paths.len() != 1 {
            return Err("Drop one text file at a time.".into());
        }
        let path = paths.into_iter().next().ok_or("Missing dropped file.")?;
        let notice = DropNotice {
            token: self.id("drop")?,
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
        };
        *self
            .inner
            .drop
            .lock()
            .map_err(|_| "Native drop state is unavailable.")? = Some((notice.token.clone(), path));
        Ok(notice)
    }
    pub fn open_drop(&self, token: &str) -> Result<ActivityInfo, String> {
        let mut pending = self
            .inner
            .drop
            .lock()
            .map_err(|_| "Native drop state is unavailable.")?;
        if pending.as_ref().is_none_or(|(id, _)| id != token) {
            return Err("This native file drop is no longer available.".into());
        }
        let (_, path) = pending.take().ok_or("Missing native file drop.")?;
        drop(pending);
        self.open(path)
    }
}
