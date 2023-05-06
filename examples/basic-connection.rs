use futures::StreamExt;
use tank_bot_rs::{TanksClient, ConnectionStreamItem, packets::{self, PacketDowncast}};
use tracing::{Level, info};
use clap::Parser;

#[derive(Parser, Debug)]
struct Args {
    /// Target server address
    #[arg(short, long)]
    target: String,

    /// Client login hash
    #[arg(short, long)]
    login_hash: Option<String>,
    
    /// Target language code
    #[arg(long, default_value = "en")]
    language_code: String,
    
    /// Target language code
    #[arg(long, default_value = "true")]
    log_protocol: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(Level::TRACE)
        .init();

    let args: Args = Args::parse();

    let mut client = TanksClient::builder()
        .set_lang_code(args.language_code)
        .set_log_packets(args.log_protocol)
        .connect(args.target.parse()?).await?;

    info!("Client connected.");
    while let Some(result) = client.connection.next().await {
        if let ConnectionStreamItem::Packet(packet) = result {
            if let Some(packet) = packet.downcast_ref::<packets::S2CResourceLoaderLoadDependencies>() {
                client.connection.send_packet(&packets::C2SResourceLoaderDependenciesLoaded{
                    callback_id: packet.callback_id
                })?;

                client.connection.send_packet(&packets::C2SResourceLoaderDependenciesLoaded{
                    callback_id: packet.callback_id
                })?;
            } else if let Some(_) = packet.downcast_ref::<packets::S2CPingMeasurePing>() {
                client.connection.send_packet(&packets::C2SPingMeasurePong{})?;
            } else if let Some(_) = packet.downcast_ref::<packets::S2CResourceLoaderResourcesLoaded>() {
                info!("Client loaded and viewing the login screen.");
                if let Some(hash) = &args.login_hash {
                    client.connection.send_packet(&packets::C2SAccountLoginHashLogin{
                        hash: hash.clone()
                    })?;
                }
            }
        }
    }
    
    info!("Client disconnected.");
    Ok(())
}
