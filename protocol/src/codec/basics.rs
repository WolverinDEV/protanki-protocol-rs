#![allow(non_camel_case_types)]
use std::io::{Write, Read};

use byteorder::{WriteBytesExt, ReadBytesExt};
use crate::{ProtocolResult, ProtocolError};

use super::Codeable;

impl Codeable for bool {
    fn encode(&self, writer: &mut dyn Write) -> ProtocolResult<()> {
        writer.write_u8(if *self { 1 } else { 0 })?;
        Ok(())
    }

    fn decode(&mut self, reader: &mut dyn Read) -> ProtocolResult<()> {
        *self = reader.read_u8()? > 0;
        Ok(())
    }
}

pub struct Length(usize);
impl Codeable for Length {
    fn encode(&self, writer: &mut dyn Write) -> ProtocolResult<()> {
        let target = self.0;
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
            return Err(ProtocolError::CodecVarIntTooLarge);
        }
        
        Ok(())
    }

    fn decode(&mut self, reader: &mut dyn Read) -> ProtocolResult<()> {
        let v0 = reader.read_u8()? as usize;
        if v0 & 128 == 0 {
            self.0 = v0;
            return Ok(());
        }

        let v1 = reader.read_u8()? as usize;
        if v1 & 64 == 0 {
            self.0 = (v0 << 8) + v1;
            return Ok(());
        }

        let v2 = reader.read_u8()? as usize;
        self.0 = (v0 << 16) + (v1 << 8) + v2;
        return Ok(());
    }
}

impl Codeable for String {
    fn encode(&self, writer: &mut dyn Write) -> ProtocolResult<()> {
        if self.is_empty() {
            return true.encode(writer);
        }

        let bytes: &[u8] = self.as_bytes();
        false.encode(writer)?;
        (bytes.len() as u32).encode(writer)?;
        writer.write_all(bytes)?;
        Ok(())
    }

    fn decode(&mut self, reader: &mut dyn Read) -> ProtocolResult<()> {
        let mut empty = false;
        empty.decode(reader)?;
        if empty {
            self.clear();
            return Ok(());
        }

        let mut length = 0u32;
        length.decode(reader)?;

        let mut buffer = vec![0; length as usize];
        reader.read_exact(&mut buffer)?;

        match String::from_utf8(buffer) {
            Ok(result) => {
                *self = result;
                Ok(())
            },
            Err(err) => Err(ProtocolError::CodecUtf8DecodeError(err.utf8_error()))
        }
    }
}

impl<T: Codeable + Default> Codeable for Vec<T> {
    fn encode(&self, writer: &mut dyn Write) -> ProtocolResult<()> {
        (self.len() as u32).encode(writer)?;

        for entry in self.iter() {
            entry.encode(writer)?;
        }
        return Ok(());
    }

    fn decode(&mut self, reader: &mut dyn Read) -> ProtocolResult<()> {
        let mut size = 0u32;
        size.decode(reader)?;

        self.resize_with(size as usize, Default::default);
        for entry in self.iter_mut() {
            entry.decode(reader)?;
        }

        Ok(())
    }
}

impl<T: Codeable + Default> Codeable for Option<T> {
    fn encode(&self, writer: &mut dyn Write) -> ProtocolResult<()> {
        if let Some(value) = &self {
            false.encode(writer)?;
            value.encode(writer)?;
        } else {
            true.encode(writer)?;
        }

        Ok(())
    }

    fn decode(&mut self, reader: &mut dyn Read) -> ProtocolResult<()> {
        let mut empty = false;
        empty.decode(reader)?;

        if empty {
            *self = None;
        } else {
            let mut entry = T::default();
            entry.decode(reader)?;
            *self = Some(entry);
        }

        Ok(())
    }
}