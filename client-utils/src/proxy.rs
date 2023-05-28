use std::{net::SocketAddr, fs::File, str::FromStr, io::{BufReader, BufRead}, fmt::Debug};

use async_trait::async_trait;
use fast_socks5::client::{Config, Socks5Stream};
use rand::{thread_rng, RngCore};
use reqwest::Url;
use fost_protocol::Socket;
use tokio::{net::TcpStream};
use anyhow::Context;

#[async_trait]
pub trait Proxy : Send + Debug {
    async fn create_stream(&mut self, target: SocketAddr) -> anyhow::Result<Box<dyn Socket + Send>>;
}

pub trait ProxyProvider {
    fn next_proxy(&self) -> Option<Box<dyn Proxy>>;
}

struct HostProxy;

#[async_trait]
impl Proxy for HostProxy {
    async fn create_stream(&mut self, target: SocketAddr) -> anyhow::Result<Box<dyn Socket + Send>> {
        Ok(
            Box::new(
                TcpStream::connect(target).await?
            )
        )
    }
}

impl Debug for HostProxy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostProxy").finish()
    }
}

pub struct HostProxyProvider;

impl HostProxyProvider {
    pub fn new() -> Self {
        Self {}
    }
}

impl ProxyProvider for HostProxyProvider {
    fn next_proxy(&self) -> Option<Box<dyn Proxy>> {
        Some(Box::new(HostProxy{}))
    }
}

#[derive(Debug, Clone)]
struct SocksProxy {
    host: String,
    port: u16,

    credentials: Option<(String, String)>
}

impl SocksProxy {
    pub fn from_url(url: Url) -> anyhow::Result<Self> {
        Ok(SocksProxy {
            host: url.host().context("missing URL host")?.to_string(),
            port: url.port().unwrap_or(1080),
            credentials: if let Some(pass) = url.password() {
                Some((url.username().to_string(), pass.to_string()))
            } else {
                None
            }
        })
    }
}

#[async_trait]
impl Proxy for SocksProxy {
    async fn create_stream(&mut self, target: SocketAddr) -> anyhow::Result<Box<dyn Socket + Send>> {
        let socks_config = Config::default();
        let stream = if let Some((user, pass)) = &self.credentials {
            Socks5Stream::connect_with_password(
                format!("{}:{}", self.host, self.port), 
                target.ip().to_string(), 
                target.port(), 
                user.to_owned(), 
                pass.to_owned(), 
                socks_config
            ).await?
        } else {
            Socks5Stream::connect(
                format!("{}:{}", self.host, self.port), 
                target.ip().to_string(), 
                target.port(), 
                socks_config
            ).await?
        };

        Ok(Box::new(stream))
    }
}

pub struct SocksProxyProvider {
    proxies: Vec<SocksProxy>
}

impl SocksProxyProvider {
    pub fn from_file(file: &mut File) -> anyhow::Result<Self> {
        let mut proxies = Vec::with_capacity(1024);
        let reader = BufReader::new(file);
        for proxy in reader.lines() {
            let parsed: Url = Url::from_str(&proxy?)?;
            proxies.push(SocksProxy::from_url(parsed)?);
        }
        
        Ok(Self {
            proxies
        })
    }
}

impl ProxyProvider for SocksProxyProvider {
    fn next_proxy(&self) -> Option<Box<dyn Proxy>> {
        if self.proxies.len() < 1 {
            None
        } else {
            let proxy = self.proxies[thread_rng().next_u32() as usize % self.proxies.len()].clone();
            Some(Box::new(proxy))
        }
    }
}