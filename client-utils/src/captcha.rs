use std::time::Duration;

use async_trait::async_trait;
use fost_protocol::{packets::{self, PacketDowncast}, codec::CaptchaLocation};
use tracing::{debug};

use crate::Session;


#[async_trait]
pub trait CaptchaSolver : Send {
    async fn solve_captcha(&mut self, captcha: Vec<u8>) -> anyhow::Result<String>;
}

pub struct CaptchaSolver2Captcha {
    auth_key: String,
    client: reqwest::Client
}

impl CaptchaSolver2Captcha {
    pub fn new(auth_key: String) -> Self {
        CaptchaSolver2Captcha { auth_key, client: reqwest::Client::new() }
    }
    async fn enqueue_request(&mut self, captcha: Vec<u8>) -> anyhow::Result<String> {
        let image = reqwest::multipart::Part::bytes(captcha)
        .file_name("captcha.png")
        .mime_str("image/png")?;

        let form = reqwest::multipart::Form::new()
            .part("file", image)
            .text::<&str, String>("key", self.auth_key.to_owned());

        let text = self.client.post("http://2captcha.com/in.php")
            .multipart(form)
            .send()
            .await?
            .text().await?;

        if !text.starts_with("OK|") {
            anyhow::bail!("expected OK| but got {}", text);
        }

        Ok(text[3..].trim().to_owned())
    }

    async fn get_response(&mut self, request_id: &str) -> anyhow::Result<Option<String>> {
        let response = self.client.get("http://2captcha.com/res.php")
            .query(&[
                ("key", self.auth_key.as_str()),
                ("action", "get"),
                ("id", &request_id)
            ])
            .send()
            .await?
            .text()
            .await?;

        if response.contains("CAPCHA_NOT_READY") {
            return Ok(None);
        }

        if !response.starts_with("OK|") {
            anyhow::bail!("expected OK| but got {}", response);
        }

        return Ok(Some(response[3..].trim().to_owned()))
    }
}

#[async_trait]
impl CaptchaSolver for CaptchaSolver2Captcha {
    async fn solve_captcha(&mut self, captcha: Vec<u8>) -> anyhow::Result<String> {
        let request_id = self.enqueue_request(captcha).await?;
        loop {
            /* recommanded are 5 seconds but who cares */
            tokio::time::sleep(Duration::from_millis(1000)).await;

            match self.get_response(&request_id).await {
                Ok(Some(code)) => return Ok(code),
                Ok(None) => continue,
                Err(error) => return Err(error),
            }
        }
    }
}

pub async fn solve_captcha(client: &mut Session, location: CaptchaLocation, solver: &mut dyn CaptchaSolver) -> anyhow::Result<bool> {
    client.connection.send_packet(&packets::c2s::CaptchaRequestLocation{ location })?;
    let captcha_data = client.await_match(
        move |_, packet| {
            if let Some(captcha) = packet.downcast_ref::<packets::s2c::CaptchaShow>() {
                if captcha.captcha_location == location {
                    Some(captcha.captcha_data.clone())
                } else {
                    None
                }
            } else {
                None
            }
        }
    ).await?;

    let captcha_value = solver.solve_captcha(captcha_data).await?;

    client.connection.send_packet(&packets::c2s::CaptchaValidateCaptcha{ captcha_location: location, var_1950: captcha_value.clone() })?;
    let captcha_solved = client.await_match(
        move |_, packet| {
            if let Some(packet) = packet.downcast_ref::<packets::s2c::CaptchaCaptchaValidated>() {
                if packet.captcha_location == location {
                    return Some(true);
                }
            } else if let Some(packet) = packet.downcast_ref::<packets::s2c::CaptchaCaptchaFailed>() {
                if packet.captcha_location == location {
                    return Some(false);
                }
            }

            None
        }
    ).await?;

    debug!("Captcha solved: {} ({})", captcha_solved, captcha_value);
    Ok(captcha_solved)
}
