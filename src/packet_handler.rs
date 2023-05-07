use std::{cell::{RefCell}, task::{Poll, Context}, collections::BTreeMap, sync::atomic::{AtomicU32, Ordering}};
use tokio::sync::oneshot;

use crate::{packets::{self, Packet, PacketDowncast}, TanksClient};

type PacketHandlerId = u32;

pub trait PacketHandler {
    fn handle_packet(&mut self, _client: &mut TanksClient, _packet: &dyn Packet) -> anyhow::Result<()> {
        Ok(())
    }

    fn poll(&mut self, _client: &mut TanksClient, _cx: &mut Context) -> Poll<anyhow::Result<()>> {
        Poll::Pending
    }
}

#[derive(Default)]
pub struct PacketHandlerRegistry {
    handler: RefCell<BTreeMap<PacketHandlerId, Box<dyn PacketHandler + Send>>>,
    handler_index: AtomicU32,
    pending_handler_updates: RefCell<BTreeMap<PacketHandlerId, Option<Box<dyn PacketHandler + Send>>>>,
}

impl PacketHandlerRegistry {
    pub fn register_handler(&self, handler: impl PacketHandler + Send + 'static) -> PacketHandlerId {
        let handler_id = 1 + self.handler_index.fetch_add(1, Ordering::Relaxed);

        match self.handler.try_borrow_mut() {
            Ok(mut handlers) => {
                handlers.insert(handler_id, Box::new(handler));
            },
            _ => {
                self.pending_handler_updates.borrow_mut().insert(handler_id, Some(Box::new(handler)));
            }
        };

        handler_id
    }

    pub fn remove_handler(&self, handler_id: PacketHandlerId) {
        match self.handler.try_borrow_mut() {
            Ok(mut handler) => {
                handler.remove(&handler_id);
            },
            _ => {
                self.pending_handler_updates.borrow_mut().insert(handler_id, None);
            }
        };
    }

    pub fn handle(&self, client: &mut TanksClient, packet: &dyn Packet) -> anyhow::Result<()> {
        let mut handlers = self.handler.borrow_mut();
        for handler in handlers.values_mut() {
            handler.handle_packet(client, packet)?;
        }
        drop(handlers);

        self.commit_post_handle_updates();
        Ok(())
    }

    pub fn poll(&self, client: &mut TanksClient, cx: &mut std::task::Context<'_>) -> Poll<anyhow::Error> {
        let mut handlers = self.handler.borrow_mut();
        for (handler_id, handler) in handlers.iter_mut() {
            match handler.poll(client, cx) {
                Poll::Ready(Ok(())) => self.remove_handler(*handler_id),
                Poll::Ready(Err(error)) => return Poll::Ready(error),
                Poll::Pending => {},
            }
        }
        drop(handlers);

        self.commit_post_handle_updates();   
        Poll::Pending
    }
    
    fn commit_post_handle_updates(&self) {
        let mut handler = self.handler.borrow_mut();
        let mut pending_handler = self.pending_handler_updates.borrow_mut();

        if pending_handler.is_empty() {
            return;
        }

        let pending_handler = std::mem::replace(&mut *pending_handler, Default::default());
        for (key, value) in pending_handler.into_iter() {
            if let Some(value) = value {
                handler.insert(key, value);
            } else {
                handler.remove(&key);
            }
        }
    }
}



pub struct LowLevelPing;
impl PacketHandler for LowLevelPing {
    fn handle_packet(&mut self, client: &mut TanksClient, packet: &dyn Packet) -> anyhow::Result<()> {
        let _packet = match packet.downcast_ref::<packets::S2CPingMeasurePing>() {
            Some(packet) => packet,
            None => return Ok(())
        };
        
        client.connection.send_packet(&packets::C2SPingMeasurePong{})?;
        Ok(())
    }
}

pub struct DummyResourceLoader;
impl PacketHandler for DummyResourceLoader {
    fn handle_packet(&mut self, client: &mut TanksClient, packet: &dyn packets::Packet) -> anyhow::Result<()> {
        let packet = match packet.downcast_ref::<packets::S2CResourceLoaderLoadDependencies>() {
            Some(packet) => packet,
            None => return Ok(()),
        };

        client.connection.send_packet(&packets::C2SResourceLoaderDependenciesLoaded{
            callback_id: packet.callback_id
        })?;
        Ok(())
    }
}


pub struct HandlerAwaitMatching<F: (Fn(&mut TanksClient, &dyn Packet) -> Option<R>) + Send, R: Send> {
    pub matcher: F,
    pub sender: Option<oneshot::Sender<R>>,
}

impl<F: (Fn(&mut TanksClient, &dyn Packet) -> Option<R>) + Send, R: Send> PacketHandler for HandlerAwaitMatching<F, R> {
    fn handle_packet(&mut self, client: &mut TanksClient, packet: &dyn Packet) -> anyhow::Result<()> {
        if let Some(result) = (self.matcher)(client, packet) {
            if let Some(sender) = self.sender.take() {
                let _ = sender.send(result);
            }
        }
    
        Ok(())
    }

    fn poll(&mut self, _client: &mut TanksClient, cx: &mut std::task::Context) -> Poll<anyhow::Result<()>> {
        if let Some(sender) = self.sender.as_mut() {
            match sender.poll_closed(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(_) => return Poll::Ready(Ok(()))
            }
        } else {
            Poll::Ready(Ok(()))
        }
    }
}