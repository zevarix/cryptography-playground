mod engine;

#[tauri::command]
fn encode_message(text: String, key: String) -> Result<engine::Encoded, String> {
    engine::encode(&text, &key)
}

#[tauri::command]
fn decode_message(ciphertext: String, key: String) -> Result<engine::Decoded, String> {
    engine::decode(&ciphertext, &key)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![encode_message, decode_message])
        .run(tauri::generate_context!())
        .expect("Tauri failed to start");
}
