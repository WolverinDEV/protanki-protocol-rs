use std::net::SocketAddr;

use tokio::net::TcpSocket;

use crate::{connection::Connection, packets};


pub struct TanksClient {
    language_code: String,
    pub connection: Connection,
}

impl TanksClient {
    pub fn builder() -> TanksClientBuilder {
        TanksClientBuilder::new()
    }

    pub fn language_code(&self) -> &str {
        &self.language_code
    } 
}

#[derive(Debug, Clone)]
pub struct TanksClientBuilder {
    language_code: String,
    log_packets: bool,
}
impl TanksClientBuilder {
    fn new() -> Self {
        Self {
            language_code: "en".to_string(),
            log_packets: false,
        }
    }

    pub fn set_lang_code<T: ToString>(mut self, code: T) -> Self {
        self.language_code = code.to_string();
        self
    }

    pub fn set_log_packets(mut self, enabled: bool) -> Self {
        self.log_packets = enabled;
        self
    }

    pub async fn connect(self, target: SocketAddr) -> anyhow::Result<TanksClient> {
        let socket = TcpSocket::new_v4()?;
        let stream = socket.connect(target).await?;
        let mut client = TanksClient{
            connection: Connection::new(false, target, Box::new(stream), self.log_packets),
            language_code: self.language_code.clone(),
        };

        client.connection.init_encryption().await?;
        client.connection.send_packet(&packets::C2SResourceLoaderEncryptionInitialized{
            lang: self.language_code
        })?;
        Ok(client)
    }
}