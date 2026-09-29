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
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};

pub const UNIVERSE_SIZE: usize = 512;
// 30Hz, matching QLC+'s Enttec Open DMX default rate for this exact class of
// FTDI-based widget -- explicit, unhurried pacing rather than firing frames back to
// back as fast as the writes complete, which leaves less margin for USB jitter.
const FRAME_INTERVAL: Duration = Duration::from_micros(33_333);

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
            // Windows can preempt this thread for tens of milliseconds at a time under
            // normal scheduling; that's long enough to land mid-frame and corrupt the
            // break/payload timing on the wire, which reads to a fixture as a brief
            // signal glitch. Asking for a higher scheduling priority doesn't eliminate
            // that risk, but it substantially reduces how often the OS pauses this
            // specific thread for other work.
            #[cfg(windows)]
            unsafe {
                windows_sys::Win32::System::Threading::SetThreadPriority(
                    windows_sys::Win32::System::Threading::GetCurrentThread(),
                    windows_sys::Win32::System::Threading::THREAD_PRIORITY_TIME_CRITICAL,
                );
            }

            let mut port: Option<Box<dyn serialport::SerialPort>> = None;
            let mut open_port_name: Option<String> = None;

            loop {
                let frame_start = Instant::now();
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
                }

                // Pace to a steady ~30Hz regardless of whether a frame was actually
                // written this iteration, rather than looping as fast as the writes
                // complete.
                let elapsed = frame_start.elapsed();
                if elapsed < FRAME_INTERVAL {
                    thread::sleep(FRAME_INTERVAL - elapsed);
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
/// Uses a real hardware break (`SerialPort::set_break`/`clear_break`, backed by
/// Windows' `SetCommBreak`/`ClearCommBreak`) rather than faking one via a baud-rate
/// switch. This matches what QLC+'s Enttec Open DMX driver does for this exact class
/// of genuine-FTDI USB-DMX widget -- including its 110us break / 16us MAB timing --
/// which is a known-working reference for this hardware. (An earlier theory that
/// this adapter's chip didn't support real breaks turned out to be based on a wrong
/// chip identification; it's genuine FTDI, confirmed via Device Manager.)
fn write_dmx_frame(
    port: &mut dyn serialport::SerialPort,
    universe: &[u8; UNIVERSE_SIZE],
) -> Result<(), std::io::Error> {
    port.set_break()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    thread::sleep(Duration::from_micros(110));
    port.clear_break()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    thread::sleep(Duration::from_micros(16)); // Mark-After-Break

    let mut frame = Vec::with_capacity(1 + UNIVERSE_SIZE);
    frame.push(0x00); // DMX start code
    frame.extend_from_slice(universe);
    port.write_all(&frame)?;
    port.flush()
}
