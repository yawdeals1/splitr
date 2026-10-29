mod commands;
mod error;
mod naming;
mod pdf;
mod presets;
mod ranges;
mod settings;
mod strategy;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // Load PDFium from the bundled resource folder. A failure is reported by every PDF command.
        .setup(|app| {
            match app.path().resource_dir() {
                Ok(dir) => {
                    if let Err(e) = pdf::init(&dir.join("pdfium")) {
                        eprintln!("{e}");
                    }
                }
                Err(e) => eprintln!("Could not find the resource folder: {e}"),
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_presets,
            commands::parse_custom_ranges,
            commands::fixed_ranges,
            commands::validate_ranges,
            commands::open_pdf,
            commands::get_settings,
            commands::save_settings,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
