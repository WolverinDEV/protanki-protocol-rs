use rand::Rng;

use super::{CipherMode, Cipher, ProtocolResult};

/// Simple XOR streaming cipher 
pub struct XOrCipher {
    mode: CipherMode,

    decrypt_key: [u8;8],
    decrypt_key_offset: u8,

    encrypt_key: [u8;8],
    encrypt_key_offset: u8,
}

impl XOrCipher {
    pub fn generate_key() -> Vec<u8> {
        let mut key = vec![0; 4];
        rand::thread_rng().fill(&mut key[..]);
    
        key
    }

    pub fn new(mode: CipherMode, key: &[u8]) -> Self {
        let seed = key.iter().fold(0, |acc, v| acc ^ *v) as u8;
        let mut key_decrypt = [0u8; 8];
        let mut key_encrypt = [0u8; 8];
        for index in 0u8..8 {
            match mode {
                CipherMode::Server => {
                    key_encrypt[index as usize] = seed ^ index << 3;
                    key_decrypt[index as usize] = seed ^ index << 3 ^ 87;
                },
                CipherMode::Client => {
                    key_decrypt[index as usize] = seed ^ index << 3;
                    key_encrypt[index as usize] = seed ^ index << 3 ^ 87;
                }
            }
        }

        Self {
            mode,

            decrypt_key: key_decrypt,
            decrypt_key_offset: 0,

            encrypt_key: key_encrypt,
            encrypt_key_offset: 0,
        }
    }
}

impl Cipher for XOrCipher {
    fn mode(&self) -> CipherMode {
        self.mode
    }

    fn encrypt(&mut self, buffer: &mut [u8]) -> ProtocolResult<()> {
        for index in 0..buffer.len() {
            let value = buffer[index];
            buffer[index] = value ^ self.encrypt_key[self.encrypt_key_offset as usize];
            self.encrypt_key[self.encrypt_key_offset as usize] = value;
            self.encrypt_key_offset ^= value & 0x7;
        }

        Ok(())
    }

    fn decrypt(&mut self, buffer: &mut [u8]) -> ProtocolResult<()> {
        for index in 0..buffer.len() {
            let value = buffer[index];
            self.decrypt_key[self.decrypt_key_offset as usize] = value ^ self.decrypt_key[self.decrypt_key_offset as usize];
            buffer[index] = self.decrypt_key[self.decrypt_key_offset as usize];
            self.decrypt_key_offset ^= self.decrypt_key[self.decrypt_key_offset as usize] & 0x7;
        }

        Ok(())
    }
}