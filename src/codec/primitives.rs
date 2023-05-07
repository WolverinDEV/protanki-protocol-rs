use std::{io::{Write, Read}, marker::PhantomData};

use byteorder::{WriteBytesExt, ReadBytesExt};
use super::{Codec, CodecRegistry};

pub struct BoolCodec;
impl Codec for BoolCodec {
    type Target = bool;

    fn encode(self: &Self, _registry: &CodecRegistry, writer: &mut dyn std::io::Write, target: &Self::Target) -> anyhow::Result<()> {
        Ok(
            writer.write_u8(if *target { 1 } else { 0 })?
        )
    }

    fn decode(self: &Self, _registry: &CodecRegistry, reader: &mut dyn std::io::Read) -> anyhow::Result<Self::Target> {
        Ok(
            reader.read_u8()
                .map(|v| v != 0)?
        )
    }
}

pub struct ByteCodec;
impl Codec for ByteCodec {
    type Target = i8;

    fn encode(self: &Self, _registry: &CodecRegistry, writer: &mut dyn std::io::Write, target: &Self::Target) -> anyhow::Result<()> {
        Ok(
            writer.write_i8(*target)?
        )
    }

    fn decode(self: &Self, _registry: &CodecRegistry, reader: &mut dyn std::io::Read) -> anyhow::Result<Self::Target> {
        Ok(
            reader.read_i8()?
        )
    }
}

pub struct UByteCodec;
impl Codec for UByteCodec {
    type Target = u8;

    fn encode(self: &Self, _registry: &CodecRegistry, writer: &mut dyn std::io::Write, target: &Self::Target) -> anyhow::Result<()> {
        Ok(
            writer.write_u8(*target)?
        )
    }

    fn decode(self: &Self, _registry: &CodecRegistry, reader: &mut dyn std::io::Read) -> anyhow::Result<Self::Target> {
        Ok(
            reader.read_u8()?
        )
    }
}

pub struct ShortCodec;
impl Codec for ShortCodec {
    type Target = i16;

    fn encode(self: &Self, _registry: &CodecRegistry, writer: &mut dyn std::io::Write, target: &Self::Target) -> anyhow::Result<()> {
        Ok(
            writer.write_i16::<byteorder::BigEndian>(*target)?
        )
    }

    fn decode(self: &Self, _registry: &CodecRegistry, reader: &mut dyn std::io::Read) -> anyhow::Result<Self::Target> {
        Ok(
            reader.read_i16::<byteorder::BigEndian>()?
        )
    }
}

pub struct UShortCodec;
impl Codec for UShortCodec {
    type Target = u16;

    fn encode(self: &Self, _registry: &CodecRegistry, writer: &mut dyn std::io::Write, target: &Self::Target) -> anyhow::Result<()> {
        Ok(
            writer.write_u16::<byteorder::BigEndian>(*target)?
        )
    }

    fn decode(self: &Self, _registry: &CodecRegistry, reader: &mut dyn std::io::Read) -> anyhow::Result<Self::Target> {
        Ok(
            reader.read_u16::<byteorder::BigEndian>()?
        )
    }
}

pub struct IntCodec;
impl Codec for IntCodec {
    type Target = i32;

    fn encode(self: &Self, _registry: &CodecRegistry, writer: &mut dyn std::io::Write, target: &Self::Target) -> anyhow::Result<()> {
        Ok(
            writer.write_i32::<byteorder::BigEndian>(*target)?
        )
    }

    fn decode(self: &Self, _registry: &CodecRegistry, reader: &mut dyn std::io::Read) -> anyhow::Result<Self::Target> {
        Ok(
            reader.read_i32::<byteorder::BigEndian>()?
        )
    }
}

pub struct UIntCodec;
impl Codec for UIntCodec {
    type Target = u32;

    fn encode(self: &Self, _registry: &CodecRegistry, writer: &mut dyn std::io::Write, target: &Self::Target) -> anyhow::Result<()> {
        Ok(
            writer.write_u32::<byteorder::BigEndian>(*target)?
        )
    }

    fn decode(self: &Self, _registry: &CodecRegistry, reader: &mut dyn std::io::Read) -> anyhow::Result<Self::Target> {
        Ok(
            reader.read_u32::<byteorder::BigEndian>()?
        )
    }
}

pub struct LongCodec;
impl Codec for LongCodec {
    type Target = i64;

    fn encode(self: &Self, _registry: &CodecRegistry, writer: &mut dyn std::io::Write, target: &Self::Target) -> anyhow::Result<()> {
        Ok(
            writer.write_i64::<byteorder::BigEndian>(*target)?
        )
    }

    fn decode(self: &Self, _registry: &CodecRegistry, reader: &mut dyn std::io::Read) -> anyhow::Result<Self::Target> {
        Ok(
            reader.read_i64::<byteorder::BigEndian>()?
        )
    }
}

pub struct ULongCodec;
impl Codec for ULongCodec {
    type Target = u64;

    fn encode(self: &Self, _registry: &CodecRegistry, writer: &mut dyn std::io::Write, target: &Self::Target) -> anyhow::Result<()> {
        Ok(
            writer.write_u64::<byteorder::BigEndian>(*target)?
        )
    }

    fn decode(self: &Self, _registry: &CodecRegistry, reader: &mut dyn std::io::Read) -> anyhow::Result<Self::Target> {
        Ok(
            reader.read_u64::<byteorder::BigEndian>()?
        )
    }
}

pub struct FloatCodec;
impl Codec for FloatCodec {
    type Target = f32;

    fn encode(self: &Self, _registry: &CodecRegistry, writer: &mut dyn std::io::Write, target: &Self::Target) -> anyhow::Result<()> {
        Ok(
            writer.write_f32::<byteorder::BigEndian>(*target)?
        )
    }

    fn decode(self: &Self, _registry: &CodecRegistry, reader: &mut dyn std::io::Read) -> anyhow::Result<Self::Target> {
        Ok(
            reader.read_f32::<byteorder::BigEndian>()?
        )
    }
}

pub struct DoubleCodec;
impl Codec for DoubleCodec {
    type Target = f64;

    fn encode(self: &Self, _registry: &CodecRegistry, writer: &mut dyn std::io::Write, target: &Self::Target) -> anyhow::Result<()> {
        Ok(
            writer.write_f64::<byteorder::BigEndian>(*target)?
        )
    }

    fn decode(self: &Self, _registry: &CodecRegistry, reader: &mut dyn std::io::Read) -> anyhow::Result<Self::Target> {
        Ok(
            reader.read_f64::<byteorder::BigEndian>()?
        )
    }
}

pub struct Length(usize);

pub struct LengthCodec;
impl Codec for LengthCodec {
    type Target = Length;

    fn encode(&self, _registry: &CodecRegistry, writer: &mut dyn Write, target: &Self::Target) -> anyhow::Result<()> {
        let target = target.0;
        if target < 128 {
            writer.write_u8(target as u8)?;
        } else if target < 16384 {
            let encoded = (target & 16383) + 32768;
            writer.write_u8(((encoded & 65280) >> 8) as u8)?;
            writer.write_u8((encoded & 255) as u8)?;
        } else if target < 4194304 {
            let encoded = (target & 4194303) + 12582912;
            writer.write_u8(((encoded & 16711680) >> 16) as u8)?;
            writer.write_u8(((encoded & 65280) >> 8) as u8)?;
            writer.write_u8((encoded & 255) as u8)?;
        } else {
            anyhow::bail!("length value too large")
        }
        
        Ok(())
    }

    fn decode(&self, _registry: &CodecRegistry, reader: &mut dyn Read) -> anyhow::Result<Self::Target> {
        let v0 = reader.read_u8()? as usize;
        if v0 & 128 == 0 {
            return Ok(Length(v0));
        }

        let v1 = reader.read_u8()? as usize;
        if v1 & 64 == 0 {
            return Ok(Length((v0 << 8) + v1));
        }

        let v2 = reader.read_u8()? as usize;
        return Ok(Length((v0 << 16) + (v1 << 8) + v2));
    }
}

pub struct StringCodec;

impl Codec for StringCodec {
    type Target = String;

    fn encode(&self, registry: &CodecRegistry, writer: &mut dyn Write, target: &Self::Target) -> anyhow::Result<()> {
        if target.is_empty() {
            registry.encode(writer, &true)?;
            return Ok(());
        }

        let bytes = target.as_bytes();
        registry.encode(writer, &false)?;
        registry.encode(writer, &(bytes.len() as u32))?;
        writer.write_all(bytes)?;
        Ok(())
    }

    fn decode(&self, registry: &CodecRegistry, reader: &mut dyn Read) -> anyhow::Result<Self::Target> {
        if registry.decode::<bool>(reader)? {
            return Ok(String::new());
        }

        let length = registry.decode::<u32>(reader)? as usize;
        let mut buffer = Vec::with_capacity(length);
        buffer.resize(length, 0);
        reader.read_exact(&mut buffer)?;

        Ok(String::from_utf8(buffer)?)
    }
}

pub struct VectorCodec<T> {
    _maker: PhantomData<T>,
}

impl<T> Default for VectorCodec<T> {
    fn default() -> Self {
        Self { _maker: Default::default() }
    }
}

impl<T: Send + Sync + 'static> Codec for VectorCodec<T> {
    type Target = Vec<T>;

    fn encode(self: &Self, registry: &CodecRegistry, writer: &mut dyn Write, target: &Self::Target) -> anyhow::Result<()> {
        registry.encode(writer, &(target.len() as u32))?;
        for entry in target.iter() {
            registry.encode(writer, entry)?;
        }
        return Ok(());
    }

    fn decode(self: &Self, registry: &CodecRegistry, reader: &mut dyn Read) -> anyhow::Result<Self::Target> {
        let length = registry.decode::<u32>(reader)? as usize;
        let mut result = Vec::with_capacity(length);
        for _ in 0..length {
            result.push(
                registry.decode::<T>(reader)?
            );
        }

        return Ok(result);
    }
}