mod commands;
mod error;
mod naming;
mod pdf;
mod presets;
mod ranges;
mod settings;
mod strategy;
mod viewer;

use tauri::{Manager, RunEvent};

use viewer::Viewer;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(Viewer::new(Viewer::session_cache_dir()))
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
            commands::render_page,
            commands::get_settings,
            commands::save_settings,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // NFR-07: delete this session's thumbnails when the app closes.
            if let RunEvent::Exit = event {
                app.state::<Viewer>().close();
            }
        });
}
