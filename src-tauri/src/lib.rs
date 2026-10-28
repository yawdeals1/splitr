mod commands;
mod error;
mod naming;
mod pdf;
mod presets;
mod ranges;
mod strategy;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::list_presets,
            commands::parse_custom_ranges,
            commands::fixed_ranges,
            commands::validate_ranges,
            commands::open_pdf,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
