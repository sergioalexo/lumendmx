//! A running playback: GO/Back/Pause/Release through a cuelist, with a
//! time-based crossfade between cues, a fader (intensity master or manual
//! crossfade), and — for a Chase cuelist — BPM-driven auto-advance. Renders
//! into a plain channel-value map every tick, the same shape as a legacy
//! asset's layer, so `manager::EngineManager`'s tick loop treats both
//! uniformly (see `manager.rs`'s "cue playbacks" section).

use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::cues::{resolve_cue_state, ChaseDirection, Cuelist, CuelistKind};
use super::presets::PresetStore;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/showfile/generated/")]
pub enum FaderMode {
    /// The fader (0.0-1.0) scales this playback's HTP/intensity channels;
    /// everything else plays at the cue's recorded value.
    IntensityMaster,
    /// The fader position *is* the crossfade position between the previous
    /// and current cue (manual crossfade), instead of time-based.
    Crossfade,
}

pub struct PlaybackRuntime {
    pub id: u32,
    pub cuelist_id: u32,
    /// Determines LTP precedence against other playbacks and the legacy
    /// playback layers sharing the same merge — see `manager.rs`.
    pub priority: i64,
    pub fader_mode: FaderMode,
    pub fader: f32, // 0.0-1.0
    pub current_index: Option<usize>,
    pub paused: bool,
    from_state: HashMap<(u32, u16), f32>,
    to_state: HashMap<(u32, u16), f32>,
    fade_ms: f64,
    elapsed_ms: f64,
    /// Time since the last `go()`, for a Chase's BPM-driven auto-advance —
    /// separate from `elapsed_ms` because a chase's total step time (the
    /// beat interval) generally isn't the same as its crossfade time.
    step_elapsed_ms: f64,
    chase_forward: bool,
}

impl PlaybackRuntime {
    pub fn new(id: u32, cuelist_id: u32, priority: i64) -> Self {
        Self {
            id,
            cuelist_id,
            priority,
            fader_mode: FaderMode::IntensityMaster,
            fader: 1.0,
            current_index: None,
            paused: false,
            from_state: HashMap::new(),
            to_state: HashMap::new(),
            fade_ms: 1.0,
            elapsed_ms: 0.0,
            step_elapsed_ms: 0.0,
            chase_forward: true,
        }
    }

    /// This tick's blended (universe,channel)->value state, before the fader
    /// is applied.
    fn render_unfadered(&self) -> HashMap<(u32, u16), f32> {
        let t = if self.fader_mode == FaderMode::Crossfade {
            self.fader as f64
        } else {
            (self.elapsed_ms / self.fade_ms).min(1.0)
        };
        let mut keys: HashSet<(u32, u16)> = self.from_state.keys().copied().collect();
        keys.extend(self.to_state.keys().copied());
        keys.into_iter()
            .map(|key| {
                let from = *self.from_state.get(&key).unwrap_or(&0.0);
                let to = *self.to_state.get(&key).unwrap_or(&0.0);
                (key, from + (to - from) * t as f32)
            })
            .collect()
    }

    /// This tick's contribution, with the fader applied (intensity-master
    /// mode scales only channels in `htp_channels`).
    pub fn render(&self, htp_channels: &HashMap<u32, HashSet<u16>>) -> HashMap<(u32, u16), f32> {
        let mut state = self.render_unfadered();
        if self.fader_mode == FaderMode::IntensityMaster {
            for (&(universe, channel), value) in state.iter_mut() {
                if htp_channels.get(&universe).is_some_and(|set| set.contains(&channel)) {
                    *value *= self.fader;
                }
            }
        }
        state
    }

    pub fn go(&mut self, cuelist: &Cuelist, presets: &PresetStore, target_index: usize) {
        self.from_state = self.render_unfadered();
        self.to_state = resolve_cue_state(cuelist, target_index, presets);
        self.fade_ms = cuelist.cues.get(target_index).map_or(1.0, |c| c.fade_in_ms.max(1.0));
        self.elapsed_ms = 0.0;
        self.step_elapsed_ms = 0.0;
        self.current_index = Some(target_index);
    }

    pub fn go_next(&mut self, cuelist: &Cuelist, presets: &PresetStore) {
        let next = self.current_index.map_or(0, |i| i + 1);
        if next < cuelist.cues.len() {
            self.go(cuelist, presets, next);
        }
    }

    pub fn go_back(&mut self, cuelist: &Cuelist, presets: &PresetStore) {
        if let Some(index) = self.current_index {
            if index > 0 {
                self.go(cuelist, presets, index - 1);
            }
        }
    }

    /// Fades everything this playback is contributing down to nothing and
    /// forgets its cue position.
    pub fn release(&mut self, release_fade_ms: f64) {
        self.from_state = self.render_unfadered();
        self.to_state = HashMap::new();
        self.fade_ms = release_fade_ms.max(1.0);
        self.elapsed_ms = 0.0;
        self.current_index = None;
    }

    /// Advances crossfade time and, for a Chase, auto-GOes to the next cue
    /// once a full beat interval has elapsed.
    pub fn tick(&mut self, dt_ms: f64, cuelist: &Cuelist, presets: &PresetStore) {
        if self.paused {
            return;
        }
        self.elapsed_ms += dt_ms;
        self.step_elapsed_ms += dt_ms;

        if let CuelistKind::Chase { bpm, direction } = cuelist.kind {
            if bpm <= 0.0 || cuelist.cues.is_empty() {
                return;
            }
            if self.current_index.is_none() {
                self.go(cuelist, presets, 0);
                return;
            }
            let beat_ms = 60_000.0 / bpm as f64;
            if self.step_elapsed_ms >= beat_ms {
                let next = self.next_chase_index(cuelist.cues.len(), direction);
                self.go(cuelist, presets, next);
            }
        }
    }

    fn next_chase_index(&mut self, len: usize, direction: ChaseDirection) -> usize {
        let current = self.current_index.unwrap_or(0);
        match direction {
            ChaseDirection::Forward => (current + 1) % len,
            ChaseDirection::Bounce => {
                if len <= 1 {
                    return 0;
                }
                if self.chase_forward {
                    let next = current + 1;
                    if next >= len - 1 {
                        self.chase_forward = false;
                        next.min(len - 1)
                    } else {
                        next
                    }
                } else if current == 0 {
                    // Hit the bottom this step: reverse and move in the new
                    // (forward) direction immediately, not the old one.
                    self.chase_forward = true;
                    1.min(len - 1)
                } else {
                    current - 1
                }
            }
            ChaseDirection::Random => {
                let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
                (nanos as usize) % len
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::cues::{Cue, CueValue};

    fn cue_with(number: f64, channel: u16, value: f32, fade_ms: f64) -> Cue {
        let mut cue = Cue::new(number, format!("Cue {number}"));
        cue.values.insert((1, channel), CueValue::Literal { value });
        cue.fade_in_ms = fade_ms;
        cue
    }

    #[test]
    fn go_crossfades_linearly_over_the_target_cues_fade_time() {
        let mut list = Cuelist::new(1, "L".to_string(), CuelistKind::Standard, true);
        list.upsert_cue(cue_with(1.0, 1, 200.0, 100.0));
        let presets = PresetStore::new();

        let mut pb = PlaybackRuntime::new(1, 1, 10);
        pb.go(&list, &presets, 0);
        pb.tick(50.0, &list, &presets);
        let mid = pb.render(&HashMap::new());
        assert_eq!(mid[&(1, 1)].round() as u8, 100); // halfway through a 100ms fade to 200

        pb.tick(50.0, &list, &presets);
        let done = pb.render(&HashMap::new());
        assert_eq!(done[&(1, 1)].round() as u8, 200);
    }

    #[test]
    fn intensity_master_fader_scales_only_htp_channels() {
        let mut list = Cuelist::new(1, "L".to_string(), CuelistKind::Standard, true);
        let mut cue = Cue::new(1.0, "Cue 1".to_string());
        cue.values.insert((1, 1), CueValue::Literal { value: 200.0 }); // intensity
        cue.values.insert((1, 2), CueValue::Literal { value: 200.0 }); // color, not HTP
        cue.fade_in_ms = 1.0;
        list.upsert_cue(cue);
        let presets = PresetStore::new();

        let mut pb = PlaybackRuntime::new(1, 1, 10);
        pb.fader = 0.5;
        pb.go(&list, &presets, 0);
        pb.tick(10.0, &list, &presets);

        let mut htp = HashMap::new();
        htp.insert(1u32, HashSet::from([1u16]));
        let rendered = pb.render(&htp);
        assert_eq!(rendered[&(1, 1)], 100.0); // scaled by the 50% fader
        assert_eq!(rendered[&(1, 2)], 200.0); // untouched
    }

    #[test]
    fn go_back_returns_to_the_previous_cue() {
        let mut list = Cuelist::new(1, "L".to_string(), CuelistKind::Standard, true);
        list.upsert_cue(cue_with(1.0, 1, 100.0, 1.0));
        list.upsert_cue(cue_with(2.0, 1, 200.0, 1.0));
        let presets = PresetStore::new();

        let mut pb = PlaybackRuntime::new(1, 1, 10);
        pb.go(&list, &presets, 1);
        pb.tick(10.0, &list, &presets);
        assert_eq!(pb.render(&HashMap::new())[&(1, 1)], 200.0);

        pb.go_back(&list, &presets);
        pb.tick(10.0, &list, &presets);
        assert_eq!(pb.render(&HashMap::new())[&(1, 1)], 100.0);
    }

    #[test]
    fn release_fades_to_nothing_and_forgets_cue_position() {
        let mut list = Cuelist::new(1, "L".to_string(), CuelistKind::Standard, true);
        list.upsert_cue(cue_with(1.0, 1, 200.0, 1.0));
        let presets = PresetStore::new();

        let mut pb = PlaybackRuntime::new(1, 1, 10);
        pb.go(&list, &presets, 0);
        pb.tick(10.0, &list, &presets);
        pb.release(100.0);
        pb.tick(100.0, &list, &presets);

        assert_eq!(pb.current_index, None);
        assert_eq!(pb.render(&HashMap::new()).get(&(1, 1)), Some(&0.0));
    }

    #[test]
    fn chase_auto_advances_forward_at_the_bpm_derived_interval() {
        let mut list = Cuelist::new(
            1,
            "Chase".to_string(),
            CuelistKind::Chase { bpm: 120.0, direction: ChaseDirection::Forward },
            true,
        );
        // 120 BPM = 500ms/beat.
        list.upsert_cue(cue_with(1.0, 1, 100.0, 1.0));
        list.upsert_cue(cue_with(2.0, 1, 200.0, 1.0));
        let presets = PresetStore::new();

        let mut pb = PlaybackRuntime::new(1, 1, 10);
        pb.tick(1.0, &list, &presets); // starts on cue 0
        assert_eq!(pb.current_index, Some(0));

        pb.tick(500.0, &list, &presets); // one full beat -> advance to cue 1
        assert_eq!(pb.current_index, Some(1));

        pb.tick(500.0, &list, &presets); // wraps back to cue 0 (Forward loops)
        assert_eq!(pb.current_index, Some(0));
    }

    #[test]
    fn chase_bounce_reverses_at_each_end() {
        let mut list = Cuelist::new(
            1,
            "Chase".to_string(),
            CuelistKind::Chase { bpm: 600.0, direction: ChaseDirection::Bounce }, // 100ms/beat
            true,
        );
        for i in 0..3 {
            list.upsert_cue(cue_with(i as f64 + 1.0, 1, i as f32, 1.0));
        }
        let presets = PresetStore::new();
        let mut pb = PlaybackRuntime::new(1, 1, 10);

        let mut indices = Vec::new();
        pb.tick(1.0, &list, &presets);
        indices.push(pb.current_index.unwrap());
        for _ in 0..5 {
            pb.tick(100.0, &list, &presets);
            indices.push(pb.current_index.unwrap());
        }
        // 0 -> 1 -> 2 -> 1 -> 0 -> 1 : bounces at both ends instead of wrapping.
        assert_eq!(indices, vec![0, 1, 2, 1, 0, 1]);
    }

    #[test]
    fn paused_playback_does_not_advance() {
        let mut list = Cuelist::new(
            1,
            "Chase".to_string(),
            CuelistKind::Chase { bpm: 600.0, direction: ChaseDirection::Forward },
            true,
        );
        list.upsert_cue(cue_with(1.0, 1, 1.0, 1.0));
        list.upsert_cue(cue_with(2.0, 1, 2.0, 1.0));
        let presets = PresetStore::new();

        let mut pb = PlaybackRuntime::new(1, 1, 10);
        pb.tick(1.0, &list, &presets);
        pb.paused = true;
        pb.tick(1000.0, &list, &presets);
        assert_eq!(pb.current_index, Some(0));
    }
}
