//! Background IPC boundary for the local library.
use super::*;

#[tauri::command]
pub async fn save_text_experiment(
    library: tauri::State<'_, Arc<Library>>,
    run_id: String,
    name: String,
) -> Result<Summary, String> {
    let library = library.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        library.save_text(&run_id, &name, &Control::default())
    })
    .await
    .map_err(|_| "Experiment save worker failed.")?
}
#[tauri::command]
pub async fn save_file_experiment(
    library: tauri::State<'_, Arc<Library>>,
    documents: tauri::State<'_, crate::documents::activities::AppDocuments>,
    job_id: String,
    reading_id: Option<String>,
    name: String,
) -> Result<Summary, String> {
    let library = library.inner().clone();
    let store = documents.store().clone();
    tauri::async_runtime::spawn_blocking(move || {
        library.save_file(
            &store,
            &job_id,
            reading_id.as_deref(),
            &name,
            &Control::default(),
        )
    })
    .await
    .map_err(|_| "Experiment save worker failed.")?
}
#[tauri::command]
pub async fn list_experiments(library: tauri::State<'_, Arc<Library>>) -> Result<Listing, String> {
    let library = library.inner().clone();
    tauri::async_runtime::spawn_blocking(move || library.list())
        .await
        .map_err(|_| "Experiment list worker failed.")?
}
#[tauri::command]
pub async fn delete_experiment(
    library: tauri::State<'_, Arc<Library>>,
    id: String,
) -> Result<(), String> {
    let library = library.inner().clone();
    tauri::async_runtime::spawn_blocking(move || library.delete(&id))
        .await
        .map_err(|_| "Experiment delete worker failed.")?
}
#[tauri::command]
pub async fn open_experiment(
    library: tauri::State<'_, Arc<Library>>,
    documents: tauri::State<'_, crate::documents::activities::AppDocuments>,
    id: String,
) -> Result<Opened, String> {
    let library = library.inner().clone();
    let store = documents.store().clone();
    tauri::async_runtime::spawn_blocking(move || library.open(&id, &store, &Control::default()))
        .await
        .map_err(|_| "Experiment open worker failed.")?
}
