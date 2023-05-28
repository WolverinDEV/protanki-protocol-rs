use std::{task, time::Duration, pin::Pin, sync::{Arc, RwLock, atomic::{AtomicBool, Ordering}}};

use anyhow::Context;
use fast_socks5::server::Authentication;
use fost_protocol::{packets::{self, PacketDowncast, s2c, c2s}, codec::{CaptchaLocation, LayoutState, ChatCC, ChatModeratorLevel, UserPropertyCC}};
use futures::FutureExt;
use tokio::time;
use tracing::{debug, info, warn};

use crate::{client::{ClientComponent, Client, AuthenticationState}, users::{UserRegistry, AuthenticationResult}, ResourceStage, server::ServerEvent};

use super::{CaptchaProvider, ClientResources};

pub struct UserAuthentication {
    initialized: bool,

    user_registry: Arc<RwLock<UserRegistry>>,
    login_pending: Arc<AtomicBool>,
}

impl UserAuthentication {
    pub fn new(user_registry: Arc<RwLock<UserRegistry>>) -> Self {
        Self {
            initialized: false,

            user_registry,
            login_pending: Arc::new(AtomicBool::new(false)),
        }
    }

    fn handle_authentication_result(&mut self, client: &mut Client, result: AuthenticationResult, remember: bool) {
        match result {
            AuthenticationResult::InvalidCredentials => {
                /* TODO: keep track of failed logins and request captcha */
                debug!("failed login attempt (credentials)");
                client.send_packet(&s2c::AccountLoginFailure{});
            },
            AuthenticationResult::InvalidToken => {
                /* TODO: keep track of failed logins and disconnect client */
                debug!("failed login attempt (token)");
                client.send_packet(&s2c::AccountLoginHashLoginFailed{});
            },
            AuthenticationResult::Success { user_id } => {
                client.send_packet(&s2c::AccountLoginSuccess{});
                self.handle_user_authenticated(client, &user_id, remember);
            }
        }
    }

    pub fn handle_user_authenticated(&mut self, client: &mut Client, user_id: &str, remember: bool) {
        info!("client authenticated for {}", user_id);
        *client.authentication_state_mut() = AuthenticationState::Authenticated { user_id: user_id.to_string() };
        if remember {
            self.send_login_token(client);
        }
        
        client.issue_server_event(ServerEvent::ClientAuthenticated(client.client_id()));
        /* this only toggles the load screen */
        client.send_packet(&s2c::LobbyLayoutSwitchStart{ state: LayoutState::BattleSelect });

        // Starts the user account bar
        client.send_packet(&s2c::AccountInfoProperties {
            user_property_cc: UserPropertyCC {
                id: user_id.to_string(),
                user_profile_url: "https://did.science/".to_string(),

                server_number: 1,

                score: 133,
                current_rank_score: 100,
                next_rank_score: 200,
                
                rank: 2,
                rating: 1337f32,
                place: 3,

                crystals: 123,
                duration_crystal_abonement: 8000,
                has_double_crystal: true,
            }
        });

        // TODO: Notify premium data

        // TODO: Notify EMail
        client.send_packet(&s2c::AccountCredentialsInit{
            email: "dev@did.science".to_string(),
            email_confirmed: true
        });

        
        client.run_async(
            async {
                time::sleep(Duration::from_secs(1)).await
            }, 
            |client, _| {
                client.send_packet(&s2c::LobbyLayoutSwitchEnd{ state: LayoutState::BattleSelect, origin: LayoutState::BattleSelect });
            }
        );
    }

    pub fn send_login_token(&mut self, client: &mut Client) -> anyhow::Result<()> {
        let user_id = match client.authentication_state() {
            AuthenticationState::Unauthenticated => anyhow::bail!("client unauthenticated"),
            AuthenticationState::Authenticated { user_id } => user_id.clone()
        };

        let user_registry = self.user_registry.read()
            .ok()
            .context("failed to aquite the user registry")?; 

        client.run_async(
            user_registry.create_authentication_token(user_id), 
            |client, token| {
                client.send_packet(&s2c::AccountLoginHashUpdate{ hash: token });
            }
        );
        Ok(())
    }

    fn initialize_resources(&mut self, client: &mut Client, resources: &mut ClientResources) -> anyhow::Result<()> {
        let resources_connect = resources.await_resources_loaded(client, ResourceStage::Connect)?;
        //let resources_auth = resources.await_resources_loaded(client, ResourceStage::Auth)?;

        client.run_async(
            async move {
                resources_connect.await;
                //resources_auth.await;
            }, 
            |client, _| {
                /* Resources loaded. Show the login screen. */        
                client.send_packet(&packets::s2c::ResourceLoaderFinished{});
            }
        );

        Ok(())
    }
}

impl ClientComponent for UserAuthentication {
    fn on_packet(&mut self, client: &mut Client, packet: &dyn packets::Packet) -> anyhow::Result<()> {
        if let Some(packet) = packet.downcast_ref::<c2s::AccountLoginExecute>() {
            if !matches!(client.authentication_state(), AuthenticationState::Unauthenticated) {
                anyhow::bail!("client is not supposed to login")
            }

            let login_pending = self.login_pending.clone();
            if login_pending.swap(true, Ordering::Relaxed) {
                anyhow::bail!("login attempt still pending")
            }

            let user_registry = self.user_registry.read()
                .ok()
                .context("failed to aquite the user registry")?;
            
                
            let username = packet.login.to_string();
            let remember = packet.remember;
            client.run_async(
                user_registry.authenticate_with_credentials(packet.login.to_string(), packet.password.to_string()), 
            move |client, result| {
                login_pending.store(false, Ordering::Relaxed);
                
                client.with_component_mut::<UserAuthentication, _>(
                    |client, authentication| {
                        authentication.handle_authentication_result(client, result, remember);
                    }
                );
            });
        } else if let Some(packet) = packet.downcast_ref::<c2s::AccountLoginHashLogin>() {
            let user_registry = self.user_registry.read()
                .ok()
                .context("failed to aquite the user registry")?;

            client.run_async(
                user_registry.authenticate_with_token(packet.hash.clone()),
                |client, result| {
                    client.with_component_mut::<UserAuthentication, _>(
                        |client, authentication| {
                            authentication.handle_authentication_result(client, result, true);
                        }
                    );
                }
            );
        }

        Ok(())
    }

    fn poll(&mut self, client: &mut Client, cx: &mut task::Context) -> anyhow::Result<()> {
        if self.initialized {
            return Ok(());
        }
        self.initialized = true;

        client.with_component_mut::<ClientResources, _>(
            |client, resources| {
                self.initialize_resources(client, resources)
            }
        )
        .context("failed to find client resources")??;
        
        Ok(())
    }
}