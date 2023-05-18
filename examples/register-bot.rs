use std::{fs::File, io::{BufRead, BufReader}, time::Duration, net::SocketAddr, sync::{Arc, Mutex}};

use tank_bot_rs::{TanksClient, packets::{self, PacketDowncast}, codec::CaptchaLocation, packet_handler};
use tokio::{time::{self}, task};
use tracing::{Level, info, warn};
use clap::Parser;
use tracing_subscriber::EnvFilter;
use utils::{Proxy, CaptchaSolver, ProxyProvider, SocksProxyProvider, HostProxyProvider, CaptchaSolver2Captcha};
use std::io::Write;

use crate::utils::solve_captcha;

mod utils;

#[derive(Parser, Debug)]
struct Args {
    /// Target server address
    #[arg(short, long)]
    target: String,

    #[arg(short, long)]
    captcha2_key: String,

    #[arg(short, long)]
    username_file: String,

    #[arg(long)]
    proxy_file: Option<String>,

    #[arg(short, long)]
    password: String,

    #[arg(long, default_value_t = 1)]
    parallel_workers: usize,
}


#[derive(Debug, PartialEq, PartialOrd)]
enum RegisterResult {
    Success,
    UsernameBusy,
    UsernameInvalid,
    CaptchaRequired,
}

async fn register_account(client: &mut TanksClient, username: String, password: String) -> anyhow::Result<RegisterResult> {
    client.connection.send_packet(&packets::C2SAccountRegisterSubmit{
        password: password,
        uid: username,
        remember_me: true,    
    })?;

    client.await_match(
        |_, packet| {
            if packet.is_type::<packets::S2CAccountRegisterUidBusy>() {
                Some(RegisterResult::UsernameBusy)
            } else if packet.is_type::<packets::S2CAccountRegisterUidIncorrect>() {
                Some(RegisterResult::UsernameInvalid)
            } else if packet.is_type::<packets::S2CAccountRegisterCaptchaRequired>() {
                Some(RegisterResult::CaptchaRequired)
            } else if packet.is_type::<packets::S2CAccountRegisterUidFree>() {
                Some(RegisterResult::Success)
            } else {
                None
            }
        }
    ).await
}

async fn register_account_loop(proxy: &mut dyn Proxy, server: SocketAddr, username: String, password: String, captcha_solver: &mut dyn CaptchaSolver) -> anyhow::Result<RegisterResult> {
    info!("Generating for {}", username);

    let socket = proxy.create_stream(server.clone()).await?;
    let mut client = {
        TanksClient::builder()
            .set_lang_code("en")
            .set_log_packets(false)
            //.set_log_filter(ModelPacketDebugFilter::blacklist(vec![ 45 ]))
            //.connect(args.target.parse()?).await?;
            .connect_with_socket(server, socket).await?
    };

    client.register_packet_handler(packet_handler::DummyResourceLoader{});
    client.register_packet_handler(packet_handler::LowLevelPing{});
    
    info!("Client connected.");
    client.await_server_resources_loaded().await?;
    info!("Login screen opened");
    
    for _ in 0..3 {
        if solve_captcha(&mut client, CaptchaLocation::RegisterForm, captcha_solver).await? {
            break;
        }
    }

    let result = register_account(&mut client, username.to_string(), password.to_string()).await?;
    if result != RegisterResult::Success {
        return Ok(result)
    }

    tokio::select! {
        /* do not act too fast */
        _ = time::sleep(Duration::from_secs(12)) => {},
        _ = &mut client => {}
    }; 

    /* Sending these command might cause the server to close the connected. Then no account has been created. */
    client.connection.send_packet(&packets::C2SGarageBuyItem {
        var_204: 300, /* normally 500! */
        item: "pro_battle_m0".to_string(),
        count: 1,
    })?;
    tokio::select! {
        /* do not act too fast */
        _ = time::sleep(Duration::from_secs(1)) => {},
        _ = &mut client => {}
    }; 

    Ok(result)
}

trait UserNameGenerator {
    fn next_username(&mut self) -> Option<String>;
}

struct FileUserNameGenerator {
    user_names: Vec<String>,
    user_name_index: usize
}

impl FileUserNameGenerator {
    pub fn new(file: &mut File) -> anyhow::Result<Self> {
        let mut user_names = Vec::with_capacity(1024);
        let reader = BufReader::new(file);
        for username in reader.lines() {
            user_names.push(username?);
        }
        
        Ok(Self {
            user_names: user_names,
            user_name_index: 0
        })
    }
}

impl UserNameGenerator for FileUserNameGenerator {
    fn next_username(&mut self) -> Option<String> {
        if self.user_name_index >= self.user_names.len() {
            None
        } else {
            let index = self.user_name_index;
            self.user_name_index = index + 1;
            Some(self.user_names[index].clone())
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(Level::TRACE)
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let args: Args = Args::parse();

    let output: File = File::options()
        .append(true)
        .create(true)
        .open(format!("{}.generated.txt", args.username_file))?;
    let output = Arc::new(Mutex::new(output));

    let protanki_address: SocketAddr = args.target.parse()?;
    let proxy_provider: Box<dyn ProxyProvider> = if let Some(file) = &args.proxy_file {
        Box::new(SocksProxyProvider::from_file(
            &mut File::open(file)?
        )?)
    } else {
        Box::new(HostProxyProvider::new())
    };
    let proxy_provider = Arc::new(Mutex::new(proxy_provider));
    let username_provider = Arc::new(Mutex::new(FileUserNameGenerator::new(
        &mut File::open(&args.username_file)?
    )?));
  
    let local = task::LocalSet::new();
    for _ in 0..args.parallel_workers {
        let username_provider = username_provider.clone();
        let proxy_provider = proxy_provider.clone();
        let protanki_address = protanki_address.clone();
        let output = output.clone();
        let mut captcha_solver = CaptchaSolver2Captcha::new(args.captcha2_key.clone());
        let password = args.password.clone();
        local.spawn_local(async move {
            loop {
                let user_name = match username_provider.lock().unwrap().next_username() {
                    Some(proxy) => proxy,
                    None => {
                        info!("No more user names. Exit loop");
                        return;
                    }
                };

                let max_attempts = 3;
                for attempt in 0..max_attempts {
                    let mut proxy = match proxy_provider.lock().unwrap().next_proxy() {
                        Some(proxy) => proxy,
                        None => {
                            info!("No more proxies. Exit loop");
                            return;
                        }
                    };

                    match register_account_loop(
                        Box::as_mut(&mut proxy), 
                        protanki_address.clone(), 
                        user_name.clone(), 
                        password.clone(),
                        &mut captcha_solver
                    ).await {
                        Ok(result) => {
                            let mut output = output.lock().unwrap();
                            writeln!(output, "{} -> {:?}", user_name, result).unwrap();
                            info!("{} -> {:?}", user_name, result);
                            break;
                        },
                        Err(error) => {
                            warn!("User register error in attempt {}: {:?}", attempt, error);
                            if attempt + 1 >= max_attempts {
                                let mut output = output.lock().unwrap();
                                writeln!(output, "{} -> error {:?}", user_name, error).unwrap();
                            }
                        }
                    };
                }
            }
        });
    }
    local.await;


    Ok(())
}
