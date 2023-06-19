#![feature(iterator_try_collect)]
#![feature(trait_alias)]
#![allow(unused)]
use std::{net::SocketAddr, sync::{Arc, Mutex, RwLock}, task::Poll, future::poll_fn};

use anyhow::Context;
use futures::FutureExt;
use tokio::net::TcpSocket;
use tracing::{Level, info, debug, warn};
use tracing_subscriber::EnvFilter;
use tracing::{ error };
use sqlx::{SqliteConnection, Connection, ConnectOptions, sqlite::{SqliteConnectOptions, SqliteJournalMode}};

use crate::{client::Client, server::Server};

mod client;
mod server;
mod client_components;
mod users;
mod rank;
pub use rank::*;

mod tasks;
pub use tasks::*;

mod resources;
pub use resources::*;

mod chat;
pub use chat::*;

mod battle;
pub use battle::*;

mod battles;
pub use battles::*;

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

    let mut database = SqliteConnectOptions::default()
        .filename("database.sqlite")
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .connect().await?;

    sqlx::migrate!("./migrations")
        .run(&mut database)
        .await?;

    let database = Arc::new(tokio::sync::Mutex::new(database));

    let server = Server::new(database)?;
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
        let accept_event = tokio::select! {
            event = socket.accept() => event,
            _ = tokio::signal::ctrl_c() => break,
        };
        let (stream, socket_address) = match accept_event {
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

    tracing::info!("Server shutdown");
    drop(socket); /* close the server socket */

    let server_shutdown = {
        let mut server = server.lock().unwrap();
        server.shutdown()
    };
    if !server_shutdown.await {
        tracing::warn!("Server already stopping. We can not wait for the server to stop and exit process anyway.");
    }
    tracing::info!("Server stopped.");
    Ok(())
}