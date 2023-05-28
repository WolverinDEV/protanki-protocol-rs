use byteorder::{WriteBytesExt, ReadBytesExt};
use crate::{ProtocolResult};

use super::{Codeable};

macro_rules! impl_primitive {
    ($ident:ident, $encode:ident, $decode:ident) => {
        impl Codeable for $ident {
            fn encode(&self, writer: &mut dyn std::io::Write) -> ProtocolResult<()> {
                writer.$encode(*self)?;
                Ok(())
            }

            fn decode(&mut self, reader: &mut dyn std::io::Read) -> ProtocolResult<()> {
                *self = reader.$decode()?;
                Ok(())
            }
        }
    };
    
    ($ident:ident, $encode:ident, $decode:ident, $endianess:ty) => {
        impl Codeable for $ident {
            fn encode(&self, writer: &mut dyn std::io::Write) -> ProtocolResult<()> {
                writer.$encode::<$endianess>(*self)?;
                Ok(())
            }

            fn decode(&mut self, reader: &mut dyn std::io::Read) -> ProtocolResult<()> {
                *self = reader.$decode::<$endianess>()?;
                Ok(())
            }
        }
    };
}

impl_primitive!(i8, write_i8, read_i8);
impl_primitive!(u8, write_u8, read_u8);

impl_primitive!(i16, write_i16, read_i16, byteorder::BigEndian);
impl_primitive!(u16, write_u16, read_u16, byteorder::BigEndian);

impl_primitive!(i32, write_i32, read_i32, byteorder::BigEndian);
impl_primitive!(u32, write_u32, read_u32, byteorder::BigEndian);

impl_primitive!(i64, write_i64, read_i64, byteorder::BigEndian);
impl_primitive!(u64, write_u64, read_u64, byteorder::BigEndian);

impl_primitive!(f32, write_f32, read_f32, byteorder::BigEndian);
impl_primitive!(f64, write_f64, read_f64, byteorder::BigEndian);
