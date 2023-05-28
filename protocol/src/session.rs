use std::{net::SocketAddr, task::{Poll, Context}, sync::Arc, collections::BTreeMap, any::{TypeId, Any}, time::Instant};

use anyhow::anyhow;
use futures::{StreamExt, Future, FutureExt};
use tokio::{net::TcpSocket, sync::{oneshot}};

use crate::{connection::Connection, packets::{self, Packet, PacketDowncast, PacketHandlerRegistry, PacketHandler}, SimplePacketDebugFilter, PacketDebugFilter, Socket, ProtocolError};

pub trait Task : Send {
    type Result : Send;

    fn handle_packet(&mut self, _session: &mut Session, _packet: &dyn Packet) -> anyhow::Result<()> {
        Ok(())
    }

    fn poll(&mut self, _session: &mut Session, _cx: &mut Context) -> Poll<anyhow::Result<Self::Result>>;
}

pub trait SessionComponent : Send {

}

pub struct TaskHandle<'a, R: Send> {
    client: &'a mut Session,
    rx: oneshot::Receiver<anyhow::Result<R>>,
}

impl<R: Send> Future for TaskHandle<'_, R> {
    type Output = anyhow::Result<R>;

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.client.poll_unpin(cx) {
            Poll::Ready(_) => return Poll::Ready(Err(anyhow!("client disconnected"))),
            Poll::Pending => {}
        }

        match self.rx.poll_unpin(cx) {
            Poll::Ready(Ok(result)) => Poll::Ready(result),
            Poll::Ready(Err(_)) => Poll::Ready(Err(anyhow!("result recv error"))),
            Poll::Pending => Poll::Pending
        }
    }
}

type PacketHandlerId = u32;

/// A tanks session with the server as abstraction over a simple connection.
/// The session allows for state management.
pub struct Session {
    sesstion_start: Instant,

    language_code: String,
    pub connection: Connection,

    packet_handler: Arc<PacketHandlerRegistry>,
    
    disconnected: bool,
    components: BTreeMap<TypeId, Box<dyn Any>>,
}

impl Session {
    pub fn builder() -> SessionBuilder {
        SessionBuilder::new()
    }

    pub fn session_timestamp(&self) -> i32 {
        self.sesstion_start.elapsed().as_millis() as i32
    }

    pub fn language_code(&self) -> &str {
        &self.language_code
    } 

    pub fn register_component<T: Any>(&mut self, component: T) {
        self.components.insert(TypeId::of::<T>(), Box::new(component));
    }

    pub fn get_component<T: Any>(&self) -> Option<&T> {
        self.components.get(&TypeId::of::<T>())
            .map(|c| c.downcast_ref::<T>().expect("to be of type T"))
    }  

    pub fn get_component_mut<T: Any>(&mut self) -> Option<&mut T> {
        self.components.get_mut(&TypeId::of::<T>())
            .map(|c| c.downcast_mut::<T>().expect("to be of type T"))
    } 

    pub fn register_packet_handler(&mut self, handler: impl PacketHandler + 'static) -> PacketHandlerId {
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
        self.disconnected = true;
    }

    fn handle_connection_error(&mut self, error: ProtocolError) {
        self.disconnected = true;
        tracing::error!("connection error: {:?}", error);
        /* TODO! */
    }

    pub fn execute_task<T: Task<Result = R> + 'static, R: Send + 'static>(&mut self, task: T) -> TaskHandle<R> {
        let (tx, rx) = oneshot::channel();

        /* packet handler unregisters automaticaly as soon as task finishes */
        self.packet_handler.register_handler(packets::TaskHandler{
            task,
            tx: Some(tx)
        });

        TaskHandle { client: self, rx }
    }

    // FIXME: Set as task!
    pub async fn await_match<F: (Fn(&mut Session, &dyn Packet) -> Option<R>) + Send + 'static, R: Send + 'static>(&mut self, matcher: F) -> anyhow::Result<R> {
        let (tx, rx) = oneshot::channel::<R>();

        let packet_handler = self.packet_handler.clone();
        let handler_id = packet_handler.register_handler(packets::HandlerAwaitMatching{ matcher, sender: Some(tx) });

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
            if packet.is_type::<packets::s2c::ResourceLoaderFinished>() {
                Some(())
            } else {
                None
            }
        }).await?;

        Ok(())
    }
}

impl Future for Session {
    type Output = ();

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        if self.disconnected {
            return Poll::Ready(())
        }
        
        /* poll all handlers first as they might trigger some action due to the previous packet */
        let packet_handler = self.packet_handler.clone();
        if let Poll::Ready(error) = packet_handler.poll(&mut self, cx) {
            self.handle_handler_error(error);
        }

        loop {
            match self.connection.poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(item))) => {
                    self.handle_packet(Box::as_ref(&item));

                    /* only handle one packet at the time */
                    cx.waker().clone().wake();
                    return Poll::Pending;
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
        
        Poll::Pending
    }
}

pub struct SessionBuilder {
    language_code: String,
    log_filter: Box<dyn PacketDebugFilter>,
    allow_unknown_packets: bool,
}

impl SessionBuilder {
    fn new() -> Self {
        Self {
            language_code: "en".to_string(),
            log_filter: Box::new(SimplePacketDebugFilter::logging_disabled()),
            allow_unknown_packets: false,
        }
    }

    pub fn set_lang_code<T: ToString>(mut self, code: T) -> Self {
        self.language_code = code.to_string();
        self
    }

    pub fn set_log_filter(mut self, filter: Box<dyn PacketDebugFilter>) -> Self {
        self.log_filter = filter;
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

    pub fn allow_unknown_packets(mut self) -> Self {
        self.allow_unknown_packets = true;
        self
    }

    pub async fn connect_with_socket(self, address: SocketAddr, socket: Box<dyn Socket + Send>) -> anyhow::Result<Session> {
        let mut session = Session{
            sesstion_start: Instant::now(),
            connection: Connection::new(false, address, socket, self.log_filter),
            language_code: self.language_code.clone(),
            packet_handler: Default::default(),
            disconnected: false,
            components: Default::default(),
        };

        if self.allow_unknown_packets {
            session.connection.allow_unknown_packets();
        }

        session.connection.init_encryption().await?;
        session.connection.send_packet(&packets::c2s::ResourceLoaderEncryptionInitialized{
            lang: self.language_code
        })?;
        Ok(session)
    }

    pub async fn connect(self, target: SocketAddr) -> anyhow::Result<Session> {
        let socket = TcpSocket::new_v4()?;
        let stream = socket.connect(target.clone()).await?;
        self.connect_with_socket(target, Box::new(stream)).await
    }
}

