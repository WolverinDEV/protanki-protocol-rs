use std::net::SocketAddr;

use futures::StreamExt;
use tank_bot_rs::{Connection, SimplePacketDebugFilter};
use tokio::net::{TcpSocket, TcpStream};
use tracing::{Level, info, warn, debug};
use clap::Parser;

#[derive(Parser, Debug)]
struct Args {
    /// Local server address
    #[arg(short, long)]
    bind: String,

    /// Target server address
    #[arg(short, long)]
    target: String,

    /// Target language code
    #[arg(long, default_value = "true")]
    log_protocol: bool,
}

async fn proxy_client(client: TcpStream, local_address: SocketAddr, target_address: SocketAddr, log_protocol: bool) -> anyhow::Result<()> {
    let server_socket = TcpSocket::new_v4()?;
    let server_stream = server_socket.connect(target_address).await?;

    let mut client_connection = Connection::new(
        true, 
        local_address, 
        Box::new(client), 
        if log_protocol {
            Box::new(SimplePacketDebugFilter::logging_enabled())
        } else {
            Box::new(SimplePacketDebugFilter::logging_disabled())
        }
    );
    let mut server_connection = Connection::new(
        false, 
        target_address, 
        Box::new(server_stream), 
        Box::new(SimplePacketDebugFilter::logging_disabled())
    );

    /* Await connection setup. */
    let (result_client, result_server) = tokio::join!(
        client_connection.init_encryption(),
        server_connection.init_encryption()
    );
    result_client?;
    result_server?;
    
    /* Connection started, proxy all packets. */
    debug!("Proxy connection setupped.");
    loop {
        tokio::select! {
            event = client_connection.next() => {
                let event = match event {
                    Some(event) => event,
                    None => {
                        info!("Client disconnect.");
                        break;
                    }
                };

                if let Ok(packet) = &event {
                    server_connection.send_packet(Box::as_ref(packet))?;
                }
            }
            event = server_connection.next() => {
                let event = match event {
                    Some(event) => event,
                    None => {
                        info!("Server disconnect.");
                        break;
                    }
                };

                if let Ok(packet) = &event {
                    client_connection.send_packet(Box::as_ref(packet))?;
                }
            }
        }
    }

    info!("Proxy session finished.");
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(Level::TRACE)
        .init();

    let args: Args = Args::parse();
    let target_address: SocketAddr = args.target.parse()?;

    let socket = TcpSocket::new_v4()?;
    socket.bind(args.bind.parse()?)?;
    let listener = socket.listen(5)?;

    loop {
        let (client, address) = listener.accept().await?;
        info!("Received new client from {}", address);

        let target_address = target_address.clone();
        tokio::task::spawn(async move {
            if let Err(error) = proxy_client(client, address, target_address, args.log_protocol).await {
                warn!("Proxy session error: {}", error);
            }
        });
    }
}
