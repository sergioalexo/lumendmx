//! FTDI Open DMX driver.
//!
//! Uses a real hardware break (`SerialPort::set_break`/`clear_break`, backed by
//! Windows' `SetCommBreak`/`ClearCommBreak`) rather than faking one via a
//! baud-rate switch. This matches what QLC+'s Enttec Open DMX driver does for
//! this exact class of genuine-FTDI USB-DMX widget -- including its 110us
//! break / 16us MAB timing -- which is a known-working reference for this
//! hardware (confirmed on the user's rig, COM9). **Never** replace this with a
//! baud-rate-switch fake break; see FIXTURES.md and BUILD_PLAN.md.

use std::thread;
use std::time::{Duration, Instant};

use super::{OutputDriver, UNIVERSE_SIZE};

/// Minimum time between reconnect attempts when the port is unplugged/unopenable,
/// so a missing cable doesn't spin the CPU retrying every frame.
const RECONNECT_BACKOFF: Duration = Duration::from_millis(500);

pub struct FtdiDriver {
    port_name: Option<String>,
    port: Option<Box<dyn serialport::SerialPort>>,
    last_attempt: Option<Instant>,
}

impl FtdiDriver {
    pub fn new(port_name: Option<String>) -> Self {
        Self {
            port_name,
            port: None,
            last_attempt: None,
        }
    }
}

impl OutputDriver for FtdiDriver {
    fn write_frame(&mut self, data: &[u8; UNIVERSE_SIZE]) -> Result<(), String> {
        if self.port.is_none() {
            let due = self
                .last_attempt
                .is_none_or(|t| t.elapsed() >= RECONNECT_BACKOFF);
            if !due {
                return Err("Reconnecting…".to_string());
            }
            self.last_attempt = Some(Instant::now());

            let name = self
                .port_name
                .as_ref()
                .ok_or_else(|| "No serial port configured".to_string())?;
            self.port = Some(open_dmx_port(name)?);
        }

        let port = self.port.as_mut().expect("just ensured Some above");
        match write_dmx_frame(port.as_mut(), data) {
            Ok(()) => Ok(()),
            Err(e) => {
                // Cable likely yanked mid-set: drop the handle so the next
                // call retries the reconnect (subject to the backoff above).
                self.port = None;
                Err(e.to_string())
            }
        }
    }

    fn needs_realtime_priority(&self) -> bool {
        true
    }
}

fn open_dmx_port(name: &str) -> Result<Box<dyn serialport::SerialPort>, String> {
    serialport::new(name, 250_000)
        .data_bits(serialport::DataBits::Eight)
        .parity(serialport::Parity::None)
        .stop_bits(serialport::StopBits::Two)
        .timeout(Duration::from_millis(50))
        .open()
        .map_err(|e| format!("Failed to open {name}: {e}"))
}

/// Writes one full DMX512 frame: BREAK, MAB, start code, 512 channel bytes.
fn write_dmx_frame(
    port: &mut dyn serialport::SerialPort,
    universe: &[u8; UNIVERSE_SIZE],
) -> Result<(), std::io::Error> {
    port.set_break().map_err(std::io::Error::other)?;
    thread::sleep(Duration::from_micros(110));
    port.clear_break().map_err(std::io::Error::other)?;
    thread::sleep(Duration::from_micros(16)); // Mark-After-Break

    let mut frame = Vec::with_capacity(1 + UNIVERSE_SIZE);
    frame.push(0x00); // DMX start code
    frame.extend_from_slice(universe);
    port.write_all(&frame)?;
    port.flush()
}
