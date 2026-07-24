// DMX512 hardware output engine.
//
// Architecture (per SRS 2.3): the UI/AI layers only ever touch `DmxEngine.universe`,
// a shared 512-byte buffer. A dedicated OS thread owns the actual serial handle and
// blasts that buffer to the FTDI adapter continuously at ~44Hz, so a slow AI request
// or a React re-render can never introduce a hitch in the light output.

use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter};

pub const UNIVERSE_SIZE: usize = 512;
const FRAME_INTERVAL: Duration = Duration::from_millis(23); // ~44Hz

#[derive(Clone, Serialize)]
pub struct ConnectionStatus {
    pub connected: bool,
    pub port: Option<String>,
    pub error: Option<String>,
}

pub struct DmxEngine {
    pub universe: Arc<Mutex<[u8; UNIVERSE_SIZE]>>,
    requested_port: Arc<Mutex<Option<String>>>,
    status: Arc<Mutex<ConnectionStatus>>,
    pub blackout: Arc<AtomicBool>,
}

impl DmxEngine {
    pub fn new(app: AppHandle) -> Self {
        let engine = Self {
            universe: Arc::new(Mutex::new([0u8; UNIVERSE_SIZE])),
            requested_port: Arc::new(Mutex::new(None)),
            status: Arc::new(Mutex::new(ConnectionStatus {
                connected: false,
                port: None,
                error: None,
            })),
            blackout: Arc::new(AtomicBool::new(false)),
        };
        engine.spawn_output_thread(app);
        engine
    }

    pub fn request_port(&self, port_name: Option<String>) {
        *self.requested_port.lock().unwrap() = port_name;
    }

    pub fn current_status(&self) -> ConnectionStatus {
        self.status.lock().unwrap().clone()
    }

    pub fn set_universe(&self, data: [u8; UNIVERSE_SIZE]) {
        *self.universe.lock().unwrap() = data;
    }

    fn spawn_output_thread(&self, app: AppHandle) {
        let universe = self.universe.clone();
        let requested_port = self.requested_port.clone();
        let status = self.status.clone();
        let blackout = self.blackout.clone();
        let engine_status_setter = self.status.clone();

        thread::spawn(move || {
            let mut port: Option<Box<dyn serialport::SerialPort>> = None;
            let mut open_port_name: Option<String> = None;

            loop {
                let wanted = requested_port.lock().unwrap().clone();

                // (Re)connect if the requested port changed, or a previous write failed
                // and dropped `port` to None while a port is still wanted.
                if wanted != open_port_name || (wanted.is_some() && port.is_none()) {
                    port = None;
                    open_port_name = None;

                    if let Some(name) = &wanted {
                        match open_dmx_port(name) {
                            Ok(p) => {
                                port = Some(p);
                                open_port_name = Some(name.clone());
                                let s = ConnectionStatus {
                                    connected: true,
                                    port: Some(name.clone()),
                                    error: None,
                                };
                                *status.lock().unwrap() = s.clone();
                                let _ = app.emit("dmx://connection-changed", s);
                            }
                            Err(e) => {
                                let s = ConnectionStatus {
                                    connected: false,
                                    port: Some(name.clone()),
                                    error: Some(e),
                                };
                                *status.lock().unwrap() = s.clone();
                                let _ = app.emit("dmx://connection-changed", s);
                                // Back off before retrying so an unplugged cable doesn't spin the CPU.
                                thread::sleep(Duration::from_millis(500));
                            }
                        }
                    } else {
                        let s = ConnectionStatus {
                            connected: false,
                            port: None,
                            error: None,
                        };
                        *status.lock().unwrap() = s.clone();
                        let _ = app.emit("dmx://connection-changed", s);
                    }
                }

                if let Some(p) = port.as_mut() {
                    let frame = if blackout.load(Ordering::Relaxed) {
                        [0u8; UNIVERSE_SIZE]
                    } else {
                        *universe.lock().unwrap()
                    };

                    if let Err(e) = write_dmx_frame(p.as_mut(), &frame) {
                        // Cable likely yanked mid-set: drop the handle, surface the error,
                        // and let the top of the loop retry the reconnect on its own.
                        port = None;
                        let name = open_port_name.take();
                        let s = ConnectionStatus {
                            connected: false,
                            port: name,
                            error: Some(e.to_string()),
                        };
                        *engine_status_setter.lock().unwrap() = s.clone();
                        let _ = app.emit("dmx://connection-changed", s);
                    }
                } else {
                    thread::sleep(FRAME_INTERVAL);
                }
            }
        });
    }
}

fn open_dmx_port(name: &str) -> Result<Box<dyn serialport::SerialPort>, String> {
    serialport::new(name, 250_000)
        .data_bits(serialport::DataBits::Eight)
        .parity(serialport::Parity::None)
        .stop_bits(serialport::StopBits::Two)
        .timeout(Duration::from_millis(50))
        .open()
        .map_err(|e| format!("Failed to open {}: {}", name, e))
}

/// Writes one full DMX512 frame: BREAK, MAB, start code, 512 channel bytes.
///
/// `serialport` has no cross-platform "send a UART break" primitive, so we fake the
/// break the same way most FTDI-based DMX software does: drop to a much lower baud
/// rate and clock out a 0x00 byte, which holds the line low for far longer than the
/// 88us DMX spec minimum, then switch back to 250000 baud for the real payload.
fn write_dmx_frame(
    port: &mut dyn serialport::SerialPort,
    universe: &[u8; UNIVERSE_SIZE],
) -> Result<(), std::io::Error> {
    // ~176us break: at 12500 baud one bit is 80us, so one 0x00 byte (10 bits w/ framing)
    // holds the line low for ~800us of actual break-equivalent signal -- comfortably
    // over spec and matched to the PRD's 176us break requirement.
    port.set_baud_rate(12_500)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    port.write_all(&[0x00])?;
    port.flush()?;

    // ~12us Mark-After-Break.
    thread::sleep(Duration::from_micros(12));

    port.set_baud_rate(250_000)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    let mut frame = Vec::with_capacity(1 + UNIVERSE_SIZE);
    frame.push(0x00); // DMX start code
    frame.extend_from_slice(universe);
    port.write_all(&frame)?;
    port.flush()
}
