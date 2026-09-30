//! Continuous effect generators (BUILD_PLAN Phase 7): a waveform applied to
//! one attribute across a fixture selection, with a phase-spread "fan"
//! across the fixtures, an optional block/grouping size, and a "wings"
//! mirror mode. An `EffectConfig` is a named, reusable template (stored in
//! `EffectStore`, the same CRUD shape as `groups::GroupStore`); an
//! `EffectRuntime` is one running instance targeting a concrete, ordered
//! list of (universe, channel) pairs — the same template/instance split
//! Phase 6's `Cuelist`/`PlaybackRuntime` uses.
//!
//! Deliberately decoupled from `PatchIndexEntry`/fixture resolution (which
//! stays `manager.rs`'s job, same as `record_cue`/`capture_attribute_values`
//! do it there): `EffectRuntime::render` takes an already-resolved,
//! fixture-ordered `&[(u32, u16)]` rather than reaching into the patch
//! itself, so this module stays unit-testable without any Tauri/manager
//! wiring, matching `cues.rs`/`playback.rs`'s own independence.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/showfile/generated/")]
pub enum Waveform {
    Sine,
    Square,
    /// Ramps up linearly (0 -> 1) then snaps back to 0.
    Ramp,
    /// Ramps down linearly (1 -> 0) then snaps back to 1 — the mirror image
    /// of `Ramp`, giving BUILD_PLAN's "saw" and "ramp" distinct shapes.
    Saw,
    /// A new pseudo-random value once per full cycle (deterministic from the
    /// cycle index, not real randomness, so a recorded effect replays
    /// identically).
    Random,
    /// A staircase: `width` sets each step's width as a fraction of a full
    /// cycle (e.g. 0.25 -> 4 steps).
    Step,
    /// A short triangular spike near the start of each cycle, `width` wide.
    Pulse,
}

impl Waveform {
    /// `raw_phase` is unwrapped (can be any real number, growing over time);
    /// `width` is 0.0-1.0 and only matters for Square/Step/Pulse. Returns a
    /// value in 0.0-1.0.
    fn sample(self, raw_phase: f32, width: f32) -> f32 {
        let width = width.clamp(0.001, 1.0);
        let cycle = raw_phase.floor();
        let phase = raw_phase - cycle;
        match self {
            Waveform::Sine => (1.0 - (phase * std::f32::consts::TAU).cos()) / 2.0,
            Waveform::Square => {
                if phase < width {
                    1.0
                } else {
                    0.0
                }
            }
            Waveform::Ramp => phase,
            Waveform::Saw => 1.0 - phase,
            Waveform::Random => pseudo_random(cycle as i64),
            Waveform::Step => {
                let steps = (1.0 / width).round().max(1.0);
                (phase * steps).floor() / (steps - 1.0).max(1.0)
            }
            Waveform::Pulse => {
                if phase >= width {
                    0.0
                } else {
                    let half = width / 2.0;
                    if phase < half {
                        phase / half
                    } else {
                        (width - phase) / half
                    }
                }
            }
        }
    }
}

/// A cheap deterministic hash -> `[0.0, 1.0)`, so `Waveform::Random` is
/// reproducible (same cycle index always gives the same value) instead of
/// depending on wall-clock entropy. `pub(super)` so `pixelmap.rs`'s Noise
/// generator can reuse it instead of a second copy of the same hash.
pub(super) fn pseudo_random(seed: i64) -> f32 {
    let mut x = (seed as u64).wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(1);
    x ^= x >> 33;
    x = x.wrapping_mul(0xFF51AFD7ED558CCD);
    x ^= x >> 33;
    (x % 1_000_000) as f32 / 1_000_000.0
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/showfile/generated/")]
pub enum EffectRate {
    Hz(f32),
    /// BPM-synced: one full cycle every `beats_per_cycle` beats at `bpm`.
    Bpm { bpm: f32, beats_per_cycle: f32 },
}

impl EffectRate {
    fn cycles_per_ms(self) -> f64 {
        match self {
            EffectRate::Hz(hz) => hz as f64 / 1000.0,
            EffectRate::Bpm { bpm, beats_per_cycle } => {
                let beat_ms = 60_000.0 / bpm.max(0.001) as f64;
                1.0 / (beat_ms * beats_per_cycle.max(0.001) as f64)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/showfile/generated/")]
pub enum EffectDirection {
    Forward,
    Reverse,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/showfile/generated/")]
pub enum EffectMode {
    /// The waveform directly sets the channel: `sample * size%`.
    Absolute,
    /// The waveform adds +/- around whatever the channel's value was when
    /// the effect started (captured once, "record what you see", same
    /// pattern as `capture_attribute_values`): `base + (sample - 0.5) *
    /// size%`.
    Relative,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/showfile/generated/")]
pub struct EffectConfig {
    pub id: u32,
    pub name: String,
    pub waveform: Waveform,
    /// Attribute name (e.g. "dimmer", "red", "pan") — resolved to a channel
    /// per target fixture by the caller, same as a preset's attribute.
    pub attribute: String,
    /// 0.0-100.0.
    pub size_percent: f32,
    pub rate: EffectRate,
    /// How far around the cycle (0.0-360.0 degrees) the *last* fixture in
    /// the target list is offset from the first — BUILD_PLAN's "phase
    /// spread across selection (fan)".
    pub phase_spread_deg: f32,
    /// Square/Step/Pulse-specific; ignored by Sine/Ramp/Saw/Random.
    pub width: f32,
    pub direction: EffectDirection,
    /// Fixtures per phase step (BUILD_PLAN's "groups/blocks"): 1 = every
    /// fixture gets its own phase slot; N = fixtures are grouped into
    /// blocks of N that share a phase slot.
    pub block_size: u32,
    /// BUILD_PLAN's "wings": fan the phase out symmetrically from the
    /// center of the target list instead of linearly end-to-end.
    pub mirror: bool,
    pub mode: EffectMode,
}

#[derive(Default)]
pub struct EffectStore {
    effects: HashMap<u32, EffectConfig>,
    next_id: u32,
}

impl EffectStore {
    pub fn new() -> Self {
        Self::default()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record(&mut self, name: String, waveform: Waveform, attribute: String, size_percent: f32, rate: EffectRate, phase_spread_deg: f32, width: f32, direction: EffectDirection, block_size: u32, mirror: bool, mode: EffectMode) -> u32 {
        self.next_id += 1;
        let id = self.next_id;
        self.effects.insert(
            id,
            EffectConfig {
                id,
                name,
                waveform,
                attribute,
                size_percent,
                rate,
                phase_spread_deg,
                width,
                direction,
                block_size: block_size.max(1),
                mirror,
                mode,
            },
        );
        id
    }

    pub fn get(&self, id: u32) -> Option<&EffectConfig> {
        self.effects.get(&id)
    }

    pub fn rename(&mut self, id: u32, name: String) -> Result<(), String> {
        let effect = self.effects.get_mut(&id).ok_or_else(|| format!("No effect {id}"))?;
        effect.name = name;
        Ok(())
    }

    pub fn delete(&mut self, id: u32) {
        self.effects.remove(&id);
    }

    pub fn list(&self) -> Vec<EffectConfig> {
        let mut effects: Vec<EffectConfig> = self.effects.values().cloned().collect();
        effects.sort_by_key(|e| e.id);
        effects
    }
}

/// One running instance of an `EffectConfig`, targeting a concrete ordered
/// list of (universe, channel) pairs (already resolved from a fixture
/// selection by the caller).
pub struct EffectRuntime {
    pub id: u32,
    pub effect_id: u32,
    pub priority: i64,
    elapsed_ms: f64,
    /// Captured once at `start`, for `EffectMode::Relative`.
    base_values: HashMap<(u32, u16), f32>,
}

impl EffectRuntime {
    pub fn new(id: u32, effect_id: u32, priority: i64) -> Self {
        Self { id, effect_id, priority, elapsed_ms: 0.0, base_values: HashMap::new() }
    }

    /// Captures each target's current value for `EffectMode::Relative` to
    /// ride on top of. `current_state` is the caller's already-merged
    /// per-channel state (e.g. from `merge_universe`), same source
    /// `record_cue` reads from.
    pub fn start(&mut self, targets: &[(u32, u16)], current_state: &HashMap<(u32, u16), f32>) {
        self.base_values = targets.iter().map(|&key| (key, current_state.get(&key).copied().unwrap_or(0.0))).collect();
    }

    pub fn tick(&mut self, dt_ms: f64) {
        self.elapsed_ms += dt_ms;
    }

    /// This tick's contribution for every target channel, in the same
    /// fixture order the caller resolved `targets` in (order determines each
    /// fixture's phase-fan slot).
    pub fn render(&self, config: &EffectConfig, targets: &[(u32, u16)]) -> HashMap<(u32, u16), f32> {
        let block_size = config.block_size.max(1) as usize;
        let slot_count = targets.len().div_ceil(block_size).max(1);
        let direction_sign: f64 = if config.direction == EffectDirection::Forward { 1.0 } else { -1.0 };
        let cycles_per_ms = config.rate.cycles_per_ms() * direction_sign;

        targets
            .iter()
            .enumerate()
            .map(|(index, &key)| {
                let slot = index / block_size;
                let fan_position = if config.mirror {
                    let center = (slot_count.saturating_sub(1)) as f32 / 2.0;
                    (slot as f32 - center).abs()
                } else {
                    slot as f32
                };
                let max_fan = if config.mirror {
                    ((slot_count.saturating_sub(1)) as f32 / 2.0).max(1.0)
                } else {
                    (slot_count.saturating_sub(1)).max(1) as f32
                };
                let phase_offset = (fan_position / max_fan) * (config.phase_spread_deg / 360.0);
                let raw_phase = (self.elapsed_ms * cycles_per_ms) as f32 + phase_offset;
                let sample = config.waveform.sample(raw_phase, config.width);

                let value = match config.mode {
                    EffectMode::Absolute => sample * 255.0 * (config.size_percent / 100.0),
                    EffectMode::Relative => {
                        let base = self.base_values.get(&key).copied().unwrap_or(0.0);
                        base + (sample - 0.5) * 255.0 * (config.size_percent / 100.0)
                    }
                };
                (key, value.clamp(0.0, 255.0))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(waveform: Waveform, size_percent: f32, phase_spread_deg: f32, block_size: u32, mirror: bool, mode: EffectMode) -> EffectConfig {
        EffectConfig {
            id: 1,
            name: "Test".to_string(),
            waveform,
            attribute: "dimmer".to_string(),
            size_percent,
            rate: EffectRate::Hz(1.0),
            phase_spread_deg,
            width: 0.5,
            direction: EffectDirection::Forward,
            block_size,
            mirror,
            mode,
        }
    }

    #[test]
    fn sine_peaks_at_half_cycle_and_troughs_at_the_ends() {
        assert_eq!(Waveform::Sine.sample(0.0, 0.5), 0.0);
        assert!((Waveform::Sine.sample(0.25, 0.5) - 0.5).abs() < 0.01); // quarter cycle = midpoint
        assert!((Waveform::Sine.sample(0.5, 0.5) - 1.0).abs() < 0.01); // half cycle = peak
    }

    #[test]
    fn ramp_and_saw_are_mirror_images() {
        assert_eq!(Waveform::Ramp.sample(0.25, 0.5), 0.25);
        assert_eq!(Waveform::Saw.sample(0.25, 0.5), 0.75);
    }

    #[test]
    fn square_is_on_for_width_then_off() {
        assert_eq!(Waveform::Square.sample(0.1, 0.3), 1.0);
        assert_eq!(Waveform::Square.sample(0.5, 0.3), 0.0);
    }

    #[test]
    fn random_is_stable_within_a_cycle_and_usually_changes_between_cycles() {
        let a1 = Waveform::Random.sample(0.1, 0.5);
        let a2 = Waveform::Random.sample(0.9, 0.5);
        assert_eq!(a1, a2, "same cycle (0) must be stable");
        let b = Waveform::Random.sample(1.1, 0.5);
        assert_ne!(a1, b, "the next cycle should (almost always) differ");
    }

    #[test]
    fn step_quantizes_into_the_requested_number_of_levels() {
        // width 0.25 -> 4 steps: 0, 1/3, 2/3, 1.
        assert_eq!(Waveform::Step.sample(0.0, 0.25), 0.0);
        assert!((Waveform::Step.sample(0.99, 0.25) - 1.0).abs() < 0.01);
    }

    #[test]
    fn phase_spread_fans_evenly_across_the_target_list() {
        let cfg = config(Waveform::Ramp, 100.0, 180.0, 1, false, EffectMode::Absolute);
        let mut effect = EffectRuntime::new(1, 1, 1);
        let targets = vec![(1, 1), (1, 2), (1, 3)];
        // At t=0 the ramp's raw value at phase 0 is 0, but each fixture's
        // phase offset (0, 90, 180 degrees = 0, 0.25, 0.5 of a cycle) still
        // shows up in its own value.
        effect.start(&targets, &HashMap::new());
        let rendered = effect.render(&cfg, &targets);
        assert_eq!(rendered[&(1, 1)], 0.0);
        assert!((rendered[&(1, 2)] - 63.75).abs() < 1.0); // 0.25 * 255
        assert!((rendered[&(1, 3)] - 127.5).abs() < 1.0); // 0.5 * 255
    }

    #[test]
    fn block_size_groups_fixtures_onto_shared_phase_slots() {
        // 270deg, not 360: a full 360 spread aliases the last slot's phase
        // back onto the first when there are only two slots.
        let cfg = config(Waveform::Ramp, 100.0, 270.0, 2, false, EffectMode::Absolute);
        let effect = EffectRuntime::new(1, 1, 1);
        let targets = vec![(1, 1), (1, 2), (1, 3), (1, 4)];
        let rendered = effect.render(&cfg, &targets);
        // Fixtures 0-1 share slot 0, fixtures 2-3 share slot 1.
        assert_eq!(rendered[&(1, 1)], rendered[&(1, 2)]);
        assert_eq!(rendered[&(1, 3)], rendered[&(1, 4)]);
        assert_ne!(rendered[&(1, 1)], rendered[&(1, 3)]);
    }

    #[test]
    fn mirror_fans_symmetrically_from_the_center() {
        let cfg = config(Waveform::Ramp, 100.0, 270.0, 1, true, EffectMode::Absolute);
        let effect = EffectRuntime::new(1, 1, 1);
        let targets = vec![(1, 1), (1, 2), (1, 3)];
        let rendered = effect.render(&cfg, &targets);
        // The two outer fixtures (distance 1 from center) match each other;
        // the center fixture (distance 0) differs from both.
        assert_eq!(rendered[&(1, 1)], rendered[&(1, 3)]);
        assert_ne!(rendered[&(1, 1)], rendered[&(1, 2)]);
    }

    #[test]
    fn relative_mode_rides_on_top_of_the_captured_base_value() {
        let cfg = config(Waveform::Sine, 100.0, 0.0, 1, false, EffectMode::Relative);
        let mut effect = EffectRuntime::new(1, 1, 1);
        let targets = vec![(1, 1)];
        let mut base = HashMap::new();
        base.insert((1, 1), 100.0);
        effect.start(&targets, &base);
        // At phase 0, sine's sample is 0.0, so relative value = base + (0 - 0.5) * 255 = 100 - 127.5, clamped to 0.
        let rendered = effect.render(&cfg, &targets);
        assert_eq!(rendered[&(1, 1)], 0.0);
    }

    #[test]
    fn reverse_direction_negates_phase_progression() {
        let forward = config(Waveform::Ramp, 100.0, 0.0, 1, false, EffectMode::Absolute);
        let mut reverse = forward.clone();
        reverse.direction = EffectDirection::Reverse;

        let mut fwd_runtime = EffectRuntime::new(1, 1, 1);
        let mut rev_runtime = EffectRuntime::new(2, 1, 1);
        let targets = vec![(1, 1)];
        fwd_runtime.tick(250.0); // quarter second = quarter cycle at 1Hz
        rev_runtime.tick(250.0);

        let fwd = fwd_runtime.render(&forward, &targets)[&(1, 1)];
        let rev = rev_runtime.render(&reverse, &targets)[&(1, 1)];
        // Forward ramp a quarter cycle in is ~0.25; reverse runs phase
        // backwards, landing near 0.75 (wrapping).
        assert!((fwd - 63.75).abs() < 2.0);
        assert!((rev - 191.25).abs() < 2.0);
    }
}
