use std::collections::{HashMap, HashSet};

use tauri::State;

use super::groups::Group;
use super::legacy_playback::{LegacyAsset, LegacyStep};
use super::manager::{CommandResult, EngineManager, PatchIndexEntry};
use super::presets::{Preset, PresetFamily};

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

// --- Groups (Phase 5) ---

#[tauri::command]
pub fn engine_record_group(engine: State<EngineManager>, name: String, fixture_numbers: Vec<u32>) -> u32 {
    engine.record_group(name, fixture_numbers)
}

#[tauri::command]
pub fn engine_list_groups(engine: State<EngineManager>) -> Vec<Group> {
    engine.list_groups()
}

#[tauri::command]
pub fn engine_rename_group(engine: State<EngineManager>, id: u32, name: String) -> Result<(), String> {
    engine.rename_group(id, name)
}

#[tauri::command]
pub fn engine_delete_group(engine: State<EngineManager>, id: u32) {
    engine.delete_group(id);
}

#[tauri::command]
pub fn engine_apply_group_master(engine: State<EngineManager>, id: u32, percent: f32) -> Result<String, String> {
    let (_ok, message) = engine.apply_group_master(id, percent)?;
    Ok(message)
}

// --- Presets (Phase 5) ---

#[tauri::command]
pub fn engine_record_preset(
    engine: State<EngineManager>,
    family: PresetFamily,
    name: String,
    fixture_numbers: Vec<u32>,
    color: Option<String>,
) -> u32 {
    engine.record_preset(family, name, &fixture_numbers.into_iter().collect::<HashSet<u32>>(), color)
}

#[tauri::command]
pub fn engine_update_preset(
    engine: State<EngineManager>,
    id: u32,
    fixture_numbers: Vec<u32>,
    color: Option<String>,
) -> Result<(), String> {
    engine.update_preset(id, &fixture_numbers.into_iter().collect::<HashSet<u32>>(), color)
}

#[tauri::command]
pub fn engine_rename_preset(engine: State<EngineManager>, id: u32, name: String) -> Result<(), String> {
    engine.rename_preset(id, name)
}

#[tauri::command]
pub fn engine_delete_preset(engine: State<EngineManager>, id: u32) {
    engine.delete_preset(id);
}

#[tauri::command]
pub fn engine_list_presets(engine: State<EngineManager>, family: Option<PresetFamily>) -> Vec<Preset> {
    engine.list_presets(family)
}

#[tauri::command]
pub fn engine_apply_preset(engine: State<EngineManager>, id: u32, fixture_numbers: Vec<u32>) -> Result<usize, String> {
    engine.apply_preset(id, &fixture_numbers.into_iter().collect::<HashSet<u32>>())
}
