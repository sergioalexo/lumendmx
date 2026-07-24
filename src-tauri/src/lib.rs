mod commands;
mod dmx;

use dmx::DmxEngine;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_http::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let engine = DmxEngine::new(app.handle().clone());
            app.manage(engine);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_serial_ports,
            commands::connect_serial_port,
            commands::disconnect_serial_port,
            commands::connection_status,
            commands::update_universe,
            commands::get_universe,
            commands::set_blackout,
            commands::get_blackout,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
