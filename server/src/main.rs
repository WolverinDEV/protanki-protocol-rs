#![feature(drain_filter)]
#![feature(iterator_try_collect)]
#![feature(btree_drain_filter)]
#![allow(unused)]
use std::{net::SocketAddr, sync::{Arc, Mutex}, task::Poll, future::poll_fn};

use futures::FutureExt;
use tokio::net::TcpSocket;
use tracing::{Level, info, debug, warn};
use tracing_subscriber::EnvFilter;
use tracing::{ error };

use crate::{client::Client, server::Server};

mod client;
mod server;
mod client_components;
mod users;

mod tasks;
pub use tasks::*;

mod resources;
pub use resources::*;

mod chat;
pub use chat::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(Level::TRACE)
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let addr: SocketAddr = "127.0.0.1:1235".parse()?;

    let socket = TcpSocket::new_v4()?;
    socket.bind(addr.clone())?;

    let socket = socket.listen(5)?;
    info!("Server started on {}", addr);

    let server = Server::new()?;
    let server = Arc::new(Mutex::new(server));

    {
        let server = server.clone();
        tokio::spawn(poll_fn(move |cx| {
            let mut server = match server.lock() {
                Ok(server) => server,
                Err(_) => return Poll::Ready(())
            };

            server.poll_unpin(cx)
        }));
    }

    loop {
        let (stream, socket_address) = match socket.accept().await {
            Ok(client) => client,
            Err(error) => {
                error!("failed to accept client: {}", error);
                break;
            }
        };

        let server = server.clone();
        tokio::task::spawn(async move {
            let client = Client::new(
                Box::new(stream),
                socket_address
            ).await;

            let client = match client {
                Ok(client) => client,
                Err(error) => {
                    warn!("failed to initialize client: {}", error);
                    return;
                }
            };

            let mut server = server.lock().unwrap();
            server.register_client(client);
        });
    }
    Ok(())
}