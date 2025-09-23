use std::net::SocketAddr;

pub struct Config {
	pub bind: SocketAddr,
	pub data_dir: String,
}

impl Config {
	pub fn load() -> anyhow::Result<Self> {
		let _ = dotenvy::dotenv();
		let port = std::env::var("PORT").ok().and_then(|p| p.parse::<u16>().ok());
		let bind = if let Some(p) = port { format!("127.0.0.1:{}", p) } else { std::env::var("HTTP_BIND").unwrap_or_else(|_| "127.0.0.1:8080".to_string()) };
		let bind: SocketAddr = bind.parse()?;
		let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string());
		Ok(Self { bind, data_dir })
	}
}
