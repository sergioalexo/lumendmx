//! Runs one `OutputDriver` on its own dedicated OS thread at a fixed pace, so
//! one universe's driver can never stall another's timing (per BUILD_PLAN
//! Phase 2: "the UI can freeze for 2s without output stuttering" carries over
//! to "one slow driver can't stall another").

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use ts_rs::TS;

use super::{OutputDriver, UNIVERSE_SIZE};

#[derive(Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/showfile/generated/")]
pub struct UniverseStatus {
    pub universe_id: u32,
    pub connected: bool,
    pub error: Option<String>,
    pub frames_per_sec: f32,
}

pub const STATUS_EVENT: &str = "dmx://universe-status-changed";

fn initial_status(universe_id: u32) -> UniverseStatus {
    UniverseStatus {
        universe_id,
        connected: false,
        error: None,
        frames_per_sec: 0.0,
    }
}

/// One universe's live output: a shared 512-byte buffer plus the thread
/// driving `driver` at `rate_hz`, until dropped.
pub struct UniverseOutput {
    buffer: Arc<Mutex<[u8; UNIVERSE_SIZE]>>,
    status: Arc<Mutex<UniverseStatus>>,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl UniverseOutput {
    pub fn spawn(
        universe_id: u32,
        mut driver: Box<dyn OutputDriver>,
        rate_hz: u32,
        blackout: Arc<AtomicBool>,
        app: AppHandle,
    ) -> Self {
        let buffer = Arc::new(Mutex::new([0u8; UNIVERSE_SIZE]));
        let status = Arc::new(Mutex::new(initial_status(universe_id)));
        let stop = Arc::new(AtomicBool::new(false));
        let frame_interval = Duration::from_secs_f64(1.0 / rate_hz.max(1) as f64);
        let realtime = driver.needs_realtime_priority();

        let thread_buffer = buffer.clone();
        let thread_status = status.clone();
        let thread_stop = stop.clone();

        let handle = thread::spawn(move || {
            // See dmx output thread rationale (BUILD_PLAN.md): a few ms of OS
            // scheduler jitter can land mid-frame and corrupt a hardware
            // break's timing. Only drivers that need it ask for this.
            #[cfg(windows)]
            if realtime {
                unsafe {
                    windows_sys::Win32::System::Threading::SetThreadPriority(
                        windows_sys::Win32::System::Threading::GetCurrentThread(),
                        windows_sys::Win32::System::Threading::THREAD_PRIORITY_TIME_CRITICAL,
                    );
                }
            }
            #[cfg(not(windows))]
            let _ = realtime;

            let mut last_connected: Option<bool> = None;
            let mut frame_count: u32 = 0;
            let mut window_start = Instant::now();

            while !thread_stop.load(Ordering::Relaxed) {
                let frame_start = Instant::now();

                let frame = if blackout.load(Ordering::Relaxed) {
                    [0u8; UNIVERSE_SIZE]
                } else {
                    *thread_buffer.lock().unwrap()
                };

                let result = driver.write_frame(&frame);
                let connected = result.is_ok();
                if last_connected != Some(connected) {
                    last_connected = Some(connected);
                    let status = UniverseStatus {
                        universe_id,
                        connected,
                        error: result.err(),
                        frames_per_sec: thread_status.lock().unwrap().frames_per_sec,
                    };
                    *thread_status.lock().unwrap() = status.clone();
                    let _ = app.emit(STATUS_EVENT, status);
                }

                frame_count += 1;
                let window_elapsed = window_start.elapsed();
                if window_elapsed >= Duration::from_secs(1) {
                    let status = UniverseStatus {
                        universe_id,
                        connected: last_connected.unwrap_or(false),
                        error: None,
                        frames_per_sec: frame_count as f32 / window_elapsed.as_secs_f32(),
                    };
                    *thread_status.lock().unwrap() = status.clone();
                    let _ = app.emit(STATUS_EVENT, status);
                    frame_count = 0;
                    window_start = Instant::now();
                }

                let elapsed = frame_start.elapsed();
                if elapsed < frame_interval {
                    thread::sleep(frame_interval - elapsed);
                }
            }
        });

        Self {
            buffer,
            status,
            stop,
            handle: Some(handle),
        }
    }

    pub fn set_frame(&self, data: [u8; UNIVERSE_SIZE]) {
        *self.buffer.lock().unwrap() = data;
    }

    pub fn get_frame(&self) -> [u8; UNIVERSE_SIZE] {
        *self.buffer.lock().unwrap()
    }

    pub fn status(&self) -> UniverseStatus {
        self.status.lock().unwrap().clone()
    }
}

impl Drop for UniverseOutput {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
