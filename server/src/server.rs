use std::{collections::{BTreeMap}, sync::{Arc, Mutex, RwLock}, future::poll_fn, task::Poll};

use futures::{FutureExt, Future};
use tokio::sync::mpsc;
use tracing::{warn, info};

use crate::{client::{Client, ClientId}, client_components::{UserAuthentication, UserRegister, CaptchaProvider, ClientResources, SettingsDialog}, users::UserRegistry, ServerResource, ServerResources, ServerChat, ServerChatComponent};

pub enum ServerEvent {
    ClientAuthenticated(ClientId),
    ClientDisconnected(ClientId),
}

pub struct Server {
    clients: BTreeMap<ClientId, Arc<Mutex<Client>>>,
    client_id_index: ClientId,

    events_rx: mpsc::UnboundedReceiver<ServerEvent>,
    events_tx: mpsc::UnboundedSender<ServerEvent>,
    
    user_registry: Arc<RwLock<UserRegistry>>,
    server_resources: Arc<RwLock<ServerResources>>,
    chat: Arc<RwLock<ServerChat>>,
}

impl Server {
    pub fn new() -> anyhow::Result<Self> {
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let resources = ServerResources::new()?;

        Ok(Self {
            clients: Default::default(),
            client_id_index: 0,

            events_rx,
            events_tx,

            user_registry: Arc::new(RwLock::new(UserRegistry::new())),
            server_resources: Arc::new(RwLock::new(resources)),
            chat: Arc::new(RwLock::new(ServerChat::new())),
        })
    }

    pub fn register_client(&mut self, mut client: Client) {
        self.client_id_index += 1;
        let client_id = self.client_id_index;

        client.setup_client(client_id, self.events_tx.clone());
        info!("Received new client from {} ({}). Assigning client id {}.", client.peer_address(), client.language(), client_id);

        client.register_component(ClientResources::new(self.server_resources.clone()));
        client.register_component(UserAuthentication::new(self.user_registry.clone()));
        client.register_component(UserRegister::new(self.user_registry.clone()));
        client.register_component(CaptchaProvider::new());

        let client = Arc::new(Mutex::new(client));
        if let Some(_old_client) = self.clients.insert(client_id, client.clone()) {
            // TODO(mh): Check that the client id never overrides another client!
            warn!("Dropping client as it got overriden by new client with the same client id");
        }
        
        let events_tx = self.events_tx.clone();
        tokio::spawn(async move {
            poll_fn(move |cx| {
                let mut client = match client.lock() {
                    Ok(client) => client,
                    Err(_) => return Poll::Ready(())
                };
    
                client.poll_unpin(cx)
            }).await;

            let _ = events_tx.send(ServerEvent::ClientDisconnected(client_id));
        });
    }

    fn handle_client_authenticated(&mut self, client_id: ClientId) {
        let client = match self.clients.get(&client_id) {
            Some(client) => client,
            None => return,
        };
        let mut client = client.lock().unwrap();

        client.register_component(ServerChatComponent::new(self.chat.clone()));
        client.register_component(SettingsDialog::new());
    }

    fn handle_event(&mut self, event: ServerEvent) {
        match event {
            ServerEvent::ClientDisconnected(client_id) => {
                info!("Client {} disconnected.", client_id);
                self.clients.remove(&client_id);
            },
            ServerEvent::ClientAuthenticated(client_id) => {
                self.handle_client_authenticated(client_id);
            },
            _ => {

            }
        }
    }
}

impl Future for Server {
    type Output = ();

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        loop {
            match self.events_rx.poll_recv(cx) {
                Poll::Ready(Some(event)) => self.handle_event(event),
                _ => break,
            }
        }

        Poll::Pending
    }
}