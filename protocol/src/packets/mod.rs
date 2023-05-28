use std::{io::{Write, Read}, any::Any};
use std::fmt::Debug;
use crate::ProtocolResult;

mod handler;
pub use handler::*;

mod registry;
pub use registry::*;

mod unknown;
pub use unknown::*;

mod generated;
pub use generated::*;

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub enum PacketDirection {
    Unknown,

    S2C,
    C2S,
}

impl Default for PacketDirection {
    fn default() -> Self {
        PacketDirection::Unknown
    }
}

pub trait Packet : Debug + Send {
    fn as_any(self: &Self) -> &dyn Any;
    fn as_any_mut(self: &mut Self) -> &mut dyn Any;

    fn direction(&self) -> PacketDirection;
    fn packet_name(&self) -> &str;
    fn packet_id(&self) -> u32;
    fn model_id(&self) -> u32;

    fn encode(&self, writer: &mut dyn Write) -> ProtocolResult<()>;
    fn decode(&mut self, reader: &mut dyn Read) -> ProtocolResult<()>;
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