use std::{fmt::Debug, any::{Any, type_name}};

use crate::{ProtocolResult};

use super::{Packet, PacketDirection};

#[derive(Default)]
pub struct UnknownPacket {
    packet_id: u32,
    direction: PacketDirection,
    payload: Vec<u8>
}

impl UnknownPacket {
    pub fn new_with_capacity(direction: PacketDirection, packet_id: u32, capacity: usize) -> Self {
        Self {
            packet_id,
            direction,
            payload: Vec::with_capacity(capacity)
        }
    }

    pub fn new(direction: PacketDirection, packet_id: u32, payload: Vec<u8>) -> Self {
        Self { direction, packet_id, payload }
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    } 
}

impl Debug for UnknownPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnknownPacket")
            .field("direction", &self.direction)
            .field("packet_id", &self.packet_id)
            .field("payload_length", &self.payload.len())
            .finish()
    }
}

impl Packet for UnknownPacket {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn packet_name(&self) -> &str {
        type_name::<Self>()
    }

    fn packet_id(&self) -> u32 {
        self.packet_id
    }

    fn direction(&self) -> super::PacketDirection {
        todo!()
    }

    fn model_id(&self) -> u32 {
        (-1i32) as u32
    }

    fn encode(&self, writer: &mut dyn std::io::Write) -> ProtocolResult<()> {
        writer.write_all(&self.payload)?;
        Ok(())
    }

    fn decode(&mut self, reader: &mut dyn std::io::Read) -> ProtocolResult<()> {
        reader.read_to_end(&mut self.payload)?;
        Ok(())
    }
}