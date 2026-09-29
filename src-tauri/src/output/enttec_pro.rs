//! Enttec DMX USB Pro driver.
//!
//! Unlike the plain FTDI Open DMX widget (`output::ftdi`), the Pro has an
//! onboard microcontroller that generates real DMX timing itself from a
//! framed serial message, so the host doesn't do a hardware break — that's
//! why `needs_realtime_priority` is false here.
//!
//! Frame format ("Send DMX Packet" request, Enttec DMX USB Pro API spec):
//! `0x7E, label(0x06), lenLo, lenHi, [0x00 start code, 512 channel bytes], 0xE7`.
//! This project has no Enttec Pro widget to verify against, so this is
//! implemented from the documented protocol only — see HARDWARE_CHECKS.md.
//! The Pro Mk2's second universe (`universe_index: 1`) needs a widget to
//! confirm its port-select framing before it's safe to guess at, so it's
//! rejected with a clear error rather than silently sending unverified bytes.

use std::time::{Duration, Instant};

use super::{OutputDriver, UNIVERSE_SIZE};

const START_DELIMITER: u8 = 0x7E;
const END_DELIMITER: u8 = 0xE7;
const LABEL_SEND_DMX: u8 = 0x06;
const RECONNECT_BACKOFF: Duration = Duration::from_millis(500);

/// Encodes a "Send DMX Packet" request for universe/port 0.
pub fn encode_send_dmx(data: &[u8; UNIVERSE_SIZE]) -> Vec<u8> {
    let payload_len: u16 = 1 + UNIVERSE_SIZE as u16; // DMX start code + channels
    let mut packet = Vec::with_capacity(5 + payload_len as usize);
    packet.push(START_DELIMITER);
    packet.push(LABEL_SEND_DMX);
    packet.push((payload_len & 0xFF) as u8);
    packet.push((payload_len >> 8) as u8);
    packet.push(0x00); // DMX start code
    packet.extend_from_slice(data);
    packet.push(END_DELIMITER);
    packet
}

pub struct EnttecProDriver {
    port_name: Option<String>,
    universe_index: u8,
    port: Option<Box<dyn serialport::SerialPort>>,
    last_attempt: Option<Instant>,
}

impl EnttecProDriver {
    pub fn new(port_name: Option<String>, universe_index: u8) -> Self {
        Self {
            port_name,
            universe_index,
            port: None,
            last_attempt: None,
        }
    }
}

impl OutputDriver for EnttecProDriver {
    fn write_frame(&mut self, data: &[u8; UNIVERSE_SIZE]) -> Result<(), String> {
        if self.universe_index != 0 {
            return Err(
                "Enttec Pro Mk2 universe 2 needs a real widget to verify its port-select \
                 framing before this driver can support it (see HARDWARE_CHECKS.md); use \
                 universe index 0 for now."
                    .to_string(),
            );
        }

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
            self.port = Some(open_port(name)?);
        }

        let port = self.port.as_mut().expect("just ensured Some above");
        let packet = encode_send_dmx(data);
        match port.write_all(&packet).and_then(|()| port.flush()) {
            Ok(()) => Ok(()),
            Err(e) => {
                self.port = None;
                Err(e.to_string())
            }
        }
    }
}

fn open_port(name: &str) -> Result<Box<dyn serialport::SerialPort>, String> {
    serialport::new(name, 250_000)
        .timeout(Duration::from_millis(50))
        .open()
        .map_err(|e| format!("Failed to open {name}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn send_dmx_frame_matches_spec() {
        let mut data = [0u8; UNIVERSE_SIZE];
        data[0] = 255;
        data[511] = 7;
        let packet = encode_send_dmx(&data);

        assert_eq!(packet[0], 0x7E);
        assert_eq!(packet[1], 0x06);
        let len = u16::from_le_bytes([packet[2], packet[3]]);
        assert_eq!(len, 513);
        assert_eq!(packet[4], 0x00); // start code
        assert_eq!(packet[5], 255); // channel 1
        assert_eq!(packet[4 + 512], 7); // channel 512
        assert_eq!(*packet.last().unwrap(), 0xE7);
        assert_eq!(packet.len(), 5 + 513);
    }

    #[test]
    fn universe_index_1_is_explicitly_rejected() {
        let mut driver = EnttecProDriver::new(None, 1);
        let err = driver.write_frame(&[0u8; UNIVERSE_SIZE]).unwrap_err();
        assert!(err.contains("Mk2"));
    }
}
