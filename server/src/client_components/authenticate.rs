use std::{task, time::Duration, pin::Pin, sync::{Arc, RwLock, atomic::{AtomicBool, Ordering}}};

use anyhow::Context;
use fast_socks5::server::Authentication;
use fost_protocol::{packets::{self, PacketDowncast, s2c, c2s}, codec::{CaptchaLocation, LayoutState, ChatCC, ChatModeratorLevel, UserPropertyCC}};
use futures::FutureExt;
use tokio::time;
use tracing::{debug, info, warn};

use crate::{client::{ClientComponent, Client, AuthenticationState}, users::{UserRegistry, AuthenticationResult}, ResourceStage, server::ServerEvent};

use super::{CaptchaProvider, ClientResources};

/// Request the client to load all required auth resources
/// and fire ResourceLoaderFinished when done.
pub struct LoginKickoff;
impl LoginKickoff {
    pub fn new() -> Self {
        Self { }
    }
}
impl ClientComponent for LoginKickoff {
    fn initialize(&mut self, client: &mut Client) -> anyhow::Result<()> {
        client.with_component_mut::<ClientResources, _>(
            |client, resources| {
                let resources_auth = resources.await_resources_loaded(client, ResourceStage::Auth)?;

                client.run_async(
                    resources_auth, 
                    |client, _| {
                        /* Resources loaded. Show the login screen. */        
                        client.send_packet(&packets::s2c::ResourceLoaderFinished{});
                    }
                );

                anyhow::Ok(())
            }
        ).context("failed to find client resources")??;

        Ok(())
    }
}

pub struct UserAuthentication {
    user_registry: Arc<RwLock<UserRegistry>>,
    login_pending: Arc<AtomicBool>,
}

impl UserAuthentication {
    pub fn new(user_registry: Arc<RwLock<UserRegistry>>) -> Self {
        Self {
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
                if let Some(token) = token {
                    client.send_packet(&s2c::AccountLoginHashUpdate{ hash: token });
                } else {
                    /* Failed to create a token. Just don't send anything to the client. */
                }
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
}