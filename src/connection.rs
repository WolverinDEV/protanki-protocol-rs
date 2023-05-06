use std::{net::SocketAddr, task::{Poll, Context, Waker}, io::{Cursor, Read, Write}, pin::Pin};

use crate::{crypto::{CryptContext, NoCryptContext, XOrCryptContext}, packets};
use crate::packets::{Packet, PacketRegistry, PacketDowncast};
use tokio::{io::{self}, net::{TcpStream}};
use byteorder::{ReadBytesExt, BigEndian, WriteBytesExt};
use futures::prelude::*;
use tracing::{warn, trace, debug};

use crate::packets::UnknownPacket;

pub trait Socket {
    fn poll_recv(&self, cx: &mut Context, buf: &mut [u8]) -> Poll<io::Result<usize>>;
	fn poll_send(&self, cx: &mut Context, buf: &[u8]) -> Poll<io::Result<usize>>;
	fn local_addr(&self) -> io::Result<SocketAddr>;
}

impl Socket for TcpStream {
    fn poll_recv(&self, cx: &mut Context, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        loop {
            match TcpStream::poll_read_ready(&self, cx) {
                Poll::Ready(Ok(_)) => {},
                Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                Poll::Pending => return Poll::Pending,
            }
    
            match TcpStream::try_read(&self, buf) {
                Ok(length) => return Poll::Ready(Ok(length)),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(error) => return Poll::Ready(Err(error))
            }
        }
    }

    fn poll_send(&self, cx: &mut Context, buf: &[u8]) -> Poll<io::Result<usize>> {
        loop {
            match TcpStream::poll_write_ready(&self, cx) {
                Poll::Ready(Ok(_)) => {},
                Poll::Ready(Err(error)) => return Poll::Ready(Err(error)),
                Poll::Pending => return Poll::Pending,
            }
    
            match TcpStream::try_write(&self, buf) {
                Ok(length) => return Poll::Ready(Ok(length)),
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(error) => return Poll::Ready(Err(error))
            }
        }
    }

    fn local_addr(&self) -> io::Result<SocketAddr> {
        TcpStream::local_addr(&self)
    }
}

#[derive(Debug)]
pub enum ConnectionStreamItem {
    Packet(Box<dyn Packet>),
    DecodeError(anyhow::Error),
    RecvError(io::Error),
    SendError(io::Error),
}

pub struct Connection {
    address: SocketAddr,
    socket: Box<dyn Socket + Send>,

    is_server: bool,
    crypt_context: Box<dyn CryptContext>,

    disconnected: bool,
    log_packets: bool,

    recv_buffer: Vec<u8>,
    recv_buffer_index: usize,

    send_buffer: Vec<u8>,
    send_waker: Option<Waker>,

    packet_registry: PacketRegistry,
}

impl Connection {
    pub fn new(is_server: bool, address: SocketAddr, socket: Box<dyn Socket + Send>, log_packets: bool) -> Self {
        let mut instance = Self {
            address,
            socket,

            is_server,
            crypt_context: Box::new(NoCryptContext{}),

            disconnected: false,
            log_packets,

            recv_buffer: Vec::with_capacity(1024 * 16),
            recv_buffer_index: 0,

            send_buffer: Vec::with_capacity(1024 * 16),
            send_waker: None,

            packet_registry: PacketRegistry::new()
        };

        packets::register_all_packets(&mut instance.packet_registry);
        instance
    }

    pub fn send_packet(&mut self, packet: &dyn Packet) -> anyhow::Result<()> {
        let mut buffer = Vec::with_capacity(1024);
        let mut cursor = Cursor::new(&mut buffer);
        cursor.write_u32::<BigEndian>(0)?; /* total length can only be known after writing */
        cursor.write_u32::<BigEndian>(packet.packet_id())?;
        if let Some(packet) = packet.downcast_ref::<packets::UnknownPacket>() {
            cursor.write_all(packet.payload())?;
        } else {
            self.packet_registry.encode(&mut cursor, packet)?;
        }

        let packet_length = cursor.position() as usize;
        cursor.set_position(0);
        cursor.write_u32::<BigEndian>(packet_length as u32)?;
        buffer.truncate(packet_length);

        if let Err(error) = self.crypt_context.encrypt(&mut buffer[8..]) {
            return Err(error);
        }

        self.send_buffer.append(&mut buffer);
        if let Some(waker) = self.send_waker.take() {
            waker.wake();
        }
        
        if self.log_packets {
            trace!("[OUT] {: >11} {: >2} {:?} ({} bytes)", packet.packet_id() as i32, packet.model_id(), packet, packet_length - 8);
        }
        Ok(())
    }

    pub async fn init_encryption(&mut self) -> anyhow::Result<()> {
        if self.is_server {
            let (crypt_context, seed) = XOrCryptContext::new_server();
            self.send_packet(&packets::S2CResourceLoaderInitializeEncryption{ protection_data: seed })?;

            self.crypt_context = Box::new(crypt_context);
            Ok(())
        } else {
            let packet = match self.next().await {
                Some(ConnectionStreamItem::Packet(packet)) => packet,
                Some(event) => anyhow::bail!("connect failed: {:?}", event),
                None => anyhow::bail!("connection closed during setup"),
            };
    
            let packet = match packet.downcast_ref::<packets::S2CResourceLoaderInitializeEncryption>() {
                Some(packet) => packet,
                None => anyhow::bail!("expected a initialize encryption packet but received {}", packet.packet_id())
            };
    
            self.crypt_context = Box::new(XOrCryptContext::new_client(&packet.protection_data));
            Ok(())
        }
    }

    fn try_parse_read_buffer(&mut self) -> Poll<anyhow::Result<Box<dyn Packet>>> {
        if self.recv_buffer_index < 8 {
            return Poll::Pending
        }

        let mut reader = Cursor::new(&self.recv_buffer[0..self.recv_buffer_index]);
        let packet_length = reader.read_u32::<BigEndian>()? as usize;
        let packet_id = reader.read_u32::<BigEndian>()?;

        if packet_length > 1024 * 1024 * 64 {
            return Poll::Ready(Err(anyhow::anyhow!("packet too large (size: {})", packet_length)));
        }

        if self.recv_buffer_index < packet_length {
            if self.recv_buffer.len() < packet_length {
                self.recv_buffer.resize(packet_length, 0);
            }

            return Poll::Pending;
        }

        let payload_offset = reader.position() as usize;
        let packet_payload = &mut self.recv_buffer[payload_offset..packet_length];
        if let Err(error) = self.crypt_context.decrypt(packet_payload) {
            return Poll::Ready(Err(error));
        }

        let mut packet_reader = Cursor::new(packet_payload);
        let packet = match self.packet_registry.decode(&mut packet_reader, packet_id) {
            Ok(Some(packet)) => packet,
            Ok(None) => {
                // println!("Received unknown packet {} ({}) with length {}", packet_id, packet_id as i32, packet_length - 8);
                let mut buffer = Vec::with_capacity(packet_length);
                packet_reader.read_to_end(&mut buffer)?;

                Box::new(UnknownPacket::new(packet_id, buffer))
            },
            Err(error) => {
                return Poll::Ready(Err(error));
            },
        };

        if !packet_reader.is_empty() {
            warn!("Packet decoder did not read whole packet of id {} ({} bytes left).", packet_id, packet_reader.remaining_slice().len());
        }

        if self.log_packets {
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
                Poll::Ready(Ok(packet)) => return Poll::Ready(ConnectionStreamItem::Packet(packet)),
                Poll::Ready(Err(error)) => return Poll::Ready(ConnectionStreamItem::DecodeError(error)),
                Poll::Pending => { /* not yet enough data */}
            }

            match self.socket.poll_recv(cx, &mut self.recv_buffer[self.recv_buffer_index..]) {
                Poll::Ready(Ok(length)) => {
                    self.recv_buffer_index += length;
                },
                Poll::Ready(Err(error)) => return Poll::Ready(ConnectionStreamItem::RecvError(error)),
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
                Poll::Ready(Err(error)) => return Poll::Ready(ConnectionStreamItem::SendError(error)),
                Poll::Pending => return Poll::Pending
            }
        }
        return Poll::Pending;
    }

    fn poll_io(&mut self, cx: &mut Context) -> Poll<ConnectionStreamItem> {
        match self.poll_outgoing(cx) {
            Poll::Ready(item) => return Poll::Ready(item),
            Poll::Pending => {}
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
            Poll::Ready(item) => {
                match &item {
                    &ConnectionStreamItem::DecodeError(_) |
                    &ConnectionStreamItem::RecvError(_) |
                    &ConnectionStreamItem::SendError(_) => {
                        debug!("connection error: {:?}", item);
                        self.disconnected = true;
                    },
                    _ => {}
                }

                Poll::Ready(Some(item))
            },
            Poll::Pending => Poll::Pending
        }
    }
}