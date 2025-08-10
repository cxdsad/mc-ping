use std::time::Duration;
use anyhow::Context;
use tokio::time::sleep;
use crate::mc_text::ServerStatus;
use crate::varint::VarInt;

/// Represents the Minecraft client handshake packet.
///
/// This packet initiates the handshake with the server before status or login requests.
///
/// # Example
/// ```
/// let handshake = ClientHandshake::new("127.0.0.1".to_string(), 25565);
/// let bytes = handshake.to_bytes();
/// ```
#[derive(Debug)]
pub struct ClientHandshake {
    /// Length of the entire packet, encoded as a VarInt.
    pub len: VarInt,
    /// Packet ID (0x00 for handshake).
    pub packet_id: VarInt,
    /// Protocol version number, e.g., 768 for Minecraft 1.21.
    pub protocol_version: VarInt,
    /// Server address as a string (domain or IP).
    pub server_addr: String,
    /// Server port number.
    pub server_port: u16,
    /// Next state after handshake: 1 = status, 2 = login.
    pub next_state: VarInt,
}

impl ClientHandshake {
    /// Creates a new ClientHandshake packet for the given server address and port.
    ///
    /// Automatically calculates packet length and uses default protocol version 768.
    pub fn new(server_addr: String, server_port: u16) -> ClientHandshake {
        let packet_id = VarInt::from(0x00);
        let protocol_version = VarInt::from(768);
        let next_state = VarInt::from(1);

        // Calculate length of the packet payload:
        // packet_id + protocol_version + length of server_addr string + server_addr bytes + port(2 bytes) + next_state
        let len_val =
            packet_id.size() +
                protocol_version.size() +
                VarInt::from(server_addr.len() as i32).size() +
                server_addr.len() +
                2 +  // server_port is 2 bytes
                next_state.size();

        let len = VarInt::from(len_val as i32);

        let handshake = ClientHandshake {
            len,
            packet_id,
            protocol_version,
            server_addr,
            server_port,
            next_state,
        };


        handshake
    }

    /// Serializes the handshake packet into a byte vector ready for sending over the network.
    ///
    /// The format follows Minecraft's VarInt and packet structure conventions.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();

        // Helper function to write VarInt bytes until continuation bit is zero.
        fn write_varint_bytes(buf: &mut Vec<u8>, varint_inner: &[u8]) {
            for &byte in varint_inner {
                buf.push(byte);
                if byte & 0b1000_0000 == 0 {
                    break;
                }
            }
        }

        // Write packet length
        write_varint_bytes(&mut buf, &self.len.inner);
        // Write packet ID
        write_varint_bytes(&mut buf, &self.packet_id.inner);
        // Write protocol version
        write_varint_bytes(&mut buf, &self.protocol_version.inner);

        // Write server address as Minecraft String: VarInt length + UTF-8 bytes
        let addr_len = VarInt::from(self.server_addr.len() as i32);
        write_varint_bytes(&mut buf, &addr_len.inner);
        buf.extend(self.server_addr.as_bytes());

        // Write server port as 2 bytes big-endian
        buf.push((self.server_port >> 8) as u8);
        buf.push(self.server_port as u8);

        // Write next state VarInt
        write_varint_bytes(&mut buf, &self.next_state.inner);

        buf
    }
}

/// Represents the status query packet.
///
/// This packet is sent after handshake to request the server status.
pub struct StatusQuery {
    len: VarInt,
    packet_id: VarInt,
}

impl StatusQuery {
    /// Creates a new status query packet.
    ///
    /// # Example
    /// ```
    /// let query = StatusQuery::new();
    /// let bytes = query.to_bytes();
    /// ```
    pub fn new() -> StatusQuery {
        let packet_id = VarInt::from(0x00);
        let len = VarInt::from(1); // Packet length: 1 byte for packet_id
        StatusQuery {
            len,
            packet_id,
        }
    }

    /// Returns the serialized bytes of the status query packet.
    ///
    /// This packet is always 2 bytes: [0x01, 0x00]
    pub fn to_bytes(&self) -> Vec<u8> {
        vec![0x01, 0x00]
    }
}

/// Represents the server's response to a status query.
///
/// Contains the raw JSON string with server information.
#[derive(Debug)]
pub struct ServerQueryResponse {
    /// Length of the entire response packet.
    pub len: VarInt,
    /// Packet ID (should be 0x00).
    pub packet_id: VarInt,
    /// Length of the JSON string.
    pub json_len: VarInt,
    /// JSON string with server status information.
    pub json: String,
}

impl ServerQueryResponse {
    /// Parses a ServerQueryResponse from raw bytes.
    ///
    /// Reads VarInts for lengths and packet IDs, then extracts the JSON string.
    ///
    /// # Panics
    /// If the byte slice is too short or malformed, this may panic.
    pub async fn from(bytes: &[u8]) -> ServerQueryResponse {
        sleep(Duration::from_millis(100)).await; // panic fix
        // Helper to read a VarInt from a byte slice,
        fn read_varint(data: &[u8]) -> (VarInt, usize) {
            let mut val = VarInt::default();
            let mut i = 0;
            loop {
                let byte = data[i];
                val.inner[i] = byte;
                i += 1;
                if byte & 0x80 == 0 {
                    break;
                }
            }
            (val, i)
        }

        let mut cursor = 0;

        // 1. Read length VarInt
        let (len, len_size) = read_varint(&bytes[cursor..]);
        cursor += len_size;

        // 2. Read packet_id VarInt
        let (packet_id, packet_id_size) = read_varint(&bytes[cursor..]);
        cursor += packet_id_size;

        // 3. Read json_len VarInt
        let (json_len, json_len_size) = read_varint(&bytes[cursor..]);
        cursor += json_len_size;

        // 4. Read JSON bytes using length from json_len
        let json_bytes = &bytes[cursor..cursor + i32::from(json_len.clone()) as usize];
        cursor += i32::from(json_len.clone()) as usize;

        let json = String::from_utf8_lossy(json_bytes).to_string();

        ServerQueryResponse {
            len,
            packet_id,
            json_len,
            json,
        }
    }

    /// Parses the JSON string into a strongly-typed ServerStatus struct.
    ///
    /// Returns an error if JSON deserialization fails.
    ///
    /// # Example
    /// ```
    /// let response = ServerQueryResponse::from(&bytes);
    /// let status = response.parse_status()?;
    /// ```
    pub fn parse_status(&self) -> anyhow::Result<ServerStatus> {
        let status: ServerStatus = serde_json::from_str(&self.json)
            .context("Failed to deserialize ServerQueryResponse.json into ServerStatus")?;
        Ok(status)
    }
}
