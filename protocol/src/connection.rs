use std::{net::SocketAddr, task::{Poll, Context, Waker}, io::Cursor, pin::Pin};

use crate::{crypto::{Cipher, PlainCipher, XOrCipher, CipherMode}, packets, ProtocolError, ProtocolResult, ConnectionClosedError, Socket};
use crate::packets::{Packet, PacketRegistry, PacketDowncast};
use byteorder::{ReadBytesExt, BigEndian, WriteBytesExt};
use futures::prelude::*;
use tracing::{warn, trace, debug};

pub trait PacketDebugFilter : Send {
    fn should_log(&self, is_send: bool, packet: &dyn Packet) -> bool;
}

#[derive(Debug, Clone)]
pub struct SimplePacketDebugFilter {
    enabled: bool,
}

impl SimplePacketDebugFilter {
    pub fn logging_enabled() -> Self {
        Self{ enabled: true }
    }

    pub fn logging_disabled() -> Self {
        Self{ enabled: false }
    }
}

impl PacketDebugFilter for SimplePacketDebugFilter {
    fn should_log(&self, _is_send: bool, _packet: &dyn Packet) -> bool {
        self.enabled
    }
}

#[derive(Debug, Clone)]
pub struct ModelPacketDebugFilter {
    model_ids: Vec<u32>,
    whitelist: bool,
}

impl ModelPacketDebugFilter {
    pub fn whitelist(model_ids: Vec<u32>) -> Self {
        Self { model_ids, whitelist: true }
    }
    
    pub fn blacklist(model_ids: Vec<u32>) -> Self {
        Self { model_ids, whitelist: false }
    }
}

impl PacketDebugFilter for ModelPacketDebugFilter {
    fn should_log(&self, _is_send: bool, packet: &dyn Packet) -> bool {
        if self.model_ids.contains(&packet.model_id()) {
            return self.whitelist;
        } else {
            return !self.whitelist;
        }
    }
}


pub type ConnectionStreamItem = std::result::Result<Box<dyn Packet>, ProtocolError>;

/// Simple tanks connection capeable of sending and receiving packets.
pub struct Connection {
    pub address: SocketAddr,
    socket: Box<dyn Socket + Send>,

    is_server: bool,
    crypt_context: Box<dyn Cipher>,

    disconnected: bool,
    log_filter: Box<dyn PacketDebugFilter>,

    recv_buffer: Vec<u8>,
    recv_buffer_index: usize,

    send_buffer: Vec<u8>,
    send_waker: Option<Waker>,

    packet_registry: PacketRegistry,
}

impl Connection {
    pub fn new(is_server: bool, address: SocketAddr, socket: Box<dyn Socket + Send>, log_filter: Box<dyn PacketDebugFilter>) -> Self {
        let mut instance = Self {
            address,
            socket,

            is_server,
            crypt_context: Box::new(PlainCipher::new(if is_server { CipherMode::Server } else { CipherMode::Client })),

            disconnected: false,
            log_filter,

            recv_buffer: Vec::with_capacity(1024 * 16),
            recv_buffer_index: 0,

            send_buffer: Vec::with_capacity(1024 * 16),
            send_waker: None,

            packet_registry: PacketRegistry::new()
        };

        /* register all packets */
        if is_server {
            packets::c2s::register_all_packets(&mut instance.packet_registry);
        } else {
            packets::s2c::register_all_packets(&mut instance.packet_registry);
        }
        
        instance
    }

    pub fn allow_unknown_packets(&mut self) {
        self.packet_registry.allow_unknown_packets();
    }

    pub fn send_packet(&mut self, packet: &dyn Packet) -> ProtocolResult<()> {
        let mut buffer = Vec::with_capacity(1024);
        let mut cursor = Cursor::new(&mut buffer);
        cursor.write_u32::<BigEndian>(0)?; /* total length can only be known after writing */
        cursor.write_u32::<BigEndian>(packet.packet_id())?;

        packet.encode(&mut cursor)?;
        
        let packet_length = cursor.position() as usize;
        cursor.set_position(0);
        cursor.write_u32::<BigEndian>(packet_length as u32)?;
        buffer.truncate(packet_length);

        if let Err(error) = self.crypt_context.encrypt(&mut buffer[8..]) {
            return Err(error.into());
        }

        self.send_buffer.append(&mut buffer);
        if let Some(waker) = self.send_waker.take() {
            waker.wake();
        }
        
        if self.log_filter.should_log(true, packet) {
            trace!("[OUT] {: >11} {: >2} {:?} ({} bytes)", packet.packet_id() as i32, packet.model_id(), packet, packet_length - 8);
        }
        Ok(())
    }

    pub async fn init_encryption(&mut self) -> ProtocolResult<()> {
        if self.is_server {
            let key = XOrCipher::generate_key();
            let crypt_context = XOrCipher::new(CipherMode::Server, &key);
            self.send_packet(&packets::s2c::ResourceLoaderInitializeEncryption{ protection_data: key })?;

            self.crypt_context = Box::new(crypt_context);
            Ok(())
        } else {
            let packet = match self.next().await {
                Some(Ok(packet)) => packet,
                Some(Err(event)) => return Err(event),
                None => return Err(ProtocolError::ConnectionAborted),
            };
    
            let packet = match packet.downcast_ref::<packets::s2c::ResourceLoaderInitializeEncryption>() {
                Some(packet) => packet,
                None => return Err(ProtocolError::UnexpectedPacket),
            };
    
            self.crypt_context = Box::new(XOrCipher::new(CipherMode::Client, &packet.protection_data));
            Ok(())
        }
    }

    fn try_parse_read_buffer(&mut self) -> Poll<ProtocolResult<Box<dyn Packet>>> {
        if self.recv_buffer_index < 8 {
            return Poll::Pending
        }

        let mut reader = Cursor::new(&self.recv_buffer[0..self.recv_buffer_index]);
        let packet_length = reader.read_u32::<BigEndian>()? as usize;
        let packet_id = reader.read_u32::<BigEndian>()?;

        if packet_length > 1024 * 1024 * 64 {
            return Poll::Ready(Err(ProtocolError::PacketTooLarge(packet_length)));
        } else if packet_length < 8 {
            return Poll::Ready(Err(ProtocolError::PacketTooSmall(packet_length)));
        }

        if self.recv_buffer_index < packet_length {
            if self.recv_buffer.len() < packet_length {
                self.recv_buffer.resize(packet_length, 0);
            }

            return Poll::Pending;
        }

        let payload_offset = reader.position() as usize;
        let packet_payload = &mut self.recv_buffer[payload_offset..packet_length];
        self.crypt_context.decrypt(packet_payload)?;

        let mut packet_reader = Cursor::new(packet_payload);
        let packet = self.packet_registry.decode(&mut packet_reader, packet_id)?;

        if !packet_reader.is_empty() {
            warn!("Packet decoder did not read whole packet of id {} ({} out of {} bytes left).", packet_id as i32, packet_reader.remaining_slice().len(), packet_length - 8);
        }

        if self.log_filter.should_log(false, Box::as_ref(&packet)) {
            trace!("[IN ] {: >11} {: >2} {:?} ({} bytes)", packet.packet_id() as i32, packet.model_id(), packet, packet_length - 8);
        }

        self.recv_buffer.copy_within(packet_length.., 0);
        self.recv_buffer_index -= packet_length;
        return Poll::Ready(Ok(packet));
    }

    fn poll_incoming(&mut self, cx: &mut Context) -> Poll<ConnectionStreamItem> {
        if self.recv_buffer.len() - self.recv_buffer_index < 1024 {
            self.recv_buffer.resize(self.recv_buffer.len() + 1024, 0);
        }

        loop {
            match self.try_parse_read_buffer() {
                Poll::Ready(Ok(packet)) => return Poll::Ready(Ok(packet)),
                Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                Poll::Pending => { /* not yet enough data */}
            }

            match self.socket.poll_recv(cx, &mut self.recv_buffer[self.recv_buffer_index..]) {
                Poll::Ready(Ok(length)) => {
                    if length == 0 {
                        return Poll::Ready(Err(ConnectionClosedError::Disconnected.into()))
                    }
                    
                    self.recv_buffer_index += length;
                },
                Poll::Ready(Err(error)) => return Poll::Ready(Err(ConnectionClosedError::ReadError(error).into())),
                Poll::Pending => return Poll::Pending
            }
        }
    }

    fn poll_outgoing(&mut self, cx: &mut Context) -> Poll<ConnectionStreamItem> {
        self.send_waker.replace(cx.waker().clone());
        while !self.send_buffer.is_empty() {
            match self.socket.poll_send(cx, &self.send_buffer) {
                Poll::Ready(Ok(length)) => {
                    self.send_buffer.copy_within(length.., 0);
                    self.send_buffer.truncate(self.send_buffer.len() - length);
                },
                Poll::Ready(Err(error)) => return Poll::Ready(Err(ConnectionClosedError::WriteError(error).into())),
                Poll::Pending => return Poll::Pending
            }
        }
        return Poll::Pending;
    }

    fn poll_io(&mut self, cx: &mut Context) -> Poll<ConnectionStreamItem> {
        match self.poll_outgoing(cx) {
            Poll::Ready(item) => return Poll::Ready(item),
            Poll::Pending => { }
        }

        match self.poll_incoming(cx) {
            Poll::Ready(item) => return Poll::Ready(item),
            Poll::Pending => {}
        }

        return Poll::Pending;
    }
}

impl Stream for Connection {
	type Item = ConnectionStreamItem;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.disconnected {
            return Poll::Ready(None);
        }

        match self.poll_io(cx) {
            Poll::Ready(Ok(item)) => Poll::Ready(Some(Ok(item))),
            Poll::Ready(Err(error)) => {
                debug!("connection error: {:?}", error);
                self.disconnected = true;
                return Poll::Ready(Some(Err(error)));
            },
            Poll::Pending => Poll::Pending
        }
    }
}