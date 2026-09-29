use std::sync::atomic::Ordering;

use serde::Serialize;
use tauri::State;

use crate::dmx::{ConnectionStatus, DmxEngine, UNIVERSE_SIZE};

#[derive(Serialize)]
pub struct SerialPortDescriptor {
    pub port_name: String,
    /// Human-readable label (USB product/manufacturer when available) so the UI
    /// can tell the user which entry is their FTDI adapter instead of a bare COMn.
    pub label: String,
}

#[tauri::command]
pub fn list_serial_ports() -> Result<Vec<SerialPortDescriptor>, String> {
    serialport::available_ports()
        .map(|ports| {
            ports
                .into_iter()
                .map(|p| {
                    let label = match &p.port_type {
                        serialport::SerialPortType::UsbPort(info) => {
                            let name = info
                                .product
                                .as_deref()
                                .filter(|s| !s.trim().is_empty())
                                .or(info.manufacturer.as_deref())
                                .map(str::to_string)
                                .unwrap_or_else(|| format!("USB {:04x}:{:04x}", info.vid, info.pid));
                            format!("{} — {}", p.port_name, name)
                        }
                        _ => p.port_name.clone(),
                    };
                    SerialPortDescriptor {
                        port_name: p.port_name,
                        label,
                    }
                })
                .collect()
        })
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
