use std::sync::{Arc, Mutex, RwLock};

use anyhow::Context;
use fost_protocol::{packets::{Packet, PacketDowncast, c2s, s2c}, codec::{CaptchaLocation, ResourceReference}};

use crate::{client::{ClientComponent, Client, AuthenticationState}, users::UserRegistry};

use super::{CaptchaProvider, UserAuthentication};

pub struct UserRegister {
    user_registry: Arc<RwLock<UserRegistry>>,
}

impl UserRegister {
    pub fn new(user_registry: Arc<RwLock<UserRegistry>>) -> Self {
        Self {
            user_registry,
        }
    }
}

impl ClientComponent for UserRegister {
    fn initialize(&mut self, client: &mut Client) -> anyhow::Result<()> {
        client.send_packet(&s2c::AccountRegisterParameters{
            bg_resource: ResourceReference{ resource_id: 122842 },
            enable_required_email: false,
            max_password_length: 100,
            min_password_length: 5
        });

        Ok(())
    }

    fn on_packet(&mut self, client: &mut Client, packet: &dyn Packet) -> anyhow::Result<()> {
        if let Some(packet) = packet.downcast_ref::<c2s::AccountRegisterValidateUid>() {
            if !matches!(client.authentication_state(), AuthenticationState::Unauthenticated) {
                anyhow::bail!("client is not supposed to register")
            }

            let user_registry = self.user_registry.read()
                .ok()
                .context("failed to accquire the user registry")?;

            client.run_async(
                user_registry.is_username_free(packet.uid.clone()),
                |client, result| {
                    if result {
                        client.send_packet(&s2c::AccountRegisterUidFree{ });
                    } else {
                        client.send_packet(&s2c::AccountRegisterUidBusy{ adviced_uids: vec![] });
                    }
                }
            );
        } else if let Some(packet) = packet.downcast_ref::<c2s::AccountRegisterSubmit>() {
            if !matches!(client.authentication_state(), AuthenticationState::Unauthenticated) {
                anyhow::bail!("client is not supposed to register")
            }
            
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

            let username = packet.uid.to_string();
            let remember = packet.remember_me;
            client.run_async(
                user_registry.register_user(packet.uid.to_string(), packet.password.to_string()), 
                move |client, result| {
                    if result {
                        client.send_packet(&s2c::AccountLoginSuccess{});
                        client.with_component_mut::<UserAuthentication, _>(
                            |client, authentication| {
                                authentication.handle_user_authenticated(client, &username, remember);
                            }
                        );
                    } else {
                        client.send_packet(&s2c::AccountRegisterUidIncorrect{});
                    }
                }
            );
        }
        Ok(())
    }
}