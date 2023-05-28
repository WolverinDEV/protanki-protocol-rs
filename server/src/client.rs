use std::{any::{Any, TypeId}, collections::BTreeMap, task::{self, Poll, Waker}, net::{TcpStream, SocketAddr}, sync::{Weak, Mutex, Arc}, time::Duration, cell::{RefCell, Ref, RefMut}, rc::Rc};
use futures::{Future, StreamExt};
use tokio::{time, sync::mpsc};
use tracing::{ error, debug };
use fost_protocol::{Connection, packets::{Packet, PacketDowncast, self}, Socket, SimplePacketDebugFilter, ProtocolError};

use crate::{server::{Server, ServerEvent}, client_components::{ConnectionPing}, Tasks};

pub enum AuthenticationState {
    /* InviteCode, */
    Unauthenticated,
    Authenticated{ user_id: String }
}

pub enum ClientEvent {
    /* Authenticated */
}

/// A basic component performing one single purpose.
/// An example would be the battle list manager or chat handler.
pub trait ClientComponent : Send {
    fn on_packet(&mut self, client: &mut Client, packet: &dyn Packet) -> anyhow::Result<()> {
        Ok(())
    }

    /* fn on_client_event */
    /* fn on_server_event */

    fn poll(&mut self, client: &mut Client, cx: &mut task::Context) -> anyhow::Result<()> {
        Ok(())
    }
}

trait RegisteredClientComponent : ClientComponent {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

struct RegisteredClientComponentImpl<T> {
    component: T
}

impl<T: ClientComponent> ClientComponent for RegisteredClientComponentImpl<T> {
    fn on_packet(&mut self, client: &mut Client, packet: &dyn Packet) -> anyhow::Result<()> {
        self.component.on_packet(client, packet)
    }

    fn poll(&mut self, client: &mut Client, cx: &mut task::Context) -> anyhow::Result<()> {
        self.component.poll(client, cx)
    }
}

impl<T: ClientComponent + 'static> RegisteredClientComponent for RegisteredClientComponentImpl<T> {
    fn as_any(&self) -> &dyn Any {
        &self.component
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        &mut self.component
    }
}

pub type ClientId = u32;
pub struct Client {
    client_id: ClientId,
    server_events: Option<mpsc::UnboundedSender<ServerEvent>>,

    connection: Connection,
    components: BTreeMap<TypeId, Arc<RefCell<dyn RegisteredClientComponent>>>,

    language: String,

    tasks: Rc<Tasks>,
    waker: Option<Waker>,

    authentication_state: AuthenticationState,
    /* TODO: Use active screen */
}

unsafe impl Send for Client {}

impl Client {
    pub async fn new(socket: Box<dyn Socket + Send>, socket_address: SocketAddr) -> anyhow::Result<Self> {
        let mut connection = Connection::new(
            true,
            socket_address,
            socket,
            Box::new(SimplePacketDebugFilter::logging_enabled())
        );

        connection.init_encryption().await?;
        let init_packet = match {
            time::timeout(
                Duration::from_secs(15), 
                connection.next()
            ).await
        } {
            Ok(Some(Ok(packet))) => packet,
            Ok(Some(Err(error))) => anyhow::bail!("setup connection error: {}", error),
            Ok(None) => anyhow::bail!("connection closed while in setup"),
            Err(_) => anyhow::bail!("setup timed out")
        };

        let init_packet = match init_packet.downcast_ref::<packets::c2s::ResourceLoaderEncryptionInitialized>() {
            Some(packet) => packet,
            None => anyhow::bail!("expected a encryption initialized packet")
        };

        let mut client = Self {
            client_id: 0,
            server_events: None,

            connection,
            components: Default::default(),

            language: init_packet.lang.to_string(),

            waker: None,
            tasks: Rc::new(Tasks::new()),

            authentication_state: AuthenticationState::Unauthenticated,
        };

        client.register_component(ConnectionPing::new(Duration::from_millis(2_500)));

        Ok(client)
    }

    pub fn client_id(&self) -> ClientId {
        self.client_id
    }

    pub fn setup_client(&mut self, client_id: ClientId, server_events: mpsc::UnboundedSender<ServerEvent>) {
        assert_eq!(self.client_id, 0);
        self.client_id = client_id;
        self.server_events = Some(server_events);
    }

    pub fn language(&self) -> &str {
        &self.language
    }

    pub fn peer_address(&self) -> &SocketAddr {
        &self.connection.address
    }

    pub fn authentication_state(&self) -> &AuthenticationState {
        &self.authentication_state
    }

    pub fn authentication_state_mut(&mut self) -> &mut AuthenticationState {
        &mut self.authentication_state
    }

    pub fn issue_server_event(&self, event: ServerEvent) {
        if let Some(sender) = &self.server_events {
            let _ = sender.send(event);
        }
    }

    pub fn register_component<T: ClientComponent + 'static>(&mut self, component: T) {
        self.components.insert(TypeId::of::<T>(), 
            Arc::new(RefCell::new(RegisteredClientComponentImpl{ component }))
        );

        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }

    pub fn get_component<T: ClientComponent + 'static>(&self) -> Option<Ref<'_, T>> {
        self.components.get(&TypeId::of::<T>())
            .map(|c| {
                let c = c.borrow();
                Ref::map(c, |c| {
                    c.as_any().downcast_ref::<T>().expect("to be of type T")
                })
            })
    }  

    pub fn with_component_mut<T: ClientComponent + 'static, R>(&mut self, callback: impl FnOnce(&mut Client, &mut T) -> R) -> Option<R> {
        let component = self.components.get(&TypeId::of::<T>())
            .cloned();

        component.map(move |component| {
            let mut component = component.borrow_mut();
            let mut component = component.as_any_mut().downcast_mut::<T>().expect("to be of type T");
            callback(self, component)
        })
    }

    pub fn get_component_mut<T: ClientComponent + 'static>(&mut self) -> Option<RefMut<'_, T>> {
        self.components.get(&TypeId::of::<T>())
            .map(|c| {
                let c = c.borrow_mut();
                RefMut::map(c, |c| {
                    c.as_any_mut().downcast_mut::<T>().expect("to be of type T")
                })
            })
    }

    
    pub fn send_packet(&mut self, packet: &dyn Packet) {
        if let Err(error) = self.connection.send_packet(packet) {
            /* TODO: handle this error */
            error!("failed to send packet: {}", error);
        }
    }

    pub fn run_async<T: 'static, F: Future<Output = T> + Send + 'static, C: FnOnce(&mut Client, T) -> () + Send + 'static>(&mut self, task: F, callback: C) {
        self.tasks.enqueue(task, callback)
    }

    pub fn disconnect(&mut self) {
        /* TODO! */
    }

    fn handle_protocol_error(&mut self, error: ProtocolError) {
        error!("protocol error: {}", error);
    }

    fn handle_handle_error(&mut self, error: anyhow::Error) {
        error!("handle error: {}", error);
    }

    fn handle_packet(&mut self, packet: Box<dyn Packet>) {
        let components = self.components.values()
                .cloned()
                .collect::<Vec<_>>();
        
        for mut component in components {
            let mut component = component.borrow_mut();
            if let Err(error) = component.on_packet(self, Box::as_ref(&packet)) {
                self.handle_handle_error(error);
            }
        }
    }
}

impl Future for Client {
    type Output = ();

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        self.waker = Some(cx.waker().clone());

        let tasks = self.tasks.clone();
        tasks.poll(&mut self, cx);

        {
            let components = self.components.values()
                .cloned()
                .collect::<Vec<_>>();
            
            for mut component in components {
                let mut component = component.borrow_mut();
                if let Err(error) = component.poll(&mut self, cx) {
                    self.handle_handle_error(error);
                }
            }
        }

        loop {
            match self.connection.poll_next_unpin(cx) {
                Poll::Ready(Some(Ok(item))) => {
                    self.handle_packet(item);
                    continue;
                },
                Poll::Ready(Some(Err(err))) => {
                    self.handle_protocol_error(err);
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