//! The engine's live state: a set of prioritized "layers" (the programmer,
//! plus one per running legacy asset playback) and, per universe, which
//! channels merge via HTP vs LTP.
//!
//! Deliberately channel-based, not fixture/attribute-based: resolving "select
//! fixture 3's red channel" into an actual (universe, channel) pair is static
//! configuration data, not timing-critical, so that resolution stays in the
//! frontend (which already has the fixture library) and only the resolved
//! result crosses into Rust. See docs/ENGINE.md.

use std::collections::{HashMap, HashSet};

/// A DMX address: which universe, which of its 512 channels (1-512).
pub type ChannelKey = (u32, u16);

/// The programmer always outranks every playback layer.
pub const PROGRAMMER_PRIORITY: i64 = i64::MAX;

#[derive(Debug, Default)]
pub struct Layer {
    /// 0.0-255.0 so fades can interpolate sub-integer without a separate
    /// float layer type; rounded to `u8` only when merged for output.
    pub values: HashMap<ChannelKey, f32>,
}

pub struct EngineState {
    pub programmer: Layer,
    /// Keyed by an arbitrary caller-chosen id (today: a triggered legacy
    /// asset's id). Iterated in priority order during merge.
    pub playbacks: HashMap<String, (i64, Layer)>,
    pub htp_channels: HashMap<u32, HashSet<u16>>,
    next_playback_priority: i64,
}

impl Default for EngineState {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineState {
    pub fn new() -> Self {
        Self {
            programmer: Layer::default(),
            playbacks: HashMap::new(),
            htp_channels: HashMap::new(),
            next_playback_priority: 0,
        }
    }

    /// Playback layers are priority-ordered by trigger order: whichever was
    /// triggered most recently wins LTP ties over one triggered earlier, a
    /// reasonable default "last one wins" until Phase 6 gives cuelists their
    /// own explicit priority levels.
    pub fn next_priority(&mut self) -> i64 {
        self.next_playback_priority += 1;
        self.next_playback_priority
    }

    pub fn is_htp(&self, universe: u32, channel: u16) -> bool {
        self.htp_channels.get(&universe).is_some_and(|set| set.contains(&channel))
    }
}
