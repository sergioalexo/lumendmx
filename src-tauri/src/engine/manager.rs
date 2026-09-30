//! Ties the engine's pieces together as Tauri-managed state: live merge
//! state, running legacy-asset playbacks, the fixture-number index the
//! command line resolves against, and the 44Hz tick thread.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use ts_rs::TS;

use super::command_line::{self, Command};
use super::cues::{Cue, CueStore, CueValue, Cuelist, CuelistKind};
use super::effects::{EffectConfig, EffectDirection, EffectMode, EffectRate, EffectRuntime, EffectStore, Waveform};
use super::groups::{Group, GroupStore};
use super::legacy_playback::{LegacyAsset, RunningPlayback};
use super::merge::merge_universe;
use super::pixelmap::{PixelGenerator, PixelMap, PixelMapRuntime, PixelMapStore};
use super::playback::{FaderMode, PlaybackRuntime};
use super::presets::{Preset, PresetFamily, PresetStore};
use super::state::EngineState;
use crate::output::manager::OutputManager;
use crate::output::UNIVERSE_SIZE;

const TICK_HZ: f64 = 44.0;
const DISPLAY_HZ: f64 = 20.0;
/// A cuelist marked `Override` gets a priority at least this high (below the
/// programmer's `i64::MAX`), so it wins LTP over any standard/chase playback
/// or legacy-asset layer regardless of trigger order.
pub const OVERRIDE_PRIORITY_BASE: i64 = 1_000_000_000;
/// Default fade time for `playback_release` when the caller doesn't specify one.
const DEFAULT_RELEASE_FADE_MS: f64 = 500.0;

/// What the command line's `1 THRU 8` etc. resolve fixture numbers against,
/// and which channel `@ value` should drive for each. Pushed from the
/// frontend whenever the patch changes (see docs/ENGINE.md) — fixture/channel
/// resolution is static config data, not timing-critical, so it stays where
/// the fixture library already lives (TypeScript) rather than being
/// duplicated in Rust.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/showfile/generated/")]
pub struct PatchIndexEntry {
    pub fixture_number: u32,
    pub universe: u32,
    /// The channel `@ value` should drive, if this fixture has a dimmer
    /// channel at all.
    pub intensity_channel: Option<u16>,
    /// Every other attribute name (e.g. "red", "pan", "gobo") this fixture
    /// has, mapped to its channel — what a preset's `apply` resolves
    /// attribute values against for an arbitrary target fixture (Phase 5).
    #[serde(default)]
    pub attribute_channels: HashMap<String, u16>,
}

#[derive(Default)]
struct ProgrammerContext {
    selection: HashSet<u32>,
    /// CLEAR's first press clears the selection; a second press (selection
    /// already empty) clears the programmer's values instead.
    pending_time_s: f32,
    /// The cuelist `RECORD CUE`/`UPDATE`/`DELETE CUE`/`NEXT`/`PREV` operate
    /// against — set by `set_current_cuelist` when the Cuelists tab selects
    /// one (Phase 6). `None` until then.
    current_cuelist: Option<u32>,
    /// The last cue number `RECORD CUE` touched, so a bare `UPDATE` (which
    /// takes no number, per the command line's grammar) knows which cue to
    /// re-capture.
    last_cue_number: Option<f64>,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/showfile/generated/")]
pub struct EngineFrame {
    pub universe_id: u32,
    pub channels: Vec<u8>,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/showfile/generated/")]
pub struct CommandResult {
    pub ok: bool,
    pub message: String,
    pub selection: Vec<u32>,
}

/// `Cue.values` is keyed by `(u32, u16)`, which can't cross the Tauri
/// boundary as-is (JSON object keys must be strings) — these DTOs flatten it
/// into a plain list. See `cues.rs`'s module docs.
#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/showfile/generated/")]
pub struct CueValueEntry {
    pub universe: u32,
    pub channel: u16,
    pub value: CueValue,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/showfile/generated/")]
pub struct CueDto {
    pub number: f64,
    pub name: String,
    pub values: Vec<CueValueEntry>,
    pub fade_in_ms: f64,
    pub fade_out_ms: f64,
    pub delay_in_ms: f64,
    pub mark: bool,
}

impl From<&Cue> for CueDto {
    fn from(cue: &Cue) -> Self {
        let mut values: Vec<CueValueEntry> = cue
            .values
            .iter()
            .map(|(&(universe, channel), value)| CueValueEntry { universe, channel, value: value.clone() })
            .collect();
        values.sort_by_key(|e| (e.universe, e.channel));
        Self {
            number: cue.number,
            name: cue.name.clone(),
            values,
            fade_in_ms: cue.fade_in_ms,
            fade_out_ms: cue.fade_out_ms,
            delay_in_ms: cue.delay_in_ms,
            mark: cue.mark,
        }
    }
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/showfile/generated/")]
pub struct CuelistDto {
    pub id: u32,
    pub name: String,
    pub kind: CuelistKind,
    pub tracking: bool,
    pub cues: Vec<CueDto>,
}

impl From<&Cuelist> for CuelistDto {
    fn from(list: &Cuelist) -> Self {
        Self {
            id: list.id,
            name: list.name.clone(),
            kind: list.kind,
            tracking: list.tracking,
            cues: list.cues.iter().map(CueDto::from).collect(),
        }
    }
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/showfile/generated/")]
pub struct PlaybackStatus {
    pub id: u32,
    pub cuelist_id: u32,
    pub current_cue_number: Option<f64>,
    pub fader: f32,
    pub fader_mode: FaderMode,
    pub paused: bool,
}

/// The (universe, channel) an effect's target attribute resolves to for one
/// fixture, resolved once when the effect starts running (see
/// `EngineManager::start_effect`) — same "resolve fixture selection once,
/// not every tick" approach as a cue's own values.
struct RunningEffect {
    runtime: EffectRuntime,
    targets: Vec<(u32, u16)>,
}

/// One pixel-map cell's resolved fixture channels. RGB channels are used
/// when the fixture has them; a dimmer-only fixture falls back to the
/// color's max component as a brightness value.
struct CellTarget {
    universe: u32,
    red: Option<u16>,
    green: Option<u16>,
    blue: Option<u16>,
    dimmer: Option<u16>,
}

struct RunningPixelMap {
    runtime: PixelMapRuntime,
    /// Parallel to the source `PixelMap.cells` — `None` for an empty cell or
    /// one whose fixture isn't patched with a usable channel.
    cell_targets: Vec<Option<CellTarget>>,
}

#[derive(Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/showfile/generated/")]
pub struct EffectConfigInput {
    pub name: String,
    pub waveform: Waveform,
    pub attribute: String,
    pub size_percent: f32,
    pub rate: EffectRate,
    pub phase_spread_deg: f32,
    pub width: f32,
    pub direction: EffectDirection,
    pub block_size: u32,
    pub mirror: bool,
    pub mode: EffectMode,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/showfile/generated/")]
pub struct EffectRunStatus {
    pub id: u32,
    pub effect_id: u32,
}

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/showfile/generated/")]
pub struct PixelMapRunStatus {
    pub id: u32,
    pub pixelmap_id: u32,
}

pub struct EngineManager {
    state: Arc<Mutex<EngineState>>,
    playbacks: Arc<Mutex<Vec<RunningPlayback>>>,
    patch_index: Arc<Mutex<Vec<PatchIndexEntry>>>,
    programmer_ctx: Mutex<ProgrammerContext>,
    groups: Mutex<GroupStore>,
    presets: Arc<Mutex<PresetStore>>,
    cues: Arc<Mutex<CueStore>>,
    cue_playbacks: Arc<Mutex<Vec<PlaybackRuntime>>>,
    next_playback_id: AtomicU32,
    effects: Arc<Mutex<EffectStore>>,
    effect_runtimes: Arc<Mutex<Vec<RunningEffect>>>,
    next_effect_runtime_id: AtomicU32,
    pixelmaps: Arc<Mutex<PixelMapStore>>,
    pixelmap_runtimes: Arc<Mutex<Vec<RunningPixelMap>>>,
    next_pixelmap_runtime_id: AtomicU32,
    tick_started: AtomicBool,
}

impl EngineManager {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(EngineState::new())),
            playbacks: Arc::new(Mutex::new(Vec::new())),
            patch_index: Arc::new(Mutex::new(Vec::new())),
            programmer_ctx: Mutex::new(ProgrammerContext::default()),
            groups: Mutex::new(GroupStore::new()),
            presets: Arc::new(Mutex::new(PresetStore::new())),
            cues: Arc::new(Mutex::new(CueStore::new())),
            cue_playbacks: Arc::new(Mutex::new(Vec::new())),
            next_playback_id: AtomicU32::new(0),
            effects: Arc::new(Mutex::new(EffectStore::new())),
            effect_runtimes: Arc::new(Mutex::new(Vec::new())),
            next_effect_runtime_id: AtomicU32::new(0),
            pixelmaps: Arc::new(Mutex::new(PixelMapStore::new())),
            pixelmap_runtimes: Arc::new(Mutex::new(Vec::new())),
            next_pixelmap_runtime_id: AtomicU32::new(0),
            tick_started: AtomicBool::new(false),
        }
    }

    /// Starts the 44Hz tick thread (idempotent — safe to call from `setup`
    /// every launch).
    pub fn start_tick(&self, app: AppHandle) {
        if self.tick_started.swap(true, Ordering::SeqCst) {
            return;
        }

        let state = self.state.clone();
        let playbacks = self.playbacks.clone();
        let cues = self.cues.clone();
        let cue_playbacks = self.cue_playbacks.clone();
        let presets = self.presets.clone();
        let effects = self.effects.clone();
        let effect_runtimes = self.effect_runtimes.clone();
        let pixelmaps = self.pixelmaps.clone();
        let pixelmap_runtimes = self.pixelmap_runtimes.clone();

        thread::spawn(move || {
            let tick_interval = Duration::from_secs_f64(1.0 / TICK_HZ);
            let display_interval = Duration::from_secs_f64(1.0 / DISPLAY_HZ);
            let mut last_tick = Instant::now();
            let mut last_display = Instant::now();

            loop {
                let now = Instant::now();
                let dt_ms = now.duration_since(last_tick).as_secs_f64() * 1000.0;
                last_tick = now;

                {
                    let mut state = state.lock().unwrap();
                    let mut playbacks = playbacks.lock().unwrap();
                    playbacks.retain_mut(|playback| {
                        let entry = state
                            .playbacks
                            .entry(playback.id.clone())
                            .or_insert_with(|| (playback.priority, Default::default()));
                        // `advance` returning false only means "stop ticking
                        // this playback" — a finished non-looping asset's
                        // last values stay live in `state.playbacks` (a Scene
                        // applies once and persists; see LightAsset's
                        // docstring) until `stop_asset`/`trigger_asset`
                        // explicitly removes that layer.
                        playback.advance(dt_ms, &mut entry.1)
                    });

                    // Cue playbacks (Phase 6) render into the same
                    // `state.playbacks` map, keyed distinctly, so they merge
                    // through exactly the same HTP/LTP path as legacy assets.
                    let cues = cues.lock().unwrap();
                    let presets = presets.lock().unwrap();
                    for pb in cue_playbacks.lock().unwrap().iter_mut() {
                        let key = format!("cue-playback-{}", pb.id);
                        let Some(cuelist) = cues.get(pb.cuelist_id) else {
                            state.playbacks.remove(&key);
                            continue;
                        };
                        pb.tick(dt_ms, cuelist, &presets);
                        let rendered = pb.render(&state.htp_channels);
                        let entry = state.playbacks.entry(key).or_insert_with(|| (pb.priority, Default::default()));
                        entry.0 = pb.priority;
                        entry.1.values = rendered;
                    }

                    // Effects (Phase 7): a running effect's already-resolved
                    // (universe, channel) targets, rendered fresh every tick
                    // from its live `EffectConfig` (so editing a running
                    // effect's config takes effect immediately).
                    let effects = effects.lock().unwrap();
                    for running in effect_runtimes.lock().unwrap().iter_mut() {
                        let key = format!("effect-{}", running.runtime.id);
                        let Some(config) = effects.get(running.runtime.effect_id) else {
                            state.playbacks.remove(&key);
                            continue;
                        };
                        running.runtime.tick(dt_ms);
                        let rendered = running.runtime.render(config, &running.targets);
                        let entry =
                            state.playbacks.entry(key).or_insert_with(|| (running.runtime.priority, Default::default()));
                        entry.0 = running.runtime.priority;
                        entry.1.values = rendered;
                    }

                    // Pixel maps (Phase 7): each cell's generated color is
                    // resolved to its fixture's RGB channels (or dimmer, for
                    // a dimmer-only fixture) via the cell targets captured
                    // when the pixel map started running.
                    let pixelmaps = pixelmaps.lock().unwrap();
                    for running in pixelmap_runtimes.lock().unwrap().iter_mut() {
                        let key = format!("pixelmap-{}", running.runtime.id);
                        let Some(map) = pixelmaps.get(running.runtime.pixelmap_id) else {
                            state.playbacks.remove(&key);
                            continue;
                        };
                        running.runtime.tick(dt_ms);
                        let colors = running.runtime.render(map);
                        let mut values = HashMap::new();
                        for (cell, color) in running.cell_targets.iter().zip(colors.iter()) {
                            let Some(target) = cell else { continue };
                            if let (Some(r), Some(g), Some(b)) = (target.red, target.green, target.blue) {
                                values.insert((target.universe, r), color.r * 255.0);
                                values.insert((target.universe, g), color.g * 255.0);
                                values.insert((target.universe, b), color.b * 255.0);
                            } else if let Some(d) = target.dimmer {
                                values.insert((target.universe, d), color.r.max(color.g).max(color.b) * 255.0);
                            }
                        }
                        let entry =
                            state.playbacks.entry(key).or_insert_with(|| (running.runtime.priority, Default::default()));
                        entry.0 = running.runtime.priority;
                        entry.1.values = values;
                    }
                }

                let output = app.state::<OutputManager>();
                let universe_ids = output.universe_ids();
                let show_display = now.duration_since(last_display) >= display_interval;

                for universe_id in universe_ids {
                    let frame = {
                        let state = state.lock().unwrap();
                        merge_universe(&state, universe_id)
                    };
                    let _ = output.set_universe_data(universe_id, frame);

                    if show_display {
                        let _ = app.emit(
                            "engine://universe-frame",
                            EngineFrame { universe_id, channels: frame.to_vec() },
                        );
                    }
                }
                if show_display {
                    last_display = now;
                }

                let elapsed = now.elapsed();
                if elapsed < tick_interval {
                    thread::sleep(tick_interval - elapsed);
                }
            }
        });
    }

    pub fn set_htp_channels(&self, universe: u32, channels: Vec<u16>) {
        let mut state = self.state.lock().unwrap();
        state.htp_channels.insert(universe, channels.into_iter().collect());
    }

    pub fn set_patch_index(&self, entries: Vec<PatchIndexEntry>) {
        *self.patch_index.lock().unwrap() = entries;
    }

    /// Immediate (unfaded) set into the programmer layer — what slider drags,
    /// color-preset buttons, and the Live AI console use.
    pub fn set_channels(&self, universe: u32, values: &HashMap<u16, u8>) {
        let mut state = self.state.lock().unwrap();
        for (&channel, &value) in values {
            state.programmer.values.insert((universe, channel), value as f32);
        }
    }

    pub fn get_universe_data(&self, universe: u32) -> [u8; UNIVERSE_SIZE] {
        let state = self.state.lock().unwrap();
        merge_universe(&state, universe)
    }

    pub fn trigger_asset(&self, id: String, universe: u32, asset: LegacyAsset) {
        self.stop_asset(&id);
        let priority = {
            let mut state = self.state.lock().unwrap();
            let priority = state.next_priority();
            state.playbacks.insert(id.clone(), (priority, Default::default()));
            priority
        };
        self.playbacks.lock().unwrap().push(RunningPlayback::new(id, universe, priority, asset));
    }

    pub fn stop_asset(&self, id: &str) {
        self.playbacks.lock().unwrap().retain(|p| p.id != id);
        self.state.lock().unwrap().playbacks.remove(id);
    }

    /// Parses and executes one command-line entry against the current
    /// selection/patch index. See `command_line`'s module docs for the
    /// supported grammar and what's a parse-only stub (groups, cues).
    pub fn execute_command(&self, text: &str) -> CommandResult {
        let selection_vec = |ctx: &ProgrammerContext| {
            let mut v: Vec<u32> = ctx.selection.iter().copied().collect();
            v.sort_unstable();
            v
        };

        let command = match command_line::parse(text) {
            Ok(c) => c,
            Err(e) => {
                let ctx = self.programmer_ctx.lock().unwrap();
                return CommandResult { ok: false, message: e, selection: selection_vec(&ctx) };
            }
        };

        let known: HashSet<u32> =
            self.patch_index.lock().unwrap().iter().map(|e| e.fixture_number).collect();
        let group_members = self.groups.lock().unwrap().fixture_numbers_by_id();
        let mut ctx = self.programmer_ctx.lock().unwrap();

        let (ok, message) = match command {
            Command::Select(expr) => match expr.resolve(&known, &group_members) {
                Ok(set) => {
                    ctx.selection = set;
                    (true, format!("Selected {} fixture(s)", ctx.selection.len()))
                }
                Err(e) => (false, e),
            },
            Command::SetIntensity(expr, value) => {
                let target = match expr {
                    // "1 THRU 8 @ 50" both selects and sets intensity, like a
                    // real console's command line.
                    Some(expr) => match expr.resolve(&known, &group_members) {
                        Ok(set) => {
                            ctx.selection = set.clone();
                            set
                        }
                        Err(e) => {
                            return CommandResult { ok: false, message: e, selection: selection_vec(&ctx) }
                        }
                    },
                    None => ctx.selection.clone(),
                };
                self.apply_intensity(&target, value.as_u8())
            }
            Command::Clear => {
                if ctx.selection.is_empty() {
                    self.state.lock().unwrap().programmer.values.clear();
                    (true, "Cleared programmer values".to_string())
                } else {
                    ctx.selection.clear();
                    (true, "Cleared selection".to_string())
                }
            }
            Command::Release => {
                ctx.selection.clear();
                self.state.lock().unwrap().programmer.values.clear();
                (true, "Released".to_string())
            }
            Command::SelectAll => {
                ctx.selection = known.clone();
                (true, format!("Selected all {} fixture(s)", ctx.selection.len()))
            }
            Command::Next | Command::Prev => match ctx.current_cuelist {
                None => (false, "No current cuelist — select one in the Cuelists tab first".to_string()),
                Some(cuelist_id) => match self.find_playback_for_cuelist(cuelist_id) {
                    None => (false, "No active playback for this cuelist — add one from the Cuelists tab".to_string()),
                    Some(playback_id) => {
                        let result = if command == Command::Next {
                            self.playback_go(playback_id)
                        } else {
                            self.playback_go_back(playback_id)
                        };
                        match result {
                            Ok(()) => (true, if command == Command::Next { "GO".to_string() } else { "Back".to_string() }),
                            Err(e) => (false, e),
                        }
                    }
                },
            },
            Command::SetTime(seconds) => {
                ctx.pending_time_s = seconds;
                (true, format!("Fade time set to {seconds}s (applies to the next RECORD CUE/UPDATE)"))
            }
            Command::RecordCue(number) => match ctx.current_cuelist {
                None => (false, "No current cuelist — select one in the Cuelists tab first".to_string()),
                Some(cuelist_id) => {
                    let number: f64 = number.into();
                    let fade_ms = (ctx.pending_time_s as f64) * 1000.0;
                    let selection = ctx.selection.clone();
                    match self.record_cue(cuelist_id, number, format!("Cue {number}"), &selection, fade_ms, fade_ms) {
                        Ok(()) => {
                            ctx.last_cue_number = Some(number);
                            (true, format!("Recorded cue {number}"))
                        }
                        Err(e) => (false, e),
                    }
                }
            },
            Command::UpdateCue => match (ctx.current_cuelist, ctx.last_cue_number) {
                (Some(cuelist_id), Some(number)) => {
                    let name = self
                        .cues
                        .lock()
                        .unwrap()
                        .get(cuelist_id)
                        .and_then(|list| list.cues.iter().find(|c| (c.number - number).abs() < f64::EPSILON))
                        .map(|c| c.name.clone())
                        .unwrap_or_else(|| format!("Cue {number}"));
                    let fade_ms = (ctx.pending_time_s as f64) * 1000.0;
                    let selection = ctx.selection.clone();
                    match self.record_cue(cuelist_id, number, name, &selection, fade_ms, fade_ms) {
                        Ok(()) => (true, format!("Updated cue {number}")),
                        Err(e) => (false, e),
                    }
                }
                _ => (false, "No cue to update yet — RECORD CUE first".to_string()),
            },
            Command::DeleteCue(number) => match ctx.current_cuelist {
                None => (false, "No current cuelist — select one in the Cuelists tab first".to_string()),
                Some(cuelist_id) => match self.delete_cue(cuelist_id, number.into()) {
                    Ok(()) => (true, format!("Deleted cue {number}")),
                    Err(e) => (false, e),
                },
            },
            // COPY/MOVE parse (and are unit-tested) but take no arguments in
            // this project's grammar (see command_line.rs's module docs) --
            // there's no target number to copy/move to. Rather than guess at
            // syntax BUILD_PLAN doesn't specify, they stay unimplemented; see
            // DECISIONS.md.
            Command::Copy | Command::Move => {
                (false, "COPY/MOVE need a target cue number the command line doesn't have syntax for yet".to_string())
            }
        };

        CommandResult { ok, message, selection: selection_vec(&ctx) }
    }

    fn apply_intensity(&self, fixture_numbers: &HashSet<u32>, value: u8) -> (bool, String) {
        let index = self.patch_index.lock().unwrap();
        let mut state = self.state.lock().unwrap();
        let mut set_count = 0;
        let mut skipped = 0;
        for &number in fixture_numbers {
            match index.iter().find(|e| e.fixture_number == number) {
                Some(entry) => match entry.intensity_channel {
                    Some(channel) => {
                        state.programmer.values.insert((entry.universe, channel), value as f32);
                        set_count += 1;
                    }
                    None => skipped += 1,
                },
                None => skipped += 1,
            }
        }
        let message = if skipped == 0 {
            format!("Set intensity on {set_count} fixture(s)")
        } else {
            format!("Set intensity on {set_count} fixture(s), skipped {skipped} (no dimmer channel or unpatched)")
        };
        (set_count > 0 || fixture_numbers.is_empty(), message)
    }

    // --- Groups (Phase 5) ---

    pub fn record_group(&self, name: String, fixture_numbers: Vec<u32>) -> u32 {
        self.groups.lock().unwrap().record(name, fixture_numbers)
    }

    pub fn list_groups(&self) -> Vec<Group> {
        self.groups.lock().unwrap().list()
    }

    pub fn rename_group(&self, id: u32, name: String) -> Result<(), String> {
        self.groups.lock().unwrap().rename(id, name)
    }

    pub fn delete_group(&self, id: u32) {
        self.groups.lock().unwrap().delete(id);
    }

    /// A "group master": sets every member's intensity to `percent` (0-100),
    /// the same operation as the command line's `GROUP N @ value`. Not a
    /// continuously-multiplying scaling layer over whatever else is live —
    /// see DECISIONS.md for why that's out of scope here.
    pub fn apply_group_master(&self, id: u32, percent: f32) -> Result<(bool, String), String> {
        let members: HashSet<u32> = self
            .groups
            .lock()
            .unwrap()
            .get(id)
            .ok_or_else(|| format!("No group {id}"))?
            .fixture_numbers
            .iter()
            .copied()
            .collect();
        let value = (percent.clamp(0.0, 100.0) / 100.0 * 255.0).round() as u8;
        Ok(self.apply_intensity(&members, value))
    }

    // --- Presets (Phase 5) ---

    /// Reads each target fixture's *currently rendered* (post-merge) value
    /// for every attribute it has, so recording a preset captures what you
    /// actually see, not just whatever the programmer layer happens to hold.
    fn capture_attribute_values(&self, fixture_numbers: &HashSet<u32>) -> HashMap<String, f32> {
        let index = self.patch_index.lock().unwrap();
        let state = self.state.lock().unwrap();
        let mut frames: HashMap<u32, [u8; UNIVERSE_SIZE]> = HashMap::new();
        let mut values: HashMap<String, f32> = HashMap::new();

        let mut entries: Vec<&PatchIndexEntry> =
            index.iter().filter(|e| fixture_numbers.contains(&e.fixture_number)).collect();
        entries.sort_by_key(|e| e.fixture_number);

        for entry in entries {
            let frame = frames.entry(entry.universe).or_insert_with(|| merge_universe(&state, entry.universe));
            for (attr, &channel) in &entry.attribute_channels {
                values.entry(attr.clone()).or_insert(frame[(channel - 1) as usize] as f32);
            }
        }
        values
    }

    pub fn record_preset(
        &self,
        family: PresetFamily,
        name: String,
        fixture_numbers: &HashSet<u32>,
        color: Option<String>,
    ) -> u32 {
        let values = self.capture_attribute_values(fixture_numbers);
        self.presets.lock().unwrap().record(family, name, values, color)
    }

    pub fn update_preset(&self, id: u32, fixture_numbers: &HashSet<u32>, color: Option<String>) -> Result<(), String> {
        let values = self.capture_attribute_values(fixture_numbers);
        self.presets.lock().unwrap().update(id, values, color)
    }

    pub fn rename_preset(&self, id: u32, name: String) -> Result<(), String> {
        self.presets.lock().unwrap().rename(id, name)
    }

    pub fn delete_preset(&self, id: u32) {
        self.presets.lock().unwrap().delete(id);
    }

    pub fn list_presets(&self, family: Option<PresetFamily>) -> Vec<Preset> {
        self.presets.lock().unwrap().list(family)
    }

    /// Applies preset `id` to every fixture in `fixture_numbers` that has a
    /// channel for one of its attributes. Always looks the preset up fresh by
    /// id — never caches/copies its values — so updating a preset changes
    /// what every future apply (including, once Phase 6 exists, a recorded
    /// cue that references it) actually does.
    pub fn apply_preset(&self, id: u32, fixture_numbers: &HashSet<u32>) -> Result<usize, String> {
        let values = {
            let presets = self.presets.lock().unwrap();
            presets.get(id).ok_or_else(|| format!("No preset {id}"))?.values.clone()
        };
        let index = self.patch_index.lock().unwrap();
        let mut state = self.state.lock().unwrap();
        let mut applied = 0;
        for entry in index.iter().filter(|e| fixture_numbers.contains(&e.fixture_number)) {
            for (attr, &value) in &values {
                if let Some(&channel) = entry.attribute_channels.get(attr) {
                    state.programmer.values.insert((entry.universe, channel), value);
                    applied += 1;
                }
            }
        }
        Ok(applied)
    }

    // --- Cuelists and cues (Phase 6) ---

    pub fn create_cuelist(&self, name: String, kind: CuelistKind, tracking: bool) -> u32 {
        self.cues.lock().unwrap().create(name, kind, tracking)
    }

    pub fn list_cuelists(&self) -> Vec<CuelistDto> {
        self.cues.lock().unwrap().list().iter().map(CuelistDto::from).collect()
    }

    pub fn delete_cuelist(&self, id: u32) {
        self.cues.lock().unwrap().delete(id);
        // Any playback still pointing at it stops contributing next tick
        // (its `cues.get(cuelist_id)` lookup will miss and its layer is
        // dropped), but explicitly drop the runtimes too so `list_playbacks`
        // doesn't keep showing them.
        self.cue_playbacks.lock().unwrap().retain(|p| p.cuelist_id != id);
    }

    pub fn rename_cuelist(&self, id: u32, name: String) -> Result<(), String> {
        let mut cues = self.cues.lock().unwrap();
        let list = cues.get_mut(id).ok_or_else(|| format!("No cuelist {id}"))?;
        list.name = name;
        Ok(())
    }

    pub fn set_cuelist_tracking(&self, id: u32, tracking: bool) -> Result<(), String> {
        let mut cues = self.cues.lock().unwrap();
        let list = cues.get_mut(id).ok_or_else(|| format!("No cuelist {id}"))?;
        list.tracking = tracking;
        Ok(())
    }

    /// Sets which cuelist the command line's `RECORD CUE`/`UPDATE`/`DELETE
    /// CUE`/`NEXT`/`PREV` operate against — called when the Cuelists tab
    /// selects one. `None` clears it (e.g. the cuelist was deleted).
    pub fn set_current_cuelist(&self, id: Option<u32>) {
        self.programmer_ctx.lock().unwrap().current_cuelist = id;
    }

    /// Records a cue from `fixture_numbers`' currently-rendered (post-merge)
    /// values — the same "record what you see" approach as
    /// `record_preset`/`update_preset`. Every attribute channel those
    /// fixtures have becomes a `CueValue::Literal`; use
    /// `set_cue_preset_reference` afterwards to turn specific channels into
    /// live preset references instead.
    pub fn record_cue(
        &self,
        cuelist_id: u32,
        number: f64,
        name: String,
        fixture_numbers: &HashSet<u32>,
        fade_in_ms: f64,
        fade_out_ms: f64,
    ) -> Result<(), String> {
        let index = self.patch_index.lock().unwrap();
        let state = self.state.lock().unwrap();
        let mut frames: HashMap<u32, [u8; UNIVERSE_SIZE]> = HashMap::new();
        let mut cue = Cue::new(number, name);
        cue.fade_in_ms = fade_in_ms;
        cue.fade_out_ms = fade_out_ms;

        let mut entries: Vec<&PatchIndexEntry> =
            index.iter().filter(|e| fixture_numbers.contains(&e.fixture_number)).collect();
        entries.sort_by_key(|e| e.fixture_number);
        for entry in entries {
            let frame = frames.entry(entry.universe).or_insert_with(|| merge_universe(&state, entry.universe));
            for &channel in entry.attribute_channels.values() {
                cue.values.insert((entry.universe, channel), CueValue::Literal { value: frame[(channel - 1) as usize] as f32 });
            }
        }
        drop(state);
        drop(index);

        let mut cues = self.cues.lock().unwrap();
        let list = cues.get_mut(cuelist_id).ok_or_else(|| format!("No cuelist {cuelist_id}"))?;
        list.upsert_cue(cue);
        Ok(())
    }

    pub fn delete_cue(&self, cuelist_id: u32, number: f64) -> Result<(), String> {
        let mut cues = self.cues.lock().unwrap();
        let list = cues.get_mut(cuelist_id).ok_or_else(|| format!("No cuelist {cuelist_id}"))?;
        list.remove_cue(number);
        Ok(())
    }

    /// Turns one channel of an existing cue into a live reference to preset
    /// `preset_id`'s `attribute` value, instead of a literal — this is what
    /// makes "updating a preset changes every cue that references it" real
    /// for an actual recorded cue, not just the unit-tested mechanism.
    pub fn set_cue_preset_reference(
        &self,
        cuelist_id: u32,
        cue_number: f64,
        fixture_number: u32,
        attribute: String,
        preset_id: u32,
    ) -> Result<(), String> {
        let (universe, channel) = {
            let index = self.patch_index.lock().unwrap();
            let entry = index
                .iter()
                .find(|e| e.fixture_number == fixture_number)
                .ok_or_else(|| format!("No fixture {fixture_number}"))?;
            let channel = *entry
                .attribute_channels
                .get(&attribute)
                .ok_or_else(|| format!("Fixture {fixture_number} has no '{attribute}' channel"))?;
            (entry.universe, channel)
        };

        let mut cues = self.cues.lock().unwrap();
        let list = cues.get_mut(cuelist_id).ok_or_else(|| format!("No cuelist {cuelist_id}"))?;
        let index = list.index_of(cue_number).ok_or_else(|| format!("No cue {cue_number}"))?;
        list.cues[index].values.insert((universe, channel), CueValue::Preset { preset_id, attribute });
        Ok(())
    }

    pub fn set_chase_tempo(&self, cuelist_id: u32, bpm: f32) -> Result<(), String> {
        let mut cues = self.cues.lock().unwrap();
        let list = cues.get_mut(cuelist_id).ok_or_else(|| format!("No cuelist {cuelist_id}"))?;
        match &mut list.kind {
            CuelistKind::Chase { bpm: current, .. } => {
                *current = bpm;
                Ok(())
            }
            _ => Err("Not a chase cuelist".to_string()),
        }
    }

    // --- Playbacks (Phase 6) ---

    pub fn create_playback(&self, cuelist_id: u32) -> Result<u32, String> {
        let kind = self.cues.lock().unwrap().get(cuelist_id).ok_or_else(|| format!("No cuelist {cuelist_id}"))?.kind;
        let base_priority = self.state.lock().unwrap().next_priority();
        let priority = if kind == CuelistKind::Override { OVERRIDE_PRIORITY_BASE + base_priority } else { base_priority };

        let id = self.next_playback_id.fetch_add(1, Ordering::SeqCst);
        let mut runtime = PlaybackRuntime::new(id, cuelist_id, priority);
        if kind == CuelistKind::Submaster {
            // A submaster has no GO stepping -- it always renders cue 0
            // (tracked), scaled by its fader.
            let cues = self.cues.lock().unwrap();
            let presets = self.presets.lock().unwrap();
            if let Some(list) = cues.get(cuelist_id) {
                if !list.cues.is_empty() {
                    runtime.go(list, &presets, 0);
                }
            }
        }
        self.cue_playbacks.lock().unwrap().push(runtime);
        Ok(id)
    }

    pub fn delete_playback(&self, id: u32) {
        self.cue_playbacks.lock().unwrap().retain(|p| p.id != id);
        self.state.lock().unwrap().playbacks.remove(&format!("cue-playback-{id}"));
    }

    pub fn list_playbacks(&self) -> Vec<PlaybackStatus> {
        let cues = self.cues.lock().unwrap();
        self.cue_playbacks
            .lock()
            .unwrap()
            .iter()
            .map(|pb| PlaybackStatus {
                id: pb.id,
                cuelist_id: pb.cuelist_id,
                current_cue_number: pb
                    .current_index
                    .and_then(|i| cues.get(pb.cuelist_id).and_then(|list| list.cues.get(i)))
                    .map(|c| c.number),
                fader: pb.fader,
                fader_mode: pb.fader_mode,
                paused: pb.paused,
            })
            .collect()
    }

    /// The first playback slot attached to `cuelist_id`, if any — what the
    /// command line's `NEXT`/`PREV` step, since the command line has no
    /// concept of playback ids, only of "the current cuelist".
    fn find_playback_for_cuelist(&self, cuelist_id: u32) -> Option<u32> {
        self.cue_playbacks.lock().unwrap().iter().find(|p| p.cuelist_id == cuelist_id).map(|p| p.id)
    }

    fn with_playback<R>(&self, id: u32, f: impl FnOnce(&mut PlaybackRuntime, &Cuelist, &PresetStore) -> R) -> Result<R, String> {
        let mut playbacks = self.cue_playbacks.lock().unwrap();
        let pb = playbacks.iter_mut().find(|p| p.id == id).ok_or_else(|| format!("No playback {id}"))?;
        let cues = self.cues.lock().unwrap();
        let list = cues.get(pb.cuelist_id).ok_or_else(|| format!("Cuelist {} is missing", pb.cuelist_id))?;
        let presets = self.presets.lock().unwrap();
        Ok(f(pb, list, &presets))
    }

    pub fn playback_go(&self, id: u32) -> Result<(), String> {
        self.with_playback(id, |pb, list, presets| pb.go_next(list, presets))
    }

    pub fn playback_go_back(&self, id: u32) -> Result<(), String> {
        self.with_playback(id, |pb, list, presets| pb.go_back(list, presets))
    }

    pub fn playback_release(&self, id: u32) -> Result<(), String> {
        self.with_playback(id, |pb, _list, _presets| pb.release(DEFAULT_RELEASE_FADE_MS))
    }

    pub fn playback_set_paused(&self, id: u32, paused: bool) -> Result<(), String> {
        let mut playbacks = self.cue_playbacks.lock().unwrap();
        let pb = playbacks.iter_mut().find(|p| p.id == id).ok_or_else(|| format!("No playback {id}"))?;
        pb.paused = paused;
        Ok(())
    }

    pub fn playback_set_fader(&self, id: u32, percent: f32) -> Result<(), String> {
        let mut playbacks = self.cue_playbacks.lock().unwrap();
        let pb = playbacks.iter_mut().find(|p| p.id == id).ok_or_else(|| format!("No playback {id}"))?;
        pb.fader = (percent.clamp(0.0, 100.0)) / 100.0;
        Ok(())
    }

    pub fn playback_set_fader_mode(&self, id: u32, mode: FaderMode) -> Result<(), String> {
        let mut playbacks = self.cue_playbacks.lock().unwrap();
        let pb = playbacks.iter_mut().find(|p| p.id == id).ok_or_else(|| format!("No playback {id}"))?;
        pb.fader_mode = mode;
        Ok(())
    }

    // --- Effects (Phase 7) ---

    pub fn create_effect(&self, input: EffectConfigInput) -> u32 {
        self.effects.lock().unwrap().record(
            input.name,
            input.waveform,
            input.attribute,
            input.size_percent,
            input.rate,
            input.phase_spread_deg,
            input.width,
            input.direction,
            input.block_size,
            input.mirror,
            input.mode,
        )
    }

    pub fn list_effects(&self) -> Vec<EffectConfig> {
        self.effects.lock().unwrap().list()
    }

    pub fn rename_effect(&self, id: u32, name: String) -> Result<(), String> {
        self.effects.lock().unwrap().rename(id, name)
    }

    pub fn delete_effect(&self, id: u32) {
        self.effects.lock().unwrap().delete(id);
        self.effect_runtimes.lock().unwrap().retain(|r| r.runtime.effect_id != id);
    }

    /// Starts a running instance of `effect_id` across `fixture_numbers`,
    /// resolving each fixture's channel for the effect's attribute once (in
    /// the given order — that order is what the phase-spread fan indexes
    /// against). For `EffectMode::Relative`, also captures each target's
    /// current rendered value to ride on top of — the same "record what you
    /// see" moment `record_cue`/`capture_attribute_values` use.
    pub fn start_effect(&self, effect_id: u32, fixture_numbers: Vec<u32>) -> Result<u32, String> {
        let config = self.effects.lock().unwrap().get(effect_id).cloned().ok_or_else(|| format!("No effect {effect_id}"))?;
        let index = self.patch_index.lock().unwrap();
        let targets: Vec<(u32, u16)> = fixture_numbers
            .iter()
            .filter_map(|n| {
                index
                    .iter()
                    .find(|e| e.fixture_number == *n)
                    .and_then(|e| e.attribute_channels.get(&config.attribute).map(|&ch| (e.universe, ch)))
            })
            .collect();
        drop(index);
        if targets.is_empty() {
            return Err(format!("None of the selected fixtures have a '{}' channel", config.attribute));
        }

        let priority = self.state.lock().unwrap().next_priority();
        let id = self.next_effect_runtime_id.fetch_add(1, Ordering::SeqCst);
        let mut runtime = EffectRuntime::new(id, effect_id, priority);
        if config.mode == EffectMode::Relative {
            let state = self.state.lock().unwrap();
            let mut frames: HashMap<u32, [u8; UNIVERSE_SIZE]> = HashMap::new();
            let current: HashMap<(u32, u16), f32> = targets
                .iter()
                .map(|&(universe, channel)| {
                    let frame = frames.entry(universe).or_insert_with(|| merge_universe(&state, universe));
                    ((universe, channel), frame[(channel - 1) as usize] as f32)
                })
                .collect();
            drop(state);
            runtime.start(&targets, &current);
        }
        self.effect_runtimes.lock().unwrap().push(RunningEffect { runtime, targets });
        Ok(id)
    }

    pub fn stop_effect(&self, id: u32) {
        self.effect_runtimes.lock().unwrap().retain(|r| r.runtime.id != id);
        self.state.lock().unwrap().playbacks.remove(&format!("effect-{id}"));
    }

    pub fn list_running_effects(&self) -> Vec<EffectRunStatus> {
        self.effect_runtimes
            .lock()
            .unwrap()
            .iter()
            .map(|r| EffectRunStatus { id: r.runtime.id, effect_id: r.runtime.effect_id })
            .collect()
    }

    // --- Pixel maps (Phase 7) ---

    pub fn create_pixelmap(&self, name: String, width: u32, height: u32) -> u32 {
        self.pixelmaps.lock().unwrap().create(name, width, height)
    }

    pub fn list_pixelmaps(&self) -> Vec<PixelMap> {
        self.pixelmaps.lock().unwrap().list()
    }

    pub fn delete_pixelmap(&self, id: u32) {
        self.pixelmaps.lock().unwrap().delete(id);
        self.pixelmap_runtimes.lock().unwrap().retain(|r| r.runtime.pixelmap_id != id);
    }

    pub fn rename_pixelmap(&self, id: u32, name: String) -> Result<(), String> {
        let mut maps = self.pixelmaps.lock().unwrap();
        let map = maps.get_mut(id).ok_or_else(|| format!("No pixel map {id}"))?;
        map.name = name;
        Ok(())
    }

    pub fn set_pixelmap_cell(&self, id: u32, x: u32, y: u32, fixture_number: Option<u32>) -> Result<(), String> {
        let mut maps = self.pixelmaps.lock().unwrap();
        let map = maps.get_mut(id).ok_or_else(|| format!("No pixel map {id}"))?;
        map.set_cell(x, y, fixture_number)
    }

    pub fn set_pixelmap_generator(&self, id: u32, generator: PixelGenerator) -> Result<(), String> {
        let mut maps = self.pixelmaps.lock().unwrap();
        let map = maps.get_mut(id).ok_or_else(|| format!("No pixel map {id}"))?;
        map.generator = generator;
        Ok(())
    }

    /// Starts running `pixelmap_id`, resolving every cell's fixture to its
    /// RGB (or dimmer-only) channels once up front — the same "resolve at
    /// start, not every tick" approach `start_effect` uses.
    pub fn start_pixelmap(&self, pixelmap_id: u32) -> Result<u32, String> {
        let map = self.pixelmaps.lock().unwrap().get(pixelmap_id).cloned().ok_or_else(|| format!("No pixel map {pixelmap_id}"))?;
        let index = self.patch_index.lock().unwrap();
        let cell_targets: Vec<Option<CellTarget>> = map
            .cells
            .iter()
            .map(|cell| {
                let fixture_number = (*cell)?;
                let entry = index.iter().find(|e| e.fixture_number == fixture_number)?;
                Some(CellTarget {
                    universe: entry.universe,
                    red: entry.attribute_channels.get("red").copied(),
                    green: entry.attribute_channels.get("green").copied(),
                    blue: entry.attribute_channels.get("blue").copied(),
                    dimmer: entry.attribute_channels.get("dimmer").copied(),
                })
            })
            .collect();
        drop(index);

        let priority = self.state.lock().unwrap().next_priority();
        let id = self.next_pixelmap_runtime_id.fetch_add(1, Ordering::SeqCst);
        let runtime = PixelMapRuntime::new(id, pixelmap_id, priority);
        self.pixelmap_runtimes.lock().unwrap().push(RunningPixelMap { runtime, cell_targets });
        Ok(id)
    }

    pub fn stop_pixelmap(&self, id: u32) {
        self.pixelmap_runtimes.lock().unwrap().retain(|r| r.runtime.id != id);
        self.state.lock().unwrap().playbacks.remove(&format!("pixelmap-{id}"));
    }

    pub fn list_running_pixelmaps(&self) -> Vec<PixelMapRunStatus> {
        self.pixelmap_runtimes
            .lock()
            .unwrap()
            .iter()
            .map(|r| PixelMapRunStatus { id: r.runtime.id, pixelmap_id: r.runtime.pixelmap_id })
            .collect()
    }
}

impl Default for EngineManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-dimmer-channel fixture. Mirrors how `useShowStore.ts`'s
    /// `syncEnginePatch` actually populates `PatchIndexEntry` in production:
    /// a dimmer channel sets both `intensity_channel` *and* an
    /// `attribute_channels["dimmer"]` entry (it falls through the "first
    /// occurrence of each attribute wins" logic same as every other
    /// channel type) — a cue/preset capture reads only `attribute_channels`,
    /// so a fixture with no non-dimmer attributes still needs this to have
    /// anything to record.
    fn patched_fixture(number: u32, universe: u32, intensity_channel: u16) -> PatchIndexEntry {
        PatchIndexEntry {
            fixture_number: number,
            universe,
            intensity_channel: Some(intensity_channel),
            attribute_channels: [("dimmer".to_string(), intensity_channel)].into_iter().collect(),
        }
    }

    #[test]
    fn record_cue_via_command_line_captures_the_current_selection() {
        let engine = EngineManager::new();
        engine.set_patch_index(vec![patched_fixture(1, 1, 1)]);
        let cuelist_id = engine.create_cuelist("List".to_string(), CuelistKind::Standard, true);
        engine.set_current_cuelist(Some(cuelist_id));

        assert!(engine.execute_command("1 @ 100").ok);
        let result = engine.execute_command("RECORD CUE 1");
        assert!(result.ok, "{}", result.message);

        let lists = engine.list_cuelists();
        let cue = &lists[0].cues[0];
        assert_eq!(cue.number, 1.0);
        assert_eq!(cue.values[0].channel, 1);
        assert!(matches!(cue.values[0].value, CueValue::Literal { value } if value == 255.0));
    }

    #[test]
    fn record_cue_without_a_current_cuelist_fails() {
        let engine = EngineManager::new();
        engine.set_patch_index(vec![patched_fixture(1, 1, 1)]);
        let result = engine.execute_command("RECORD CUE 1");
        assert!(!result.ok);
    }

    #[test]
    fn update_re_captures_the_last_recorded_cue_and_delete_removes_it() {
        let engine = EngineManager::new();
        engine.set_patch_index(vec![patched_fixture(1, 1, 1)]);
        let cuelist_id = engine.create_cuelist("List".to_string(), CuelistKind::Standard, true);
        engine.set_current_cuelist(Some(cuelist_id));

        engine.execute_command("1 @ 50");
        assert!(engine.execute_command("RECORD CUE 1").ok);

        engine.execute_command("1 @ 100");
        let update = engine.execute_command("UPDATE");
        assert!(update.ok, "{}", update.message);
        let after_update = engine.list_cuelists();
        assert!(matches!(
            after_update[0].cues[0].values[0].value,
            CueValue::Literal { value } if value == 255.0
        ));

        let delete = engine.execute_command("DELETE CUE 1");
        assert!(delete.ok, "{}", delete.message);
        assert!(engine.list_cuelists()[0].cues.is_empty());
    }

    #[test]
    fn next_and_prev_step_the_playback_attached_to_the_current_cuelist() {
        let engine = EngineManager::new();
        engine.set_patch_index(vec![patched_fixture(1, 1, 1)]);
        let cuelist_id = engine.create_cuelist("List".to_string(), CuelistKind::Standard, true);
        let fixtures: HashSet<u32> = [1].into_iter().collect();
        engine.record_cue(cuelist_id, 1.0, "Cue 1".to_string(), &fixtures, 1.0, 1.0).unwrap();
        engine.record_cue(cuelist_id, 2.0, "Cue 2".to_string(), &fixtures, 1.0, 1.0).unwrap();
        engine.create_playback(cuelist_id).unwrap();
        engine.set_current_cuelist(Some(cuelist_id));

        assert!(engine.execute_command("NEXT").ok);
        assert_eq!(engine.list_playbacks()[0].current_cue_number, Some(1.0));

        assert!(engine.execute_command("NEXT").ok);
        assert_eq!(engine.list_playbacks()[0].current_cue_number, Some(2.0));

        assert!(engine.execute_command("PREV").ok);
        assert_eq!(engine.list_playbacks()[0].current_cue_number, Some(1.0));
    }

    #[test]
    fn next_without_an_attached_playback_fails_with_a_clear_message() {
        let engine = EngineManager::new();
        let cuelist_id = engine.create_cuelist("List".to_string(), CuelistKind::Standard, true);
        engine.set_current_cuelist(Some(cuelist_id));
        let result = engine.execute_command("NEXT");
        assert!(!result.ok);
    }
}
