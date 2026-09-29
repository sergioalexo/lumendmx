//! Output drivers: one `OutputDriver` implementation per way of getting a DMX
//! universe onto the wire (or network). `manager.rs` owns one dedicated thread
//! per active universe via `runner.rs`, so a slow/blocked driver on one
//! universe can never stall another's timing.

pub mod artnet;
pub mod commands;
pub mod config;
pub mod enttec_pro;
pub mod ftdi;
pub mod manager;
pub mod null;
pub mod runner;
pub mod sacn;

pub const UNIVERSE_SIZE: usize = 512;

/// One way of sending a DMX universe out. Implementations are driven by
/// `runner::UniverseOutput` on a dedicated thread at a configured rate; they
/// own their own connection state and handle their own reconnect/backoff.
pub trait OutputDriver: Send {
    fn write_frame(&mut self, data: &[u8; UNIVERSE_SIZE]) -> Result<(), String>;

    /// Whether this driver's timing is sensitive enough to OS scheduler jitter
    /// that its output thread should run at real-time priority. True for
    /// local serial drivers doing a hardware break; false for network drivers,
    /// where a few milliseconds of jitter doesn't corrupt anything on the wire.
    fn needs_realtime_priority(&self) -> bool {
        false
    }
}
