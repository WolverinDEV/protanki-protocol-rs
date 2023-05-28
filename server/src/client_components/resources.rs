use std::{sync::{Arc, RwLock}, time::{Instant, Duration}, pin::Pin};

use anyhow::Context;
use fost_protocol::packets::{s2c, PacketDowncast, c2s};
use futures::Future;
use serde::Serialize;
use tokio::sync::oneshot;

use crate::{ServerResources, client::{ClientComponent, Client}, ResourceStage, resources};

enum LoadRequestState {
    Enqueued { json: String },
    Pending { timestamp: Instant },
    Finished { duration: Duration },
}

struct LoadRequest {
    id: u32,
    stage: ResourceStage,

    state: LoadRequestState,

    finish_listener: Vec<oneshot::Sender<()>>,
}

pub struct ClientResources {
    server_resources: Arc<RwLock<ServerResources>>,

    requests: Vec<LoadRequest>,
    request_id: u32,

    request_pending: bool,
}

impl ClientResources {
    pub fn new(server_resources: Arc<RwLock<ServerResources>>) -> Self {
        Self {
            server_resources,
            requests: Default::default(),
            request_id: 0,
            request_pending: false,
        }
    }

    pub fn await_resources_loaded(&mut self, client: &mut Client, stage: ResourceStage) -> anyhow::Result<Pin<Box<dyn Future<Output = ()> + Send>>> {
        let request = self.requests.iter_mut()
            .find(|request| request.stage == stage);
        let request = match request {
            Some(request) => request,
            None => self.enqueue_load_request(client, stage)?,
        };

        if matches!(&request.state, LoadRequestState::Finished { .. }) {
            return Ok(Box::pin(async {}))
        }

        let (tx, rx) = oneshot::channel();
        request.finish_listener.push(tx);
        return Ok(Box::pin(async move {
            rx.await;
        }))
    }

    fn enqueue_load_request(&mut self, client: &mut Client, stage: ResourceStage) -> anyhow::Result<&mut LoadRequest> {
        let server_resources = self.server_resources.read()
            .ok()
            .context("failed to accquire server resources")?;

        #[derive(Serialize)]
        struct Resources {
            resources: Vec<resources::json::Resource>
        }

        let resources = server_resources.get_resources(stage)
            .into_iter()
            .map(|resource| resource.as_json_resource())
            .collect::<Vec<_>>();
        let resources = Resources{ resources };
        let json = serde_json::to_string(&resources)?;

        self.request_id += 1;
        let request_id = self.request_id;

        self.requests.push(LoadRequest { 
            id: request_id, 
            stage, 
            
            state: LoadRequestState::Enqueued { json },
            finish_listener: Default::default()
        });

        drop(server_resources);
        self.try_send_next_load_request(client);
        
        Ok(
            self.requests.last_mut()
                .expect("to be the current request")
        )
    }

    fn try_send_next_load_request(&mut self, client: &mut Client) {
        if self.request_pending {
            return;
        }

        let next_request = match {
            self.requests.iter_mut()
                .find(|entry| matches!(entry.state, LoadRequestState::Enqueued { .. }))
        } {
            Some(request) => request,
            None => return,
        };

        let old_state = std::mem::replace(&mut next_request.state, LoadRequestState::Pending { timestamp: Instant::now() });
        let json = match old_state {
            LoadRequestState::Enqueued { json } => json,
            _ => unreachable!()
        };

        self.request_pending = true;
        client.send_packet(&s2c::ResourceLoaderRegisterResources{
            callback_id: next_request.id as i32,
            json
        });
    }
}

impl ClientComponent for ClientResources {
    fn on_packet(&mut self, client: &mut Client, packet: &dyn fost_protocol::packets::Packet) -> anyhow::Result<()> {
        let packet = match packet.downcast_ref::<c2s::ResourceLoaderResourcesRegistered>() {
            Some(packet) => packet,
            None => return Ok(()),
        };

        let request = self.requests.iter_mut()
            .find(|request| request.id == packet.callback_id as u32)
            .context("invalid resource callback id")?;

        match &request.state {
            LoadRequestState::Enqueued { .. } => anyhow::bail!("request has not been send yet"),
            LoadRequestState::Finished { .. } => anyhow::bail!("request already completed"),
            LoadRequestState::Pending { timestamp } => {
                request.state = LoadRequestState::Finished { duration: timestamp.elapsed() };
            },
        };

        let listener = std::mem::replace(&mut request.finish_listener, Default::default());
        for listener in listener {
            listener.send(());
        }

        self.request_pending = false;
        self.try_send_next_load_request(client);
        Ok(())
    }
}
