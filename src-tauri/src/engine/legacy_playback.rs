//! Frame-accurate playback of a pre-showfile Scene/Chase/FX asset, replacing
//! the old `src/lib/chaseEngine.ts` `setTimeout` loop (BUILD_PLAN Phase 3:
//! "retire chaseEngine.ts; replace it with a Rust playback that's frame-
//! accurate"). Each triggered asset becomes one playback layer in the engine,
//! advanced every engine tick instead of on its own JS timer — which also
//! means two triggered assets touching the same channel now go through the
//! same HTP/LTP merge as everything else, instead of silently fighting over
//! `applyPatch` calls the way the old runner did.

use std::collections::HashMap;

use super::state::Layer;

#[derive(Debug, Clone)]
pub struct LegacyStep {
    /// channel (1-512) -> target value (0-255).
    pub channels: HashMap<u16, u8>,
    pub fade_ms: f64,
    pub hold_ms: f64,
}

#[derive(Debug, Clone)]
pub struct LegacyAsset {
    pub steps: Vec<LegacyStep>,
    pub loop_: bool,
}

#[derive(Debug, PartialEq)]
enum Phase {
    Fading,
    Holding,
}

pub struct RunningPlayback {
    pub id: String,
    pub universe: u32,
    pub priority: i64,
    asset: LegacyAsset,
    step_index: usize,
    phase: Phase,
    elapsed_ms: f64,
    /// Values each channel in the current step held at the start of its fade,
    /// i.e. wherever the previous step (or nothing, for the first) left them.
    from: HashMap<u16, f32>,
}

impl RunningPlayback {
    pub fn new(id: String, universe: u32, priority: i64, asset: LegacyAsset) -> Self {
        Self {
            id,
            universe,
            priority,
            asset,
            step_index: 0,
            phase: Phase::Fading,
            elapsed_ms: 0.0,
            from: HashMap::new(),
        }
    }

    /// Advances by `dt_ms` and writes this playback's current values into
    /// `layer`. Returns `false` once a non-looping asset has played through
    /// its last step — the caller should stop *ticking* it at that point, but
    /// per `LightAsset`'s "Scenes always apply steps[0] once" contract, the
    /// values already written into `layer` are meant to persist (a Scene is a
    /// static look, not a blip), so the caller shouldn't drop `layer` itself,
    /// only this `RunningPlayback`.
    ///
    /// Loops rather than recursing through phase transitions, carrying over
    /// leftover time each time, so a single large `dt_ms` (a slow tick, or a
    /// step with 0 fade/hold time) still lands on the correct value instead of
    /// needing an extra call to "catch up" — a step's value used to only get
    /// written on the *next* call after a Hold-to-Fade transition, leaving a
    /// stale value on the wire for one extra tick.
    pub fn advance(&mut self, dt_ms: f64, layer: &mut Layer) -> bool {
        self.elapsed_ms += dt_ms;

        loop {
            let Some(step) = self.asset.steps.get(self.step_index) else {
                return false;
            };

            match self.phase {
                Phase::Fading => {
                    let fade = step.fade_ms.max(1.0);
                    if self.elapsed_ms >= fade {
                        for (&channel, &target) in &step.channels {
                            layer.values.insert((self.universe, channel), target as f32);
                        }
                        self.phase = Phase::Holding;
                        self.elapsed_ms -= fade;
                        continue;
                    }
                    let t = self.elapsed_ms / fade;
                    for (&channel, &target) in &step.channels {
                        let from = *self.from.get(&channel).unwrap_or(&0.0);
                        let value = from + (target as f32 - from) * t as f32;
                        layer.values.insert((self.universe, channel), value);
                    }
                    return true;
                }
                Phase::Holding => {
                    if self.elapsed_ms >= step.hold_ms {
                        // Snapshot this step's targets as the next step's
                        // fade-from, then advance (looping back to step 0 if
                        // configured).
                        for (&channel, &target) in &step.channels {
                            self.from.insert(channel, target as f32);
                        }
                        self.elapsed_ms -= step.hold_ms;
                        self.step_index += 1;
                        if self.step_index >= self.asset.steps.len() {
                            if self.asset.loop_ {
                                self.step_index = 0;
                            } else {
                                return false;
                            }
                        }
                        self.phase = Phase::Fading;
                        continue;
                    }
                    return true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(channels: &[(u16, u8)], fade_ms: f64, hold_ms: f64) -> LegacyStep {
        LegacyStep {
            channels: channels.iter().copied().collect(),
            fade_ms,
            hold_ms,
        }
    }

    #[test]
    fn fades_linearly_from_zero_to_target() {
        let asset = LegacyAsset { steps: vec![step(&[(1, 200)], 100.0, 0.0)], loop_: false };
        let mut playback = RunningPlayback::new("a".into(), 1, 1, asset);
        let mut layer = Layer::default();

        playback.advance(50.0, &mut layer);
        assert_eq!(layer.values[&(1, 1)].round() as u8, 100); // halfway through a 100ms fade to 200

        playback.advance(50.0, &mut layer);
        assert_eq!(layer.values[&(1, 1)].round() as u8, 200);
    }

    #[test]
    fn holds_after_fade_then_advances_to_next_step() {
        let asset = LegacyAsset {
            steps: vec![step(&[(1, 255)], 0.0, 100.0), step(&[(1, 0)], 0.0, 100.0)],
            loop_: false,
        };
        let mut playback = RunningPlayback::new("a".into(), 1, 1, asset);
        let mut layer = Layer::default();

        assert!(playback.advance(10.0, &mut layer)); // instant fade, now holding step 0
        assert_eq!(layer.values[&(1, 1)] as u8, 255);

        // Past step 0's 100ms hold (elapsed so far: 10ms) -> step 1's instant
        // fade lands on its target, with leftover time carried into step 1's
        // hold (but not enough to finish it, so playback is still running).
        assert!(playback.advance(140.0, &mut layer));
        assert_eq!(layer.values[&(1, 1)] as u8, 0);
    }

    #[test]
    fn a_large_dt_can_finish_the_whole_asset_in_one_call() {
        let asset = LegacyAsset {
            steps: vec![step(&[(1, 255)], 0.0, 10.0), step(&[(1, 0)], 0.0, 10.0)],
            loop_: false,
        };
        let mut playback = RunningPlayback::new("a".into(), 1, 1, asset);
        let mut layer = Layer::default();

        // One big tick blows straight through both steps' fades and holds.
        assert!(!playback.advance(1000.0, &mut layer));
        assert_eq!(layer.values[&(1, 1)] as u8, 0); // still lands on the final target
    }

    #[test]
    fn non_looping_asset_ends_after_its_last_step() {
        let asset = LegacyAsset { steps: vec![step(&[(1, 255)], 0.0, 10.0)], loop_: false };
        let mut playback = RunningPlayback::new("a".into(), 1, 1, asset);
        let mut layer = Layer::default();

        assert!(playback.advance(5.0, &mut layer));
        assert!(!playback.advance(20.0, &mut layer)); // hold expired, no next step, not looping
    }

    #[test]
    fn looping_asset_wraps_back_to_its_first_step() {
        let asset = LegacyAsset {
            steps: vec![step(&[(1, 255)], 0.0, 10.0), step(&[(1, 0)], 0.0, 10.0)],
            loop_: true,
        };
        let mut playback = RunningPlayback::new("a".into(), 1, 1, asset);
        let mut layer = Layer::default();

        for _ in 0..10 {
            assert!(playback.advance(15.0, &mut layer));
        }
        // Still running many cycles later, not stuck or ended.
    }

    #[test]
    fn empty_asset_ends_immediately() {
        let asset = LegacyAsset { steps: vec![], loop_: false };
        let mut playback = RunningPlayback::new("a".into(), 1, 1, asset);
        let mut layer = Layer::default();
        assert!(!playback.advance(10.0, &mut layer));
    }
}
