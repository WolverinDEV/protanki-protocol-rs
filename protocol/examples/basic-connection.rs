use std::fs::File;

use futures::StreamExt;
use fost_protocol::{Session, packets::{self, PacketDowncast}};
use serde::Deserialize;
use tracing::{Level, info, error, warn};
use clap::Parser;
use tracing_subscriber::EnvFilter;

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
    #[arg(long)]
    log_protocol: bool,
}

#[derive(Deserialize)]
struct Resources {
    pub resources: Vec<ResourceInfo>,
}

#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ResourceInfo {
    pub idhigh: i64,
    pub idlow: i64,

    pub versionhigh: i64,
    pub versionlow: i64,

    pub lazy: bool,
    pub alpha: Option<bool>,
    #[serde(rename = "type")]
    pub resource_type: i64,
    
    pub weight: Option<i64>,
    pub height: Option<i64>,
    pub num_frames: Option<i64>,
    pub fps: Option<i64>,
    pub file_names: Option<Vec<String>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(Level::TRACE)
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args: Args = Args::parse();

    let mut client = Session::builder()
        .set_lang_code(args.language_code)
        .set_log_packets(args.log_protocol)
        .connect(args.target.parse()?).await?;

    info!("Client connected.");
    info!("{}", env!("OUT_DIR"));
    while let Some(result) = client.connection.next().await {
        match result {
            Ok(packet) => {
                if let Some(packet) = packet.downcast_ref::<packets::s2c::ResourceLoaderRegisterResources>() {
                    use std::io::Write;
                    let mut file = File::options()
                        .create(true)
                        .write(true)
                        .open(format!("output_{}.txt", packet.callback_id))
                        .unwrap();
                    write!(&mut file, "{}", packet.json).unwrap();
                    client.connection.send_packet(&packets::c2s::ResourceLoaderResourcesRegistered{
                        callback_id: packet.callback_id
                    })?;
                } else if let Some(_) = packet.downcast_ref::<packets::s2c::PingMeasurePing>() {
                    client.connection.send_packet(&packets::c2s::PingMeasurePong{})?;
                } else if let Some(_) = packet.downcast_ref::<packets::s2c::ResourceLoaderFinished>() {
                    info!("Client loaded and viewing the login screen.");
                    if let Some(hash) = &args.login_hash {
                        client.connection.send_packet(&packets::c2s::AccountLoginHashLogin{
                            hash: hash.clone()
                        })?;
                    } else {
                        warn!("Missing login hash. Idle in login phase...");
                    }
                }
            },
            Err(err) => {
                error!("Connection error: {}", err);
                break;
            }
        }
    }
    
    info!("Client disconnected.");
    Ok(())
}
