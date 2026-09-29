//! sACN / E1.31 output (ANSI E1.31 streaming ACN data packet).
//!
//! Byte layout verified by construction (unit tests below assert the known
//! 638-byte total for a full 512-channel universe and each layer's fields);
//! real interop with an sACN receiver/viewer still needs an on-network check
//! (HARDWARE_CHECKS.md).

use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};

use super::config::SacnDestination;
use super::{OutputDriver, UNIVERSE_SIZE};

pub const SACN_PORT: u16 = 5568;
const ACN_ID: [u8; 12] = *b"ASC-E1.17\0\0\0";
const VECTOR_ROOT_E131_DATA: [u8; 4] = [0x00, 0x00, 0x00, 0x04];
const VECTOR_E131_DATA_PACKET: [u8; 4] = [0x00, 0x00, 0x00, 0x02];
const VECTOR_DMP_SET_PROPERTY: u8 = 0x02;

fn flags_and_length(length: u16) -> [u8; 2] {
    (0x7000 | (length & 0x0FFF)).to_be_bytes()
}

/// Standard sACN multicast group for a universe: 239.255.hi(universe).lo(universe).
pub fn multicast_group(universe: u16) -> std::net::Ipv4Addr {
    let [hi, lo] = universe.to_be_bytes();
    std::net::Ipv4Addr::new(239, 255, hi, lo)
}

pub fn encode_e131(
    cid: [u8; 16],
    source_name: &str,
    priority: u8,
    sequence: u8,
    universe: u16,
    data: &[u8; UNIVERSE_SIZE],
) -> Vec<u8> {
    let prop_value_count: u16 = 1 + UNIVERSE_SIZE as u16; // DMX start code + 512 channels

    // DMP layer
    let dmp_length: u16 = 2 + 1 + 1 + 2 + 2 + 2 + prop_value_count;
    let mut dmp = Vec::with_capacity(dmp_length as usize);
    dmp.extend_from_slice(&flags_and_length(dmp_length));
    dmp.push(VECTOR_DMP_SET_PROPERTY);
    dmp.push(0xa1); // Address Type & Data Type
    dmp.extend_from_slice(&0u16.to_be_bytes()); // First Property Address
    dmp.extend_from_slice(&1u16.to_be_bytes()); // Address Increment
    dmp.extend_from_slice(&prop_value_count.to_be_bytes());
    dmp.push(0x00); // DMX start code
    dmp.extend_from_slice(data);

    // Framing layer
    let framing_length: u16 = 2 + 4 + 64 + 1 + 2 + 1 + 1 + 2 + dmp_length;
    let mut framing = Vec::with_capacity(framing_length as usize);
    framing.extend_from_slice(&flags_and_length(framing_length));
    framing.extend_from_slice(&VECTOR_E131_DATA_PACKET);
    let mut name_bytes = [0u8; 64];
    let src = source_name.as_bytes();
    let copy_len = src.len().min(63);
    name_bytes[..copy_len].copy_from_slice(&src[..copy_len]);
    framing.extend_from_slice(&name_bytes);
    framing.push(priority);
    framing.extend_from_slice(&0u16.to_be_bytes()); // Synchronization Address (unused)
    framing.push(sequence);
    framing.push(0x00); // Options
    framing.extend_from_slice(&universe.to_be_bytes());
    framing.extend_from_slice(&dmp);

    // Root layer
    let root_length: u16 = 2 + 4 + 16 + framing_length;
    let mut root = Vec::with_capacity(root_length as usize);
    root.extend_from_slice(&flags_and_length(root_length));
    root.extend_from_slice(&VECTOR_ROOT_E131_DATA);
    root.extend_from_slice(&cid);
    root.extend_from_slice(&framing);

    let mut packet = Vec::with_capacity(4 + ACN_ID.len() + root.len());
    packet.extend_from_slice(&0x0010u16.to_be_bytes()); // Preamble size
    packet.extend_from_slice(&0x0000u16.to_be_bytes()); // Postamble size
    packet.extend_from_slice(&ACN_ID);
    packet.extend_from_slice(&root);
    packet
}

/// Decodes an E1.31 data packet carrying a full 512-channel universe into
/// `(universe, data)`. Returns `None` for anything shorter than the standard
/// 638-byte full-universe packet or that doesn't match the expected root/
/// framing/DMP vectors at their fixed offsets.
pub fn decode_e131(packet: &[u8]) -> Option<(u16, [u8; UNIVERSE_SIZE])> {
    const FRAMING_START: usize = 38;
    const UNIVERSE_OFFSET: usize = FRAMING_START + 2 + 4 + 64 + 1 + 2 + 1 + 1; // 113
    const DMP_START: usize = FRAMING_START + 77; // 115
    const DMP_VECTOR_OFFSET: usize = DMP_START + 2; // 117
    const START_CODE_OFFSET: usize = DMP_START + 10; // 125

    if packet.len() < START_CODE_OFFSET + 1 + UNIVERSE_SIZE {
        return None;
    }
    if packet[4..16] != ACN_ID {
        return None;
    }
    if packet[18..22] != VECTOR_ROOT_E131_DATA {
        return None;
    }
    if packet[FRAMING_START + 2..FRAMING_START + 6] != VECTOR_E131_DATA_PACKET {
        return None;
    }
    if packet[DMP_VECTOR_OFFSET] != VECTOR_DMP_SET_PROPERTY {
        return None;
    }

    let universe = u16::from_be_bytes([packet[UNIVERSE_OFFSET], packet[UNIVERSE_OFFSET + 1]]);
    let mut data = [0u8; UNIVERSE_SIZE];
    data.copy_from_slice(&packet[START_CODE_OFFSET + 1..START_CODE_OFFSET + 1 + UNIVERSE_SIZE]);
    Some((universe, data))
}

pub struct SacnDriver {
    socket: UdpSocket,
    destination: SocketAddr,
    cid: [u8; 16],
    source_name: String,
    universe: u16,
    priority: u8,
    sequence: u8,
}

impl SacnDriver {
    pub fn new(destination: &SacnDestination, universe: u16, priority: u8) -> Result<Self, String> {
        let socket = UdpSocket::bind("0.0.0.0:0").map_err(|e| e.to_string())?;
        let addr = match destination {
            SacnDestination::Multicast => multicast_group(universe).to_string(),
            SacnDestination::Unicast { address } => address.clone(),
        };
        let destination = (addr.as_str(), SACN_PORT)
            .to_socket_addrs()
            .map_err(|e| format!("Invalid sACN destination {addr}: {e}"))?
            .next()
            .ok_or_else(|| format!("Invalid sACN destination {addr}"))?;

        Ok(Self {
            socket,
            destination,
            cid: *uuid::Uuid::new_v4().as_bytes(),
            source_name: "LumenDMX".to_string(),
            universe,
            priority,
            sequence: 0,
        })
    }
}

impl OutputDriver for SacnDriver {
    fn write_frame(&mut self, data: &[u8; UNIVERSE_SIZE]) -> Result<(), String> {
        self.sequence = self.sequence.wrapping_add(1);
        let packet = encode_e131(
            self.cid,
            &self.source_name,
            self.priority,
            self.sequence,
            self.universe,
            data,
        );
        self.socket
            .send_to(&packet, self.destination)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn full_universe_packet_is_638_bytes() {
        let data = [0u8; UNIVERSE_SIZE];
        let packet = encode_e131([0u8; 16], "LumenDMX", 100, 1, 1, &data);
        assert_eq!(packet.len(), 638);
    }

    #[test]
    fn header_fields_match_spec() {
        let data = [0u8; UNIVERSE_SIZE];
        let packet = encode_e131([7u8; 16], "Src", 200, 42, 5, &data);

        assert_eq!(&packet[0..2], &0x0010u16.to_be_bytes()); // preamble
        assert_eq!(&packet[2..4], &0x0000u16.to_be_bytes()); // postamble
        assert_eq!(&packet[4..16], b"ASC-E1.17\0\0\0");
        assert_eq!(&packet[16..18], &flags_and_length(622));
        assert_eq!(&packet[18..22], &VECTOR_ROOT_E131_DATA);
        assert_eq!(&packet[22..38], &[7u8; 16]); // CID

        // Framing layer starts at 38
        assert_eq!(&packet[38..40], &flags_and_length(600));
        assert_eq!(&packet[40..44], &VECTOR_E131_DATA_PACKET);
        assert_eq!(&packet[44..47], b"Src"); // source name (null-padded)
        let priority = packet[44 + 64];
        assert_eq!(priority, 200);
        let sequence = packet[44 + 64 + 1 + 2];
        assert_eq!(sequence, 42);
        let universe_bytes = &packet[44 + 64 + 1 + 2 + 1 + 1..44 + 64 + 1 + 2 + 1 + 1 + 2];
        assert_eq!(universe_bytes, &5u16.to_be_bytes());
    }

    #[test]
    fn dmp_layer_carries_start_code_and_all_channels() {
        let mut data = [0u8; UNIVERSE_SIZE];
        data[0] = 255;
        data[511] = 42;
        let packet = encode_e131([0u8; 16], "LumenDMX", 100, 1, 1, &data);

        // DMP layer begins at byte 38 (framing) + 77 (framing header) = 115.
        let dmp_start = 115;
        assert_eq!(packet[dmp_start + 2], VECTOR_DMP_SET_PROPERTY);
        let start_code_offset = dmp_start + 10;
        assert_eq!(packet[start_code_offset], 0x00); // DMX start code
        assert_eq!(packet[start_code_offset + 1], 255); // channel 1
        assert_eq!(packet[start_code_offset + 512], 42); // channel 512
    }

    #[test]
    fn decode_e131_round_trips_encode_e131() {
        let mut data = [0u8; UNIVERSE_SIZE];
        data[0] = 1;
        data[300] = 88;
        data[511] = 255;
        let packet = encode_e131([9u8; 16], "LumenDMX", 150, 7, 42, &data);

        let (universe, decoded) = decode_e131(&packet).expect("should decode");
        assert_eq!(universe, 42);
        assert_eq!(decoded, data);
    }

    #[test]
    fn decode_e131_rejects_short_or_malformed_packets() {
        assert!(decode_e131(&[0u8; 20]).is_none());
        let mut bad_id = encode_e131([0u8; 16], "X", 100, 1, 1, &[0u8; UNIVERSE_SIZE]);
        bad_id[4] = 0xFF;
        assert!(decode_e131(&bad_id).is_none());
    }

    #[test]
    fn multicast_group_follows_239_255_hi_lo() {
        assert_eq!(multicast_group(1).to_string(), "239.255.0.1");
        assert_eq!(multicast_group(63999).to_string(), "239.255.249.255");
    }

    #[test]
    fn loopback_send_receive_round_trips_the_frame() {
        let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
        receiver.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut driver = SacnDriver {
            socket: UdpSocket::bind("127.0.0.1:0").unwrap(),
            destination: receiver.local_addr().unwrap(),
            cid: [1u8; 16],
            source_name: "Test".to_string(),
            universe: 1,
            priority: 100,
            sequence: 0,
        };

        let mut data = [0u8; UNIVERSE_SIZE];
        data[0] = 200;
        driver.write_frame(&data).unwrap();

        let mut buf = [0u8; 1024];
        let (n, _) = receiver.recv_from(&mut buf).unwrap();
        assert_eq!(n, 638);
        assert_eq!(buf[4..16], ACN_ID);
    }
}
