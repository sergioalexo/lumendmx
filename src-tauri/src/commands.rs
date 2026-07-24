use std::sync::atomic::Ordering;

use tauri::State;

use crate::dmx::{ConnectionStatus, DmxEngine, UNIVERSE_SIZE};

#[tauri::command]
pub fn list_serial_ports() -> Result<Vec<String>, String> {
    serialport::available_ports()
        .map(|ports| ports.into_iter().map(|p| p.port_name).collect())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn connect_serial_port(engine: State<DmxEngine>, port_name: String) -> Result<(), String> {
    engine.request_port(Some(port_name));
    Ok(())
}

#[tauri::command]
pub fn disconnect_serial_port(engine: State<DmxEngine>) -> Result<(), String> {
    engine.request_port(None);
    Ok(())
}

#[tauri::command]
pub fn connection_status(engine: State<DmxEngine>) -> ConnectionStatus {
    engine.current_status()
}

/// Overwrites the entire shared DMX universe. Called whenever a Scene/Chase is
/// triggered or the AI modifies the running state (SRS 2.3, step 2).
#[tauri::command]
pub fn update_universe(engine: State<DmxEngine>, channels: Vec<u8>) -> Result<(), String> {
    if channels.len() != UNIVERSE_SIZE {
        return Err(format!(
            "Expected {} channels, got {}",
            UNIVERSE_SIZE,
            channels.len()
        ));
    }
    let mut data = [0u8; UNIVERSE_SIZE];
    data.copy_from_slice(&channels);
    engine.set_universe(data);
    Ok(())
}

#[tauri::command]
pub fn get_universe(engine: State<DmxEngine>) -> Vec<u8> {
    engine.universe.lock().unwrap().to_vec()
}

/// Safety override (PRD gap fix): forces all 512 channels to 0 on the wire without
/// touching the stored universe buffer, so releasing blackout resumes exactly where
/// the show was.
#[tauri::command]
pub fn set_blackout(engine: State<DmxEngine>, active: bool) -> Result<(), String> {
    engine.blackout.store(active, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub fn get_blackout(engine: State<DmxEngine>) -> bool {
    engine.blackout.load(Ordering::Relaxed)
}
