//! The lighting engine core: HTP/LTP merge, the programmer, the command
//! line, and frame-accurate legacy asset playback. See docs/ENGINE.md.

pub mod command_line;
pub mod commands;
pub mod groups;
pub mod legacy_playback;
pub mod manager;
pub mod merge;
pub mod presets;
pub mod state;
