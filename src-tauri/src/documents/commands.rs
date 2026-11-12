//! Thin native-dialog/IPC boundary. Commands never accept arbitrary file paths.
use super::{
    activities::{ActivityInfo, AppDocuments},
    export::Companion,
    jobs::{Direction, JobInfo},
    storage::Preview,
};
use crate::recipes::{self, Recipe};
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub async fn doc_open(
    app: AppHandle,
    documents: State<'_, AppDocuments>,
) -> Result<Option<ActivityInfo>, String> {
    let controller = documents.inner().clone();
    let path = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Open UTF-8 text document")
            .add_filter("Text documents", &["txt", "md"])
            .add_filter("All files", &["*"])
            .blocking_pick_file()
    })
    .await
    .map_err(|_| "Native Open dialog failed.")?;
    path.map(|path| {
        controller.open(
            path.into_path()
                .map_err(|_| "The selected file is not a native filesystem path.")?,
        )
    })
    .transpose()
}
#[tauri::command]
pub fn doc_open_drop(
    documents: State<'_, AppDocuments>,
    token: String,
) -> Result<ActivityInfo, String> {
    documents.open_drop(&token)
}
#[tauri::command]
pub async fn doc_import(
    app: AppHandle,
    documents: State<'_, AppDocuments>,
    expected: String,
) -> Result<Option<ActivityInfo>, String> {
    let controller = documents.inner().clone();
    let path = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Import recipe JSON or fingerprint")
            .add_filter("Recipe companions", &["json", "txt"])
            .blocking_pick_file()
    })
    .await
    .map_err(|_| "Native Import dialog failed.")?;
    path.map(|path| {
        controller.import(
            path.into_path()
                .map_err(|_| "The companion is not a native filesystem path.")?,
            expected,
        )
    })
    .transpose()
}
#[tauri::command]
pub async fn doc_save(
    app: AppHandle,
    documents: State<'_, AppDocuments>,
    job_id: String,
    reading_id: Option<String>,
    companion: Companion,
) -> Result<Option<ActivityInfo>, String> {
    let controller = documents.inner().clone();
    let job = controller.store().job(&job_id)?;
    let info = controller.store().status(&job_id)?;
    job.result()?;
    let stem = job
        .source
        .source
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let direction = if matches!(info.direction, Direction::Encode) {
        "encoded"
    } else {
        "decoded"
    };
    let suggested = format!("{stem}.{direction}.txt");
    let path = tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .set_title("Save result as a new file")
            .set_file_name(suggested)
            .blocking_save_file()
    })
    .await
    .map_err(|_| "Native Save As dialog failed.")?;
    path.map(|path| {
        controller.save(
            job_id,
            reading_id,
            path.into_path()
                .map_err(|_| "The destination is not a native filesystem path.")?,
            companion,
        )
    })
    .transpose()
}
#[tauri::command]
pub fn doc_activity(
    documents: State<'_, AppDocuments>,
    id: String,
) -> Result<ActivityInfo, String> {
    documents.status(&id)
}
#[tauri::command]
pub fn doc_cancel_activity(documents: State<'_, AppDocuments>, id: String) -> Result<(), String> {
    documents.cancel(&id)
}
#[tauri::command]
pub fn doc_discard_activity(
    documents: State<'_, AppDocuments>,
    id: String,
    keep_document: bool,
) -> Result<(), String> {
    documents.discard(&id, keep_document)
}
#[tauri::command]
pub fn doc_start(
    documents: State<'_, AppDocuments>,
    document_id: String,
    recipe: Recipe,
    key: String,
    direction: Direction,
) -> Result<JobInfo, String> {
    documents
        .store()
        .start(&document_id, recipe, key, direction)
}
#[tauri::command]
pub fn doc_job(documents: State<'_, AppDocuments>, id: String) -> Result<JobInfo, String> {
    documents.store().status(&id)
}
#[tauri::command]
pub fn doc_cancel_job(documents: State<'_, AppDocuments>, id: String) -> Result<(), String> {
    documents.store().cancel(&id)
}
#[tauri::command]
pub fn doc_discard_job(documents: State<'_, AppDocuments>, id: String) -> Result<(), String> {
    documents.store().discard_job(&id)
}
#[tauri::command]
pub fn doc_discard_document(documents: State<'_, AppDocuments>, id: String) -> Result<(), String> {
    documents.store().discard_document(&id)
}
#[tauri::command]
pub async fn doc_preview(
    documents: State<'_, AppDocuments>,
    document_id: Option<String>,
    job_id: Option<String>,
    reading_id: Option<String>,
    stage: Option<usize>,
    offset: String,
) -> Result<Preview, String> {
    let store = documents.store().clone();
    let offset: u64 = offset.parse().map_err(|_| "Invalid preview byte offset.")?;
    tauri::async_runtime::spawn_blocking(move || match (document_id, job_id, reading_id) {
        (Some(id), None, None) => store.preview_document(&id, offset),
        (None, Some(id), None) => store.preview_job(&id, stage, offset),
        (None, Some(id), Some(reading)) if stage.is_none() => {
            super::reconstruct::preview(&store, &id, &reading, offset)
        }
        _ => Err("Select exactly one preview source.".into()),
    })
    .await
    .map_err(|_| "Document preview worker failed.")?
}
#[tauri::command]
pub fn resolve_recipe_identifier(
    app: AppHandle,
    identifier: String,
) -> Result<Option<Recipe>, String> {
    let mut available = recipes::builtins()?;
    available.extend(recipes::store::list(&app)?);
    recipes::lookup::lookup(&identifier, &available)
}

#[tauri::command]
pub fn doc_reconstruct(
    documents: State<'_, AppDocuments>,
    job_id: String,
    choices: Vec<super::reconstruct::DocChoice>,
    key: Option<String>,
) -> Result<ActivityInfo, String> {
    documents.reconstruct_with_key(job_id, choices, key)
}

#[tauri::command]
pub async fn doc_candidate_page(
    documents: State<'_, AppDocuments>,
    job_id: String,
    reading_id: String,
    offset: String,
) -> Result<super::reconstruct::CandidatePage, String> {
    let store = documents.store().clone();
    let offset = offset
        .parse()
        .map_err(|_| "Invalid candidate byte offset.")?;
    tauri::async_runtime::spawn_blocking(move || {
        super::reconstruct::page(&store, &job_id, &reading_id, offset)
    })
    .await
    .map_err(|_| "Candidate page worker failed.")?
}
