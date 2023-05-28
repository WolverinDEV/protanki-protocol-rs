use std::{task, time::Duration, pin::Pin, sync::{Arc, RwLock, atomic::{AtomicBool, Ordering}}};

use anyhow::Context;
use fost_protocol::{packets::{self, PacketDowncast, s2c, c2s}, codec::CaptchaLocation};
use futures::FutureExt;
use tokio::time;
use tracing::{debug, info};

use crate::{client::{ClientComponent, Client}, users::UserRegistry};

use super::CaptchaProvider;

pub struct UserAuthentication {
    user_registry: Arc<RwLock<UserRegistry>>,
    client_initialize_timer: Option<Pin<Box<time::Sleep>>>,

    login_pending: Arc<AtomicBool>,
}

impl UserAuthentication {
    pub fn new(user_registry: Arc<RwLock<UserRegistry>>) -> Self {
        Self {
            user_registry,
            client_initialize_timer: Some(
                Box::pin(time::sleep(Duration::from_millis(50)))
            ),
            login_pending: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl ClientComponent for UserAuthentication {
    fn on_packet(&mut self, client: &mut Client, packet: &dyn packets::Packet) -> anyhow::Result<()> {
        if let Some(packet) = packet.downcast_ref::<c2s::AccountLoginExecute>() {
            let login_pending = self.login_pending.clone();
            if login_pending.swap(true, Ordering::Relaxed) {
                anyhow::bail!("login attempt still pending")
            }

            let user_registry = self.user_registry.read()
                .ok()
                .context("failed to aquite the user registry")?;
            
                
            let username = packet.login.to_string();
            client.run_async(
                user_registry.authenticate_with_credentials(packet.login.to_string(), packet.password.to_string()), 
            move |client, result| {
                login_pending.store(false, Ordering::Relaxed);
                if result {
                    info!("client authenticated for {}", username);
                    client.send_packet(&s2c::AccountLoginSuccess{});
                } else {
                    /* TODO: keep track of failed logins and request captcha */
                    debug!("failed login attempt for account {}", username);
                    client.send_packet(&s2c::AccountLoginFailure{});
                }
            });
        }

        Ok(())
    }

    fn poll(&mut self, client: &mut Client, cx: &mut task::Context) -> anyhow::Result<()> {
        let mut sleep = match self.client_initialize_timer.as_mut() {
            Some(sleep) => sleep,
            None => return Ok(())
        };

        /* wait a little as we don't have to load any resources */
        if sleep.poll_unpin(cx).is_pending() {
            return Ok(());
        }

        self.client_initialize_timer = None;

        /* trigger the login screen to show. */        
        client.send_packet(&packets::s2c::ResourceLoaderFinished{});
        Ok(())
    }
}