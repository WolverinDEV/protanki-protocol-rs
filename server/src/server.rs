use std::{collections::{BTreeMap}, sync::{Arc, Mutex, RwLock}, future::poll_fn, task::Poll};

use futures::{FutureExt, Future};
use tokio::sync::mpsc;
use tracing::{warn, info};

use crate::{client::{Client, ClientId}, client_components::{UserAuthentication, UserRegister, CaptchaProvider}, users::UserRegistry};

enum ServerEvent {
    ClientDisconnected(ClientId),
}

pub struct Server {
    clients: BTreeMap<ClientId, Arc<Mutex<Client>>>,
    client_id_index: ClientId,

    events_rx: mpsc::UnboundedReceiver<ServerEvent>,
    events_tx: mpsc::UnboundedSender<ServerEvent>,
    
    user_registry: Arc<RwLock<UserRegistry>>,
}

impl Server {
    pub fn new() -> Self {
        let (events_tx, events_rx) = mpsc::unbounded_channel();

        Self {
            clients: Default::default(),
            client_id_index: 0,

            events_rx,
            events_tx,

            user_registry: Arc::new(RwLock::new(UserRegistry::new())),
        }
    }

    pub fn register_client(&mut self, mut client: Client) {
        self.client_id_index += 1;
        let client_id = self.client_id_index;

        client.setup_client_id(client_id);
        info!("Received new client from {} ({}). Assigning client id {}.", client.peer_address(), client.language(), client_id);

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

    fn handle_event(&mut self, event: ServerEvent) {
        match event {
            ServerEvent::ClientDisconnected(client_id) => {
                info!("Client {} disconnected.", client_id);
                self.clients.remove(&client_id);
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