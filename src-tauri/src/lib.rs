mod output;
mod showfile;

use output::input::DmxInputManager;
use output::manager::OutputManager;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let manager = OutputManager::new(app.handle().clone());
            app.manage(manager);
            app.manage(DmxInputManager::new());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            output::commands::list_serial_ports,
            output::commands::output_configure_universe,
            output::commands::output_remove_universe,
            output::commands::output_update_universe_data,
            output::commands::output_get_universe_data,
            output::commands::output_universe_status,
            output::commands::output_all_statuses,
            output::commands::output_set_blackout,
            output::commands::output_get_blackout,
            output::commands::output_get_universe_config,
            output::commands::output_driver_defaults,
            output::commands::output_driver_label,
            output::commands::output_discover_artnet_nodes,
            output::commands::input_start_artnet,
            output::commands::input_get_artnet_frame,
            output::commands::input_start_sacn,
            output::commands::input_stop_sacn,
            output::commands::input_get_sacn_frame,
            showfile::commands::showfile_new,
            showfile::commands::showfile_open,
            showfile::commands::showfile_save,
            showfile::commands::showfile_autosave,
            showfile::commands::showfile_recent,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
