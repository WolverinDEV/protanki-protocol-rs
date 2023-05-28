use crate::{ProtocolResult, ProtocolError};
use std::{io::Read, any::type_name, collections::BTreeMap};

use super::{Packet, UnknownPacket, PacketDirection};


trait RegisteredPacket : Send {
    fn name(&self) -> &str;
    fn decode(&self, reader: &mut dyn Read) -> ProtocolResult<Box<dyn Packet>>;
}

struct RegisteredPacketImpl<T: Packet + Send + 'static> {
    _instance: T
}

impl<T: Packet + Default + Send + 'static> RegisteredPacket for RegisteredPacketImpl<T> {
    fn name(self: &Self) -> &str {
        type_name::<T>()
    }

    fn decode(self: &Self, reader: &mut dyn Read) -> ProtocolResult<Box<dyn Packet>> {
        let mut packet = Box::<T>::default();
        packet.decode(reader)?;
        Ok(packet)
    }
}

pub struct PacketRegistry {
    packets: BTreeMap<u32, Box<dyn RegisteredPacket>>,
    decode_as_unknown: bool,
}

impl PacketRegistry {
    pub fn new() -> Self {
        Self {
            packets: Default::default(),
            decode_as_unknown: Default::default(),
        }
    }

    pub fn allow_unknown_packets(&mut self) {
        self.decode_as_unknown = true
    }

    pub fn register_packet<T: Packet + Default + Send + 'static>(&mut self) {
        let instance = T::default();

        let packet_id = instance.packet_id();
        if let Some(_) = self.packets.insert(packet_id, Box::new(RegisteredPacketImpl{
            _instance: instance
        })) {
            panic!("tried to register {} twice", packet_id);
        }
    }

    pub fn decode(&self, reader: &mut dyn Read, packet_id: u32) -> ProtocolResult<Box<dyn Packet>> {
        let registered_packet = match self.packets.get(&packet_id) {
            Some(registered_packet) => registered_packet,
            None => {
                return if self.decode_as_unknown {
                    // TODO(mh): Is it possible to pass the total packet length or use a sized Reader?
                    let mut packet = Box::new(UnknownPacket::new_with_capacity(PacketDirection::Unknown, packet_id, 1024));
                    packet.decode(reader)?;
                    Ok(packet)
                } else {
                    Err(ProtocolError::PacketUnknownId(packet_id as i32))
                }
            },
        };

        Ok(registered_packet.decode(reader)?)
    }
}