mod engine;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            encode_message,
            decode_message,
            choose_reconstruction
        ])
        .run(tauri::generate_context!())
        .expect("Tauri failed to start");
}
