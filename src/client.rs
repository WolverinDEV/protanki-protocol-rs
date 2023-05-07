use std::{net::SocketAddr, task::{Poll}, sync::Arc};

use futures::{StreamExt, Future};
use tokio::{net::TcpSocket, sync::{oneshot}};

use crate::{connection::Connection, packets::{self, Packet, PacketDowncast}, packet_handler::{PacketHandler, PacketHandlerRegistry, HandlerAwaitMatching}, ConnectionError, SimplePacketDebugFilter, PacketDebugFilter, Socket};

type PacketHandlerId = u32;
pub struct TanksClient {
    language_code: String,
    pub connection: Connection,

    packet_handler: Arc<PacketHandlerRegistry>,
    
    disconnected: bool,
}

impl TanksClient {
    pub fn builder() -> TanksClientBuilder {
        TanksClientBuilder::new()
    }

    pub fn language_code(&self) -> &str {
        &self.language_code
    } 

    pub fn register_packet_handler(&mut self, handler: impl PacketHandler + Send + 'static) -> PacketHandlerId {
        self.packet_handler.register_handler(handler)
    }
    
    pub fn remove_packet_handler(&mut self, handler_id: PacketHandlerId) {
        self.packet_handler.remove_handler(handler_id);
    }

    fn handle_packet(&mut self, packet: &dyn Packet) {
        let packet_handler = self.packet_handler.clone();
        if let Err(error) = packet_handler.handle(self, packet) {
            self.handle_handler_error(error);
        }
    }

    fn handle_handler_error(&mut self, error: anyhow::Error) {
        /* TODO: Critical error. Close client. */
        tracing::error!("handle error: {}", error);
    }

    fn handle_connection_error(&mut self, error: ConnectionError) {
        self.disconnected = true;
        tracing::error!("connection error: {:?}", error);
        /* TODO! */
    }

    pub async fn await_match<F: (Fn(&mut TanksClient, &dyn Packet) -> Option<R>) + Send + 'static, R: Send + 'static>(&mut self, matcher: F) -> anyhow::Result<R> {
        let (tx, rx) = oneshot::channel::<R>();

        let packet_handler = self.packet_handler.clone();
        let handler_id = packet_handler.register_handler(HandlerAwaitMatching{ matcher, sender: Some(tx) });

        tokio::select! {
            result = rx => {
                /* since the channel completed the packet handler will be removed due to polling */
                Ok(result?)
            },
            _ = self => {
                packet_handler.remove_handler(handler_id);
                anyhow::bail!("client disconnected")
            }
        }
    }

    pub async fn await_server_resources_loaded(&mut self) -> anyhow::Result<()> {
        self.await_match(|_, packet| {
            if packet.is_type::<packets::S2CResourceLoaderResourcesLoaded>() {
                Some(())
            } else {
                None
            }
        }).await?;

        Ok(())
    }
}

impl Future for TanksClient {
    type Output = ();

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        loop {
            match self.connection.poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(item))) => {
                    self.handle_packet(Box::as_ref(&item));
                },
                Poll::Ready(Some(Err(err))) => {
                    self.handle_connection_error(err);
                },
                Poll::Ready(None) => {
                    return Poll::Ready(());
                },
                Poll::Pending => break,
            }
        }

        let packet_handler = self.packet_handler.clone();
        if let Poll::Ready(error) = packet_handler.poll(&mut self, cx) {
            self.handle_handler_error(error);
        }
        
        Poll::Pending
    }
}

pub struct TanksClientBuilder {
    language_code: String,
    log_filter: Box<dyn PacketDebugFilter>,
}

impl TanksClientBuilder {
    fn new() -> Self {
        Self {
            language_code: "en".to_string(),
            log_filter: Box::new(SimplePacketDebugFilter::logging_disabled()),
        }
    }

    pub fn set_lang_code<T: ToString>(mut self, code: T) -> Self {
        self.language_code = code.to_string();
        self
    }

    pub fn set_log_filter(mut self, filter: impl PacketDebugFilter + 'static) -> Self {
        self.log_filter = Box::new(filter);
        self
    }

    pub fn set_log_packets(mut self, enabled: bool) -> Self {
        self.log_filter = Box::new(if enabled {
            SimplePacketDebugFilter::logging_enabled()
        } else {
            SimplePacketDebugFilter::logging_disabled()
        });
        self
    }

    pub async fn connect_with_socket(self, address: SocketAddr, socket: Box<dyn Socket + Send>) -> anyhow::Result<TanksClient> {
        let mut client = TanksClient{
            connection: Connection::new(false, address, socket, self.log_filter),
            language_code: self.language_code.clone(),
            packet_handler: Default::default(),
            disconnected: false,
        };

        client.connection.init_encryption().await?;
        client.connection.send_packet(&packets::C2SResourceLoaderEncryptionInitialized{
            lang: self.language_code
        })?;
        Ok(client)
    }

    pub async fn connect(self, target: SocketAddr) -> anyhow::Result<TanksClient> {
        let socket = TcpSocket::new_v4()?;
        let stream = socket.connect(target.clone()).await?;
        self.connect_with_socket(target, Box::new(stream)).await
    }
}

