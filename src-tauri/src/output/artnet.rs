//! Art-Net 4 output (ArtDMX) and ArtPoll/ArtPollReply discovery.
//!
//! Spec reference: Art-Net 4 (Artistic Licence), the de facto standard also
//! implemented by QLC+/ETC/etc. Verified here by construction (byte-exact unit
//! tests) and a same-host UDP loopback test; real interop with a hardware/
//! software Art-Net node still needs an on-network check (HARDWARE_CHECKS.md).

use std::net::{ToSocketAddrs, UdpSocket};
use std::time::Duration;

use super::{OutputDriver, UNIVERSE_SIZE};

pub const ARTNET_PORT: u16 = 6454;
const ID: &[u8; 8] = b"Art-Net\0";
const OP_DMX: [u8; 2] = [0x00, 0x50]; // OpCode 0x5000, low byte first on the wire
const OP_POLL: [u8; 2] = [0x00, 0x20]; // OpCode 0x2000
const OP_POLL_REPLY: [u8; 2] = [0x00, 0x21]; // OpCode 0x2100
const PROT_VER: [u8; 2] = [0x00, 0x0e]; // ProtVerHi/Lo = 14, high byte first

/// Encodes an ArtDMX packet for `net`/`subnet`/`universe` (the 15-bit
/// Port-Address, split net:7 subnet:4 universe:4) carrying all 512 channels.
pub fn encode_artdmx(net: u8, subnet: u8, universe: u8, sequence: u8, data: &[u8; UNIVERSE_SIZE]) -> Vec<u8> {
    let mut packet = Vec::with_capacity(18 + UNIVERSE_SIZE);
    packet.extend_from_slice(ID);
    packet.extend_from_slice(&OP_DMX);
    packet.extend_from_slice(&PROT_VER);
    packet.push(sequence);
    packet.push(0); // Physical (informational input port)
    packet.push(((subnet & 0x0F) << 4) | (universe & 0x0F)); // SubUni
    packet.push(net & 0x7F); // Net
    let len = UNIVERSE_SIZE as u16;
    packet.push((len >> 8) as u8);
    packet.push((len & 0xFF) as u8);
    packet.extend_from_slice(data);
    packet
}

/// Decodes an ArtDMX packet into `(net, subnet, universe, data)`. Returns
/// `None` for anything that isn't a well-formed ArtDMX packet carrying a full
/// 512-channel universe.
pub fn decode_artdmx(packet: &[u8]) -> Option<(u8, u8, u8, [u8; UNIVERSE_SIZE])> {
    if packet.len() < 18 + UNIVERSE_SIZE || &packet[0..8] != ID || packet[8..10] != OP_DMX {
        return None;
    }
    let sub_uni = packet[14];
    let net = packet[15] & 0x7F;
    let subnet = (sub_uni >> 4) & 0x0F;
    let universe = sub_uni & 0x0F;
    let len = u16::from_be_bytes([packet[16], packet[17]]) as usize;
    if len < UNIVERSE_SIZE || packet.len() < 18 + len {
        return None;
    }
    let mut data = [0u8; UNIVERSE_SIZE];
    data.copy_from_slice(&packet[18..18 + UNIVERSE_SIZE]);
    Some((net, subnet, universe, data))
}

pub fn encode_artpoll() -> Vec<u8> {
    let mut packet = Vec::with_capacity(14);
    packet.extend_from_slice(ID);
    packet.extend_from_slice(&OP_POLL);
    packet.extend_from_slice(&PROT_VER);
    packet.push(0x00); // TalkToMe: no extra diagnostics/reply behaviour requested
    packet.push(0x00); // Priority: report all
    packet
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArtPollReply {
    pub ip: [u8; 4],
    pub short_name: String,
    pub long_name: String,
}

/// Parses the fields we care about out of an ArtPollReply. Returns `None` for
/// anything that isn't a long-enough Art-Net ArtPollReply packet.
pub fn decode_artpoll_reply(buf: &[u8]) -> Option<ArtPollReply> {
    if buf.len() < 108 || &buf[0..8] != ID || buf[8..10] != OP_POLL_REPLY {
        return None;
    }
    let ip = [buf[10], buf[11], buf[12], buf[13]];
    let short_name = read_c_string(&buf[26..44]);
    let long_name = read_c_string(&buf[44..108]);
    Some(ArtPollReply {
        ip,
        short_name,
        long_name,
    })
}

fn read_c_string(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// Broadcasts an ArtPoll and collects ArtPollReply packets for `timeout`.
/// Best-effort discovery for the Setup UI; never blocks output (runs on its
/// own short-lived socket, not the per-universe output thread).
pub fn discover_nodes(timeout: Duration) -> Result<Vec<ArtPollReply>, String> {
    let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| e.to_string())?;
    socket.set_broadcast(true).map_err(|e| e.to_string())?;
    socket
        .set_read_timeout(Some(timeout))
        .map_err(|e| e.to_string())?;
    socket
        .send_to(&encode_artpoll(), ("255.255.255.255", ARTNET_PORT))
        .map_err(|e| e.to_string())?;

    let mut nodes = Vec::new();
    let mut buf = [0u8; 1024];
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        match socket.recv_from(&mut buf) {
            Ok((n, _)) => {
                if let Some(reply) = decode_artpoll_reply(&buf[..n]) {
                    nodes.push(reply);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock || e.kind() == std::io::ErrorKind::TimedOut => {
                break;
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(nodes)
}

pub struct ArtNetDriver {
    socket: UdpSocket,
    destination: std::net::SocketAddr,
    net: u8,
    subnet: u8,
    universe: u8,
    sequence: u8,
}

impl ArtNetDriver {
    pub fn new(destination: &str, net: u8, subnet: u8, universe: u8) -> Result<Self, String> {
        let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| e.to_string())?;
        socket.set_broadcast(true).map_err(|e| e.to_string())?;
        let destination = (destination, ARTNET_PORT)
            .to_socket_addrs()
            .map_err(|e| format!("Invalid Art-Net destination {destination}: {e}"))?
            .next()
            .ok_or_else(|| format!("Invalid Art-Net destination {destination}"))?;
        Ok(Self {
            socket,
            destination,
            net,
            subnet,
            universe,
            sequence: 0,
        })
    }
}

impl OutputDriver for ArtNetDriver {
    fn write_frame(&mut self, data: &[u8; UNIVERSE_SIZE]) -> Result<(), String> {
        // Sequence 0 means "sequencing disabled" per spec, so wrap 255 -> 1, not -> 0.
        self.sequence = if self.sequence == 255 { 1 } else { self.sequence + 1 };
        let packet = encode_artdmx(self.net, self.subnet, self.universe, self.sequence, data);
        self.socket
            .send_to(&packet, self.destination)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artdmx_header_matches_spec() {
        let data = [0u8; UNIVERSE_SIZE];
        let packet = encode_artdmx(3, 0xA, 0x5, 7, &data);

        assert_eq!(&packet[0..8], b"Art-Net\0");
        assert_eq!(&packet[8..10], &[0x00, 0x50]); // OpDmx
        assert_eq!(&packet[10..12], &[0x00, 0x0e]); // ProtVer 14
        assert_eq!(packet[12], 7); // Sequence
        assert_eq!(packet[13], 0); // Physical
        assert_eq!(packet[14], 0xA5); // SubUni = subnet<<4 | universe
        assert_eq!(packet[15], 3); // Net
        assert_eq!(&packet[16..18], &[0x02, 0x00]); // Length = 512
        assert_eq!(packet.len(), 18 + UNIVERSE_SIZE);
        assert_eq!(&packet[18..], &data[..]);
    }

    #[test]
    fn artpoll_is_well_formed() {
        let packet = encode_artpoll();
        assert_eq!(&packet[0..8], b"Art-Net\0");
        assert_eq!(&packet[8..10], &[0x00, 0x20]);
        assert_eq!(packet.len(), 14);
    }

    #[test]
    fn decodes_a_hand_built_artpoll_reply() {
        let mut buf = vec![0u8; 108];
        buf[0..8].copy_from_slice(b"Art-Net\0");
        buf[8..10].copy_from_slice(&[0x00, 0x21]);
        buf[10..14].copy_from_slice(&[10, 0, 0, 50]);
        buf[26..26 + 6].copy_from_slice(b"Node1\0");
        buf[44..44 + 11].copy_from_slice(b"Long Name\0\0");

        let reply = decode_artpoll_reply(&buf).expect("should parse");
        assert_eq!(reply.ip, [10, 0, 0, 50]);
        assert_eq!(reply.short_name, "Node1");
        assert_eq!(reply.long_name, "Long Name");
    }

    #[test]
    fn rejects_non_artnet_or_short_packets() {
        assert!(decode_artpoll_reply(&[0u8; 10]).is_none());
        let mut wrong_id = vec![0u8; 108];
        wrong_id[0..8].copy_from_slice(b"NotArt\0\0");
        assert!(decode_artpoll_reply(&wrong_id).is_none());
    }

    #[test]
    fn loopback_send_receive_round_trips_the_frame() {
        let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
        receiver.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let receiver_addr = receiver.local_addr().unwrap();

        let mut driver = ArtNetDriver {
            socket: UdpSocket::bind("127.0.0.1:0").unwrap(),
            destination: receiver_addr,
            net: 0,
            subnet: 0,
            universe: 1,
            sequence: 0,
        };

        let mut data = [0u8; UNIVERSE_SIZE];
        data[0] = 255;
        data[1] = 128;
        driver.write_frame(&data).unwrap();

        let mut buf = [0u8; 1024];
        let (n, _) = receiver.recv_from(&mut buf).unwrap();
        let (net, subnet, universe, received) = decode_artdmx(&buf[..n]).expect("should decode");
        assert_eq!((net, subnet, universe), (0, 0, 1));
        assert_eq!(received, data);
    }

    #[test]
    fn decode_artdmx_round_trips_encode_artdmx() {
        let mut data = [0u8; UNIVERSE_SIZE];
        data[10] = 200;
        data[500] = 5;
        let packet = encode_artdmx(3, 0xA, 0x5, 1, &data);

        let (net, subnet, universe, decoded) = decode_artdmx(&packet).expect("should decode");
        assert_eq!((net, subnet, universe), (3, 0xA, 0x5));
        assert_eq!(decoded, data);
    }

    #[test]
    fn decode_artdmx_rejects_wrong_opcode_and_short_packets() {
        assert!(decode_artdmx(&[0u8; 10]).is_none());
        let mut wrong_op = encode_artdmx(0, 0, 0, 1, &[0u8; UNIVERSE_SIZE]);
        wrong_op[8] = 0xFF;
        assert!(decode_artdmx(&wrong_op).is_none());
    }
}
