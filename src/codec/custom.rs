use super::Codec;

pub type ResourceId = u32;

pub struct ResourceIdCodec;
impl Codec for ResourceIdCodec {
    type Target = ResourceId;

    fn encode(self: &Self, registry: &super::CodecRegistry, writer: &mut dyn std::io::Write, target: &Self::Target) -> anyhow::Result<()> {
        registry.encode::<u32>(writer, target)
    }

    fn decode(self: &Self, registry: &super::CodecRegistry, reader: &mut dyn std::io::Read) -> anyhow::Result<Self::Target> {
        registry.decode::<u32>(reader)
    }
}