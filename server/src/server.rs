use std::{collections::{BTreeMap}, sync::{Arc, Mutex, RwLock}, future::poll_fn, task::Poll, time::Duration};

use anyhow::Context;
use fost_protocol::{packets::s2c, codec::{LayoutState, UserPropertyCC}};
use futures::{FutureExt, Future};
use sqlx::SqliteConnection;
use tokio::{sync::mpsc, time};
use tracing::{warn, info};

use crate::{client::{Client, ClientId}, client_components::{UserAuthentication, UserRegister, CaptchaProvider, ClientResources, SettingsDialog, LoginKickoff}, users::UserRegistry, ServerResource, ServerResources, ServerChat, ServerChatComponent, ResourceStage};

pub type DatabaseConnection = SqliteConnection;
pub type DatabaseHandle = Arc<tokio::sync::Mutex<DatabaseConnection>>;

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

    database: DatabaseHandle,
}

impl Server {
    pub fn new(database: DatabaseHandle) -> anyhow::Result<Self> {
        let (events_tx, events_rx) = mpsc::unbounded_channel();
        let resources = ServerResources::new()?;

        Ok(Self {
            clients: Default::default(),
            client_id_index: 0,

            events_rx,
            events_tx,

            user_registry: Arc::new(RwLock::new(UserRegistry::new(database.clone()))),
            server_resources: Arc::new(RwLock::new(resources)),
            chat: Arc::new(RwLock::new(ServerChat::new())),
            database
        })
    }

    pub fn register_client(&mut self, mut client: Client) -> anyhow::Result<()> {
        self.client_id_index += 1;
        let client_id = self.client_id_index;

        client.setup_client(client_id, self.events_tx.clone());
        info!("Received new client from {} ({}). Assigning client id {}.", client.peer_address(), client.language(), client_id);

        /* load the connect resource before transition to the login phase */
        client.register_component(ClientResources::new(self.server_resources.clone()));

        let user_registry = self.user_registry.clone();
        client.with_component_mut::<ClientResources, _>(move |client, resources| {
            let connect_resources = resources.await_resources_loaded(client, ResourceStage::Connect)?;
            client.run_async(connect_resources, move |client, _| {
                client.register_component(UserAuthentication::new(user_registry.clone()));
                client.register_component(UserRegister::new(user_registry.clone()));
                client.register_component(CaptchaProvider::new());
                client.register_component(LoginKickoff::new());
            });

            anyhow::Ok(())
        }).context("missing client resources")??;

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

        Ok(())
    }

    fn handle_client_authenticated(&mut self, client_id: ClientId) -> anyhow::Result<()> {
        let client = match self.clients.get(&client_id) {
            Some(client) => client,
            None => return Ok(()),
        };
        let mut client = client.lock().unwrap();

        /* this only toggles the load screen */
        client.send_packet(&s2c::LobbyLayoutSwitchStart{ state: LayoutState::BattleSelect });

        let user_id = client.user_id().context("missing client user id")?.to_string();
        {
            let user_query = self.user_registry.read()
                .expect("to lock the user registry")
                .find_user(user_id.clone());

            client.run_async(user_query, |client, user_info| {
                let user_info = match user_info {
                    Some(info) => info,
                    /* should not occur and if so just do nothing and bug out the client */
                    None => return,
                };

                client.send_packet(&s2c::AccountInfoProperties {
                    user_property_cc: UserPropertyCC {
                        id: user_id,
                        user_profile_url: "https://did.science/".to_string(),
        
                        server_number: 1,
        
                        rank: 2, // FIXME: TODO!
                        score: user_info.experience as i32,
                        current_rank_score: 100, // FIXME: TODO!
                        next_rank_score: 200, // FIXME: TODO!

                        rating: 1337f32,
                        place: 3,
        
                        crystals: user_info.crystals as i32,
                        duration_crystal_abonement: 8000,
                        has_double_crystal: true,
                    }
                });
                
                // TODO: Notify premium data

                if let Some(email) = user_info.email {
                    client.send_packet(&s2c::AccountCredentialsInit{
                        email: email,
                        email_confirmed: user_info.email_confirmed
                    });
                } else {
                    client.send_packet(&s2c::AccountCredentialsInit{
                        email: "".to_string(),
                        email_confirmed: false
                    });
                }
            });
        }

        /* register lobby components */
        client.register_component(SettingsDialog::new());

        // TODO: Module 23?

        let resource_task = client.with_component_mut::<ClientResources, _>(|client, resources| {
            resources.await_resources_loaded(client, ResourceStage::Lobby)
        }).context("missing client resources")??;

        let server_chat = self.chat.clone();
        client.run_async(
            resource_task, 
            move |client, _| {
                client.send_packet(&s2c::LobbyLayoutSwitchEnd{ state: LayoutState::BattleSelect, origin: LayoutState::BattleSelect });
                client.register_component(ServerChatComponent::new(server_chat));
            }
        );

        Ok(())
    }

    fn handle_event(&mut self, event: ServerEvent) -> anyhow::Result<()> {
        match event {
            ServerEvent::ClientDisconnected(client_id) => {
                info!("Client {} disconnected.", client_id);
                self.clients.remove(&client_id);
            },
            ServerEvent::ClientAuthenticated(client_id) => {
                self.handle_client_authenticated(client_id)?;
            },
            _ => {

            }
        }
    
        Ok(())
    }
}

impl Future for Server {
    type Output = ();

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        loop {
            match self.events_rx.poll_recv(cx) {
                Poll::Ready(Some(event)) => {
                    match self.handle_event(event) {
                        Ok(_) => {}
                        Err(error) => {
                            tracing::error!("server event error: {}", error);
                        },
                    }
                },
                _ => break,
            }
        }

        Poll::Pending
    }
}