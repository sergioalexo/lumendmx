use std::collections::HashMap;

use tauri::State;

use super::legacy_playback::{LegacyAsset, LegacyStep};
use super::manager::{CommandResult, EngineManager, PatchIndexEntry};

#[tauri::command]
pub fn engine_set_htp_channels(engine: State<EngineManager>, universe: u32, channels: Vec<u16>) {
    engine.set_htp_channels(universe, channels);
}

#[tauri::command]
pub fn engine_set_patch_index(engine: State<EngineManager>, entries: Vec<PatchIndexEntry>) {
    engine.set_patch_index(entries);
}

/// Immediate (unfaded) programmer set — sliders, color-preset buttons, the
/// Live AI console.
#[tauri::command]
pub fn engine_set_channels(engine: State<EngineManager>, universe: u32, values: HashMap<u16, u8>) {
    engine.set_channels(universe, &values);
}

#[tauri::command]
pub fn engine_get_universe_data(engine: State<EngineManager>, universe: u32) -> Vec<u8> {
    engine.get_universe_data(universe).to_vec()
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyStepDto {
    /// channel (1-512, as a string key, matching the show file's LegacyStep) -> value.
    pub channels: HashMap<String, u8>,
    pub fade_ms: f64,
    pub hold_ms: f64,
}

/// Starts (or restarts, if already running) a legacy Scene/Chase/FX asset as
/// an engine playback layer — the frame-accurate replacement for the old
/// `chaseEngine.ts` `setTimeout` loop.
#[tauri::command]
pub fn engine_trigger_asset(
    engine: State<EngineManager>,
    id: String,
    universe: u32,
    steps: Vec<LegacyStepDto>,
    looped: bool,
) -> Result<(), String> {
    let steps = steps
        .into_iter()
        .map(|s| {
            let channels = s
                .channels
                .into_iter()
                .map(|(k, v)| k.parse::<u16>().map(|ch| (ch, v)).map_err(|_| format!("Invalid channel '{k}'")))
                .collect::<Result<HashMap<u16, u8>, String>>()?;
            Ok(LegacyStep { channels, fade_ms: s.fade_ms, hold_ms: s.hold_ms })
        })
        .collect::<Result<Vec<LegacyStep>, String>>()?;

    engine.trigger_asset(id, universe, LegacyAsset { steps, loop_: looped });
    Ok(())
}

#[tauri::command]
pub fn engine_stop_asset(engine: State<EngineManager>, id: String) {
    engine.stop_asset(&id);
}

#[tauri::command]
pub fn engine_execute_command(engine: State<EngineManager>, text: String) -> CommandResult {
    engine.execute_command(&text)
}
