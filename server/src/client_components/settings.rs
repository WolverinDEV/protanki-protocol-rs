use fost_protocol::{packets::{Packet, PacketDowncast, c2s, s2c}, codec::{SocialNetworkPanelCC, SocialNetworkPanelParams}};

use crate::client::{ClientComponent, Client};

pub struct SettingsDialog {}
impl SettingsDialog {
    pub fn new() -> Self {
        Self {}
    }
}
impl ClientComponent for SettingsDialog {
    fn on_packet(&mut self, client: &mut Client, packet: &dyn Packet) -> anyhow::Result<()> {
        /* TODO: This currently does not work. Settings do not show up. */
        if let Some(_packet) = packet.downcast_ref::<c2s::SettingsRequestOpen>() {
            client.send_packet(&s2c::SocialNetworkPannelInit{
                init_params: SocialNetworkPanelCC{
                    password_created: true,
                    social_network_params: vec![
                        SocialNetworkPanelParams {
                            authorization_url: "https://did.science/".to_string(),
                            sn_id: "vkontakte".to_string(),
                            link_exists: false,
                        }
                    ]
                }
            });
            client.send_packet(&s2c::SettingsOpen{ notification_enabled: true });
        } else if let Some(_packet) = packet.downcast_ref::<c2s::SettingsRequestClose>() {
            client.send_packet(&s2c::SettingsClose{ });
        }

        Ok(())
    }
}