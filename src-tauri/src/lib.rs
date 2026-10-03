mod prompt;
mod pty;

use pty::PtyManager;
use std::sync::Arc;
use tauri::Manager;

#[tauri::command]
async fn spawn_pty(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Arc<PtyManager>>,
    cols: u16,
    rows: u16,
) -> Result<String, String> {
    state.spawn(&app_handle, cols, rows)
}

#[tauri::command]
async fn write_pty(
    state: tauri::State<'_, Arc<PtyManager>>,
    session_id: String,
    data: String,
) -> Result<(), String> {
    state.write(&session_id, &data)
}

#[tauri::command]
async fn resize_pty(
    state: tauri::State<'_, Arc<PtyManager>>,
    session_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    state.resize(&session_id, cols, rows)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let pty_manager = Arc::new(PtyManager::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .manage(pty_manager)
        .invoke_handler(tauri::generate_handler![
            spawn_pty,
            write_pty,
            resize_pty,
        ])
        .setup(|app| {
            // Apply vibrancy effect on Windows 11 safely without crashing
            #[cfg(target_os = "windows")]
            {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window_vibrancy::apply_acrylic(&window, Some((18, 18, 18, 125)))
                        .or_else(|_| window_vibrancy::apply_mica(&window, Some(true)));
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Atlas Terminal");
}
