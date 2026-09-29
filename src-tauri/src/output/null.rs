//! No-op driver: always "succeeds" without sending anything anywhere. Used for
//! universes with no driver configured, and as a hardware-free test double.

use super::{OutputDriver, UNIVERSE_SIZE};

#[derive(Default)]
pub struct NullDriver;

impl OutputDriver for NullDriver {
    fn write_frame(&mut self, _data: &[u8; UNIVERSE_SIZE]) -> Result<(), String> {
        Ok(())
    }
}
