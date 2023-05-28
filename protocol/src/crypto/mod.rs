mod plain;
pub use plain::*;

mod xor;
pub use xor::*;

use crate::ProtocolResult;

#[derive(Debug, Copy, Clone)]
pub enum CipherMode {
    Server,
    Client,
}

pub trait Cipher : Send {
    fn mode(&self) -> CipherMode;

    fn encrypt(&mut self, buffer: &mut [u8]) -> ProtocolResult<()>;
    fn decrypt(&mut self, buffer: &mut [u8]) -> ProtocolResult<()>;
}