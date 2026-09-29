//! DMX input receivers (Art-Net, sACN). Decodes incoming packets into
//! per-universe frames the frontend can read (e.g. a future DMX input
//! monitor, per BUILD_PLAN Phase 12).
//!
//! **Scope note (Phase 2):** this receives and decodes input but does not yet
//! merge it into any universe's live output. BUILD_PLAN Phase 2 asks for
//! input "with a merge option"; Phase 3 is where the real HTP/LTP merge
//! engine gets built, and bolting a second, throwaway merge implementation
//! onto the output runner now — the same code path the safety-critical FTDI
//! driver runs through — was judged not worth the risk this late in the
//! phase. See DECISIONS.md.

use std::collections::HashMap;
use std::net::{Ipv4Addr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use socket2::{Domain, Socket, Type};

use super::artnet::{decode_artdmx, ARTNET_PORT};
use super::sacn::{decode_e131, multicast_group, SACN_PORT};
use super::UNIVERSE_SIZE;

type ArtNetKey = (u8, u8, u8);

struct SacnListener {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

pub struct DmxInputManager {
    artnet_frames: Arc<Mutex<HashMap<ArtNetKey, [u8; UNIVERSE_SIZE]>>>,
    artnet_thread: Mutex<Option<JoinHandle<()>>>,
    sacn_frames: Arc<Mutex<HashMap<u16, [u8; UNIVERSE_SIZE]>>>,
    sacn_listeners: Mutex<HashMap<u16, SacnListener>>,
}

impl Default for DmxInputManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DmxInputManager {
    pub fn new() -> Self {
        Self {
            artnet_frames: Arc::new(Mutex::new(HashMap::new())),
            artnet_thread: Mutex::new(None),
            sacn_frames: Arc::new(Mutex::new(HashMap::new())),
            sacn_listeners: Mutex::new(HashMap::new()),
        }
    }

    /// Starts the shared Art-Net input listener (idempotent — one listener
    /// serves every net/subnet/universe, since they all arrive on the same
    /// port and are demultiplexed by the packet's own address fields).
    pub fn start_artnet_listener(&self) -> Result<(), String> {
        let mut slot = self.artnet_thread.lock().unwrap();
        if slot.is_some() {
            return Ok(());
        }

        let socket = UdpSocket::bind(("0.0.0.0", ARTNET_PORT))
            .map_err(|e| format!("Failed to bind Art-Net input port {ARTNET_PORT}: {e}"))?;
        let frames = self.artnet_frames.clone();

        let handle = thread::spawn(move || {
            let mut buf = [0u8; 1024];
            while let Ok((n, _)) = socket.recv_from(&mut buf) {
                if let Some((net, subnet, universe, data)) = decode_artdmx(&buf[..n]) {
                    frames.lock().unwrap().insert((net, subnet, universe), data);
                }
            }
        });
        *slot = Some(handle);
        Ok(())
    }

    pub fn artnet_frame(&self, net: u8, subnet: u8, universe: u8) -> Option<[u8; UNIVERSE_SIZE]> {
        self.artnet_frames.lock().unwrap().get(&(net, subnet, universe)).copied()
    }

    /// Starts (or reuses) a multicast listener for one sACN universe. Uses
    /// SO_REUSEADDR so multiple universes' listeners can share port 5568, as
    /// any real sACN receiver needs to.
    pub fn start_sacn_listener(&self, universe: u16) -> Result<(), String> {
        if self.sacn_listeners.lock().unwrap().contains_key(&universe) {
            return Ok(());
        }

        let socket = bind_sacn_socket(universe)?;
        socket
            .set_read_timeout(Some(Duration::from_millis(500)))
            .map_err(|e| e.to_string())?;

        let frames = self.sacn_frames.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();

        let handle = thread::spawn(move || {
            let mut buf = [0u8; 2048];
            while !thread_stop.load(Ordering::Relaxed) {
                match socket.recv_from(&mut buf) {
                    Ok((n, _)) => {
                        if let Some((pkt_universe, data)) = decode_e131(&buf[..n]) {
                            if pkt_universe == universe {
                                frames.lock().unwrap().insert(universe, data);
                            }
                        }
                    }
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {}
                    Err(_) => break,
                }
            }
        });

        self.sacn_listeners.lock().unwrap().insert(universe, SacnListener { stop, handle });
        Ok(())
    }

    pub fn stop_sacn_listener(&self, universe: u16) {
        self.sacn_frames.lock().unwrap().remove(&universe);
        if let Some(listener) = self.sacn_listeners.lock().unwrap().remove(&universe) {
            listener.stop.store(true, Ordering::Relaxed);
            let _ = listener.handle.join();
        }
    }

    pub fn sacn_frame(&self, universe: u16) -> Option<[u8; UNIVERSE_SIZE]> {
        self.sacn_frames.lock().unwrap().get(&universe).copied()
    }
}

fn bind_sacn_socket(universe: u16) -> Result<UdpSocket, String> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, None).map_err(|e| e.to_string())?;
    socket.set_reuse_address(true).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    socket.set_reuse_port(true).map_err(|e| e.to_string())?;
    let addr: std::net::SocketAddr = (Ipv4Addr::UNSPECIFIED, SACN_PORT).into();
    socket.bind(&addr.into()).map_err(|e| e.to_string())?;

    let socket: UdpSocket = socket.into();
    socket
        .join_multicast_v4(&multicast_group(universe), &Ipv4Addr::UNSPECIFIED)
        .map_err(|e| format!("Failed to join sACN multicast group for universe {universe}: {e}"))?;
    Ok(socket)
}
