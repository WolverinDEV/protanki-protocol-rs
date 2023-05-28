use super::{Cipher, CipherMode, ProtocolResult};

/// Plain cipher doing nothing.
/// Assumes the data is not encrypted.
pub struct PlainCipher {
    mode: CipherMode
}

impl PlainCipher {
    pub fn new(mode: CipherMode) -> Self {
        Self {
            mode
        }
    }
}

impl Cipher for PlainCipher {
    fn mode(&self) -> CipherMode {
        self.mode
    }

    fn encrypt(&mut self, _buffer: &mut [u8]) -> ProtocolResult<()> {
        Ok(())
    }

    fn decrypt(&mut self, _buffer: &mut [u8]) -> ProtocolResult<()> {
        Ok(())
    }
}