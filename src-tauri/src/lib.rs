mod manager;
use manager::*;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            use tauri::Manager;
            app.manage(ManagerState::new(app.path().app_local_data_dir()?)?);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            set_game_path,
            import_mod,
            set_enabled,
            set_mods_enabled,
            remove_mod,
            remove_mods,
            install_loader,
            uninstall_loader,
            launch_game,
            open_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
