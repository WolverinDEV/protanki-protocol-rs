use rand::{Rng};

pub trait CryptContext : Send {
    fn encrypt(&mut self, buffer: &mut [u8]) -> anyhow::Result<()>;
    fn decrypt(&mut self, buffer: &mut [u8]) -> anyhow::Result<()>;
}

pub struct NoCryptContext;
impl CryptContext for NoCryptContext {
    fn encrypt(&mut self, _buffer: &mut [u8]) -> anyhow::Result<()> {
        Ok(())
    }

    fn decrypt(&mut self, _buffer: &mut [u8]) -> anyhow::Result<()> {
        Ok(())
    }
}

pub struct XOrCryptContext {
    decrypt_key: [u8;8],
    decrypt_key_offset: u8,

    encrypt_key: [u8;8],
    encrypt_key_offset: u8,
}

impl XOrCryptContext {
    pub fn new_server() -> (Self, Vec<u8>) {
        let mut rng = rand::thread_rng();

        let mut seed = Vec::new();
        seed.resize(4, 0);
        rng.fill(seed.as_mut_slice());

        (Self::new(true, &seed), seed)
    }

    pub fn new_client(initial_hash: &[u8]) -> Self {
        Self::new(false, initial_hash)
    }

    fn new(is_server: bool, initial_hash: &[u8]) -> Self {
        let seed = initial_hash.iter().fold(0, |acc, v| acc ^ *v) as u8;
        let mut key_decrypt = [0u8; 8];
        let mut key_encrypt = [0u8; 8];
        for index in 0u8..8 {
            if is_server {
                key_encrypt[index as usize] = seed ^ index << 3;
                key_decrypt[index as usize] = seed ^ index << 3 ^ 87;
            } else {
                key_decrypt[index as usize] = seed ^ index << 3;
                key_encrypt[index as usize] = seed ^ index << 3 ^ 87;
            }
        }

        Self {
            decrypt_key: key_decrypt,
            decrypt_key_offset: 0,

            encrypt_key: key_encrypt,
            encrypt_key_offset: 0,
        }
    }
}

impl CryptContext for XOrCryptContext {
    fn encrypt(&mut self, buffer: &mut [u8]) -> anyhow::Result<()> {
        for index in 0..buffer.len() {
            let value = buffer[index];
            buffer[index] = value ^ self.encrypt_key[self.encrypt_key_offset as usize];
            self.encrypt_key[self.encrypt_key_offset as usize] = value;
            self.encrypt_key_offset ^= value & 0x7;
        }

        Ok(())
    }

    fn decrypt(&mut self, buffer: &mut [u8]) -> anyhow::Result<()> {
        for index in 0..buffer.len() {
            let value = buffer[index];
            self.decrypt_key[self.decrypt_key_offset as usize] = value ^ self.decrypt_key[self.decrypt_key_offset as usize];
            buffer[index] = self.decrypt_key[self.decrypt_key_offset as usize];
            self.decrypt_key_offset ^= self.decrypt_key[self.decrypt_key_offset as usize] & 0x7;
        }

        Ok(())
    }
}