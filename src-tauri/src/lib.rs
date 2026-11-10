mod documents;
mod engine;
mod recipes;
use tauri::{Emitter, Manager};

#[tauri::command]
fn encode_message(text: String, key: String) -> Result<engine::Encoded, String> {
    engine::encode(&text, &key)
}

#[tauri::command]
fn decode_message(ciphertext: String, key: String) -> Result<engine::Decoded, String> {
    engine::decode(&ciphertext, &key)
}

#[tauri::command]
fn choose_reconstruction(
    ciphertext: String,
    key: String,
    choices: Vec<engine::WordChoice>,
) -> Result<engine::Decoded, String> {
    engine::decode_with_choices(&ciphertext, &key, &choices)
}

#[tauri::command]
fn builtin_recipes() -> Result<Vec<recipes::Recipe>, String> {
    recipes::builtins()
}

#[tauri::command]
fn list_saved_recipes(app: tauri::AppHandle) -> Result<Vec<recipes::Recipe>, String> {
    recipes::store::list(&app)
}

#[tauri::command]
fn save_recipe(app: tauri::AppHandle, recipe: recipes::Recipe) -> Result<recipes::Recipe, String> {
    recipes::store::save(&app, recipe)
}

#[tauri::command]
fn delete_saved_recipe(app: tauri::AppHandle, id: String) -> Result<(), String> {
    recipes::store::delete(&app, &id)
}

#[tauri::command]
fn recipe_fingerprint(
    recipe: recipes::Recipe,
) -> Result<recipes::fingerprint::RecipeFingerprint, String> {
    recipes::fingerprint::fingerprint(&recipe)
}

#[tauri::command]
fn evaluate_recipe(
    recipe: recipes::Recipe,
    text: String,
    key: String,
    direction: String,
    choices: Vec<engine::WordChoice>,
) -> Result<recipes::RecipeResult, String> {
    match direction.as_str() {
        "encode" => {
            if !choices.is_empty() {
                return Err("Cannot select plaintext candidates while encoding.".into());
            }
            recipes::encode(&recipe, &text, &key)
        }
        "decode" => recipes::decode(&recipe, &text, &key, &choices),
        _ => Err("Direction must be encode or decode.".into()),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            app.manage(documents::activities::AppDocuments::new().map_err(std::io::Error::other)?);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) = event {
                if let Some(documents) = window
                    .app_handle()
                    .try_state::<documents::activities::AppDocuments>()
                {
                    match documents.register_drop(paths.clone()) {
                        Ok(notice) => {
                            let _ = window.emit("document-drop", notice);
                        }
                        Err(error) => {
                            let _ = window.emit("document-drop-error", error);
                        }
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            encode_message,
            decode_message,
            choose_reconstruction,
            builtin_recipes,
            list_saved_recipes,
            save_recipe,
            delete_saved_recipe,
            evaluate_recipe,
            recipe_fingerprint,
            documents::commands::doc_open,
            documents::commands::doc_open_drop,
            documents::commands::doc_import,
            documents::commands::doc_save,
            documents::commands::doc_activity,
            documents::commands::doc_cancel_activity,
            documents::commands::doc_discard_activity,
            documents::commands::doc_start,
            documents::commands::doc_job,
            documents::commands::doc_cancel_job,
            documents::commands::doc_discard_job,
            documents::commands::doc_discard_document,
            documents::commands::doc_preview,
            documents::commands::resolve_recipe_identifier,
            documents::commands::doc_reconstruct,
            documents::commands::doc_candidate_page
        ])
        .run(tauri::generate_context!())
        .expect("Tauri failed to start");
}
