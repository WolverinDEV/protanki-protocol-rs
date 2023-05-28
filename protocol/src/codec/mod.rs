use std::io::{Write, Read};
use crate::ProtocolResult;

mod primitives;
pub use primitives::*;

mod custom;
pub use custom::*;

mod basics;
pub use basics::*;

mod enums;
pub use enums::*;

mod structs;
pub use structs::*;

/// All typed having the Codeable trait can be encoded with the tanks
/// protocol.
pub trait Codeable : Send + Sync {
    fn encode(&self, writer: &mut dyn Write) -> ProtocolResult<()>;
    fn decode(&mut self, reader: &mut dyn Read) -> ProtocolResult<()>;
}