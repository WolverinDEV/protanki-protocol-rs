use std::{any::{Any, type_name}, io::{Write, Read}, sync::Arc, collections::BTreeMap, fmt::Debug};

use crate::codec::{CodecRegistry, Codec};

pub trait Packet : Debug + Send {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;

    fn packet_name(&self) -> &str;
    fn packet_id(&self) -> u32;
    fn model_id(&self) -> u32;
}

pub trait PacketDowncast {
    fn downcast_ref<T: 'static>(&self) -> Option<&T>;
    fn is_type<T: 'static>(&self) -> bool;
}
impl PacketDowncast for dyn Packet + '_ {
    fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }

    fn is_type<T: 'static>(&self) -> bool {
        self.downcast_ref::<T>().is_some()
    }
}


trait RegisteredPacket : Send {
    fn name(self: &Self) -> &str;
    fn encode(self: &Self, registry: &CodecRegistry, writer: &mut dyn Write, packet: &dyn Packet) -> anyhow::Result<()>;
    fn decode(self: &Self, registry: &CodecRegistry, reader: &mut dyn Read) -> anyhow::Result<Box<dyn Packet>>;
}

struct RegisteredPacketImpl<T: Packet + Codec<Target = T> + Send + 'static> {
    codec: Arc<dyn Codec<Target = T>>
}

impl<T: Packet + Codec<Target = T> + Send + 'static> RegisteredPacket for RegisteredPacketImpl<T> {
    fn name(self: &Self) -> &str {
        type_name::<T>()
    }

    fn encode(self: &Self, registry: &CodecRegistry, writer: &mut dyn Write, packet: &dyn Packet) -> anyhow::Result<()> {
        let packet = match packet.as_any().downcast_ref::<T>() {
            Some(packet) => packet,
            None => anyhow::bail!("packet does not match expected packet")
        };

        self.codec.encode(registry, writer, packet)
    }

    fn decode(self: &Self, registry: &CodecRegistry, reader: &mut dyn Read) -> anyhow::Result<Box<dyn Packet>> {
        let packet = self.codec.decode(registry, reader)?;
        Ok(Box::new(packet))
    }
}

#[derive(Default)]
pub struct PacketRegistry {
    codec_registry: CodecRegistry,
    packet_codecs: BTreeMap<u32, Box<dyn RegisteredPacket>>,
}

impl PacketRegistry {
    pub fn new() -> Self {
        let mut instance = PacketRegistry {
            ..Default::default()
        };

        instance.codec_registry.register_primatives();
        instance
    }

    pub fn register_packet<T: Packet + Codec<Target = T> + 'static>(&mut self, packet: T) {
        let packet_id = packet.packet_id();
        let codec = self.codec_registry.register_codec(packet);

        if let Some(_) = self.packet_codecs.insert(packet_id, Box::new(RegisteredPacketImpl{ codec })) {
            panic!("tried to register {} twice", packet_id);
        }
    }

    pub fn encode(&self, writer: &mut dyn Write, packet: &dyn Packet) -> anyhow::Result<()> {
        let registered_packet = self.packet_codecs.get(&packet.packet_id())
            .ok_or_else(|| anyhow::anyhow!("packet {} hasn't been registered", packet.packet_id()))?;

        registered_packet.encode(&self.codec_registry, writer, packet)
    }

    pub fn decode(&self, reader: &mut dyn Read, packet_id: u32) -> anyhow::Result<Option<Box<dyn Packet>>> {
        let registered_packet = match self.packet_codecs.get(&packet_id) {
            Some(registered_packet) => registered_packet,
            None => return Ok(None)
        };

        Ok(Some(
            registered_packet.decode(&self.codec_registry, reader)?
        ))
    }
}

pub struct UnknownPacket {
    packet_id: u32,
    payload: Vec<u8>
}

impl UnknownPacket {
    pub fn new(packet_id: u32, payload: Vec<u8>) -> Self {
        Self { packet_id, payload }
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    } 
}

impl Debug for UnknownPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnknownPacket")
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

    fn model_id(&self) -> u32 {
        (-1i32) as u32
    }
}

include!(concat!(env!("OUT_DIR"), "/packets.rs"));