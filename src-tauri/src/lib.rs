mod manager;
use manager::*;
use std::path::PathBuf;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, cwd| {
            if let Some(state) = app.try_state::<ManagerState>() {
                let cwd = PathBuf::from(cwd);
                open_package_args(&state, args.into_iter().skip(1).map(|arg| cwd.join(arg)));
            }
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            use tauri::Manager;
            let state = ManagerState::new(app.path().app_local_data_dir()?)?;
            open_package_args(&state, std::env::args_os().skip(1).map(PathBuf::from));
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            set_game_path,
            import_mod,
            import_mods_package,
            stage_scriptclient_package,
            take_opened_packages,
            export_mods_package,
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
