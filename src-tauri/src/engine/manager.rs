//! Ties the engine's pieces together as Tauri-managed state: live merge
//! state, running legacy-asset playbacks, the fixture-number index the
//! command line resolves against, and the 44Hz tick thread.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use ts_rs::TS;

use super::command_line::{self, Command};
use super::legacy_playback::{LegacyAsset, RunningPlayback};
use super::merge::merge_universe;
use super::state::EngineState;
use crate::output::manager::OutputManager;
use crate::output::UNIVERSE_SIZE;

const TICK_HZ: f64 = 44.0;
const DISPLAY_HZ: f64 = 20.0;

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
}

#[derive(Default)]
struct ProgrammerContext {
    selection: HashSet<u32>,
    /// CLEAR's first press clears the selection; a second press (selection
    /// already empty) clears the programmer's values instead.
    pending_time_s: f32,
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

pub struct EngineManager {
    state: Arc<Mutex<EngineState>>,
    playbacks: Arc<Mutex<Vec<RunningPlayback>>>,
    patch_index: Arc<Mutex<Vec<PatchIndexEntry>>>,
    programmer_ctx: Mutex<ProgrammerContext>,
    tick_started: AtomicBool,
}

impl EngineManager {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(EngineState::new())),
            playbacks: Arc::new(Mutex::new(Vec::new())),
            patch_index: Arc::new(Mutex::new(Vec::new())),
            programmer_ctx: Mutex::new(ProgrammerContext::default()),
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
        let mut ctx = self.programmer_ctx.lock().unwrap();

        let (ok, message) = match command {
            Command::Select(expr) => match expr.resolve(&known) {
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
                    Some(expr) => match expr.resolve(&known) {
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
            Command::Next | Command::Prev => {
                (false, "NEXT/PREV need a cuelist to step through (Phase 6)".to_string())
            }
            Command::SetTime(seconds) => {
                ctx.pending_time_s = seconds;
                (true, format!("Fade time set to {seconds}s (applies once cue recording exists — Phase 6)"))
            }
            Command::RecordCue(_) | Command::UpdateCue | Command::DeleteCue(_) | Command::Copy | Command::Move => {
                (false, "Cuelists aren't implemented yet (Phase 6)".to_string())
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
}

impl Default for EngineManager {
    fn default() -> Self {
        Self::new()
    }
}
