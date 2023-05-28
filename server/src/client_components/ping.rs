use std::{time::{Duration, Instant}, task::Poll};

use fost_protocol::packets::{self, PacketDowncast};
use tokio::time;
use tracing::trace;

use crate::client::{ClientComponent, Client};

pub struct ConnectionPing {
    interval: time::Interval,
    send_timestamp: Option<Instant>,
}

impl ConnectionPing {
    pub fn new(period: Duration) -> Self {
        let mut interval = time::interval(period);
        interval.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
        Self {
            interval,
            send_timestamp: None,
        }
    }
}

impl ClientComponent for ConnectionPing {
    fn on_packet(&mut self, _client: &mut Client, packet: &dyn packets::Packet) -> anyhow::Result<()> {
        if !packet.is_type::<packets::c2s::PingMeasurePong>() {
            return Ok(())
        }

        let elapsed = match self.send_timestamp.take() {
            Some(timestamp) => timestamp.elapsed(),
            None => return Ok(())
        };

        //trace!("Connection speed: {:#?}", elapsed);
        Ok(())
    }

    fn poll(&mut self, client: &mut Client, cx: &mut std::task::Context) -> anyhow::Result<()> {
        while let Poll::Ready(_) = self.interval.poll_tick(cx) {
            if self.send_timestamp.is_some() {
                /* a ping is still pending */
                continue;
            }

            self.send_timestamp = Some(Instant::now());
            client.send_packet(&packets::s2c::PingMeasurePing {});
        }

        Ok(())
    }
}