use std::sync::{Arc, Mutex, RwLock};

use anyhow::Context;
use fost_protocol::{packets::{Packet, PacketDowncast, c2s, s2c}, codec::{CaptchaLocation, ResourceReference}};

use crate::{client::{ClientComponent, Client}, users::UserRegistry};

use super::CaptchaProvider;

pub struct UserRegister {
    client_initialized: bool,
    user_registry: Arc<RwLock<UserRegistry>>,
}

impl UserRegister {
    pub fn new(user_registry: Arc<RwLock<UserRegistry>>) -> Self {
        Self {
            client_initialized: false,
            user_registry,
        }
    }
}

impl ClientComponent for UserRegister {
    fn on_packet(&mut self, client: &mut Client, packet: &dyn Packet) -> anyhow::Result<()> {
        if let Some(packet) = packet.downcast_ref::<c2s::AccountRegisterValidateUid>() {
            let user_registry = self.user_registry.read()
                .ok()
                .context("failed to aquite the user registry")?;

            client.run_async(
                user_registry.validate_username(packet.uid.clone()),
                |client, result| {
                    if result {
                        client.send_packet(&s2c::AccountRegisterUidFree{ });
                    } else {
                        client.send_packet(&s2c::AccountRegisterUidIncorrect{ });
                    }
                }
            );
        } else if let Some(packet) = packet.downcast_ref::<c2s::AccountRegisterSubmit>() {
            let captcha_valid = {
                let mut captcha_service = client.get_component_mut::<CaptchaProvider>()
                    .context("missing captcha service")?;

                captcha_service.solved_for(CaptchaLocation::RegisterForm)
            };

            if !captcha_valid {
                client.send_packet(&s2c::AccountRegisterCaptchaRequired{});
                return Ok(());
            }

            let mut user_registry = self.user_registry.write()
                .ok()
                .context("failed to aquite the user registry")?;

            client.run_async(
                user_registry.register_user(packet.uid.to_string(), packet.password.to_string()), 
                |client, result| {
                    if result {
                        /* TODO! */
                        client.send_packet(&s2c::AlertShow{ text: "Registered! Next step is TODO!".to_string() });
                    } else {
                        client.send_packet(&s2c::AccountRegisterUidIncorrect{});
                    }
                }
            );
        }
        Ok(())
    }

    fn poll(&mut self, client: &mut crate::client::Client, _cx: &mut std::task::Context) -> anyhow::Result<()> {
        if self.client_initialized {
            return Ok(())
        }

        self.client_initialized = true;
        client.send_packet(&s2c::AccountRegisterParameters{
            bg_resource: ResourceReference{ resource_id: 122842 },
            enable_required_email: false,
            max_password_length: 100,
            min_password_length: 5
        });
        Ok(())
    }
}