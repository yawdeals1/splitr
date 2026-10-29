mod ai;
mod commands;
mod error;
mod export;
mod key_detect;
mod keystore;
mod naming;
mod pdf;
mod presets;
mod ranges;
mod settings;
mod strategy;
mod user_presets;
mod viewer;

use tauri::{Manager, RunEvent};

use viewer::Viewer;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Page images are cached in the user's own cache folder (NFR-07). Folders left by a
            // crashed session are deleted first.
            let cache_root = match app.path().app_cache_dir() {
                Ok(dir) => dir.join("thumbnails"),
                Err(e) => {
                    eprintln!("Could not find the cache folder, using the temp folder: {e}");
                    std::env::temp_dir().join("splitr-thumbnails")
                }
            };
            Viewer::remove_stale_sessions(&cache_root);
            app.manage(Viewer::new(cache_root));
            app.manage(ai::AiCalls::default());

            // Load PDFium from the bundled resource folder. A failure is reported by every PDF command.
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
            commands::export_parts,
            commands::save_user_preset,
            commands::delete_user_preset,
            commands::get_settings,
            commands::save_settings,
            commands::save_api_key,
            commands::api_key_status,
            commands::delete_api_key,
            commands::saved_api_keys,
            commands::verify_api_key,
            commands::ask_ai,
            commands::cancel_ai,
            commands::default_ai_models,
            commands::list_ai_models,
            commands::detect_api_key,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // NFR-07: delete this session's thumbnails when the app closes.
            if let RunEvent::Exit = event
                && let Some(viewer) = app.try_state::<Viewer>()
            {
                viewer.close();
            }
        });
}
