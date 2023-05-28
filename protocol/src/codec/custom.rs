use nalgebra::Vector3;

use crate::ProtocolResult;
use super::Codeable;

impl Codeable for Vector3<f32> {
    fn encode(&self, writer: &mut dyn std::io::Write) -> ProtocolResult<()> {
        self.x.encode(writer)?;
        self.y.encode(writer)?;
        self.z.encode(writer)
    }

    fn decode(&mut self, reader: &mut dyn std::io::Read) -> ProtocolResult<()> {
        self.x.decode(reader)?;
        self.y.decode(reader)?;
        self.z.decode(reader)
    }
}