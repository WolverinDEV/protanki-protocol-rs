use std::collections::BTreeMap;

use fost_protocol::{codec::CaptchaLocation, packets::{self, PacketDowncast, c2s, s2c}};

use crate::client::ClientComponent;

static DEFAULT_CAPTCHA: &[u8] = include_bytes!("./captcha.png");

#[derive(Debug)]
enum SolveState {
    Invalid,
    Pending { code: String },
    Solved,
}

pub struct CaptchaProvider {
    client_initialized: bool,
    solve_states: BTreeMap<CaptchaLocation, SolveState>
}

impl CaptchaProvider {
    pub fn new() -> Self {
        let mut provider = Self {
            client_initialized: false,
            solve_states: Default::default(),
        };

        provider.require_for(CaptchaLocation::RegisterForm);
        provider.require_for(CaptchaLocation::ClientStartup);

        provider
    }

    pub fn require_for(&mut self, location: CaptchaLocation) {
        self.solve_states.insert(location, SolveState::Invalid);
    }

    pub fn required_for(&self, location: CaptchaLocation) -> bool {
        self.solve_states.contains_key(&location)
    }

    /// Tests if the captcha has been solved for a certain location and invalidates the
    /// captcha for that location.
    pub fn solved_for(&mut self, location: CaptchaLocation) -> bool {
        let state = match self.solve_states.get_mut(&location) {
            Some(state) => state,
            None => return true,
        };

        match state {
            SolveState::Invalid => false,
            SolveState::Pending { .. } => false,
            SolveState::Solved => {
                self.invalidate_for(location);
                true
            }
        }
    }

    pub fn invalidate_for(&mut self, location: CaptchaLocation) {
        if let Some(state) = self.solve_states.get_mut(&location) {
            *state = SolveState::Invalid;
        }
    }

    fn send_new_captcha(&mut self, client: &mut crate::client::Client, location: CaptchaLocation, is_show: bool) {
        let captcha_data = DEFAULT_CAPTCHA.to_vec();
        let code: String = "WolverinDEV".to_string();

        if is_show {
            client.send_packet(&s2c::CaptchaShow{
                location,
                captcha_data: captcha_data,
            });
        } else {
            client.send_packet(&s2c::CaptchaCaptchaFailed{
                location,
                new_captcha: captcha_data,
            });
        }
        
        self.solve_states.insert(location, SolveState::Pending { code });
    }
}

impl ClientComponent for CaptchaProvider {
    fn on_packet(&mut self, client: &mut crate::client::Client, packet: &dyn packets::Packet) -> anyhow::Result<()> {
        if let Some(packet) = packet.downcast_ref::<c2s::CaptchaRequestLocation>() {
            self.send_new_captcha(client, packet.location, true);
        } else if let Some(packet) = packet.downcast_ref::<c2s::CaptchaValidateCaptcha>() {
            let state = match self.solve_states.get_mut(&packet.location) {
                Some(state) => state,
                None => {
                    /* client tried to solve a captcha for a location which is not required */
                    client.send_packet(&s2c::CaptchaCaptchaValidated{ location: packet.location });
                    return Ok(())
                }
            };

            match state {
                SolveState::Invalid => {
                    /* no captcha present yet */
                    self.send_new_captcha(client, packet.location, false);
                },
                SolveState::Pending { code } => {
                    if code == &packet.code {
                        *state = SolveState::Solved;
                        client.send_packet(&s2c::CaptchaCaptchaValidated{ location: packet.location });
                    } else {
                        self.send_new_captcha(client, packet.location, false);
                    }
                },
                SolveState::Solved => {
                    /* Captcha has already been solved. No need to solve it again. */
                }
            }
        }
        Ok(())
    }

    fn poll(&mut self, client: &mut crate::client::Client, _cx: &mut std::task::Context) -> anyhow::Result<()> {
        if self.client_initialized {
            return Ok(());
        }

        self.client_initialized = true;
        
        client.send_packet(&packets::s2c::CaptchaParameters{
            init_params: self.solve_states.keys().cloned().collect::<Vec<_>>()
        });

        Ok(())
    }
}