use serde::Deserialize;
use std::env;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RedisConfig {
    pub url: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ZenohConfig {
    pub listen_endpoint: String,
    pub state_topic_pattern: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct PathsConfig {
    pub assets_dir: String,
    pub graph_file: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub redis: RedisConfig,
    pub zenoh: ZenohConfig,
    pub paths: PathsConfig,
}

impl AppConfig {
    pub fn load() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let config_path = env::var("CONFIG_FILE").unwrap_or_else(|_| "config/default.json".to_string());
        
        let mut config: AppConfig = if Path::new(&config_path).exists() {
            let content = fs::read_to_string(&config_path)?;
            serde_json::from_str(&content)?
        } else {
            // Fallback safe defaults if file is missing
            AppConfig {
                server: ServerConfig { host: "0.0.0.0".into(), port: 3000 },
                redis: RedisConfig { url: "redis://127.0.0.1:6379".into() },
                zenoh: ZenohConfig { listen_endpoint: "tcp/127.0.0.1:7447".into(), state_topic_pattern: "issem/v3/*/*/state".into() },
                paths: PathsConfig {
                    assets_dir: "static/assets".into(),
                    graph_file: "static/assets/graph.json".into(),
                },
            }
        };

        // Environment variable overrides
        if let Ok(redis_url) = env::var("REDIS_URL") {
            config.redis.url = redis_url;
        }
        if let Ok(port) = env::var("PORT") {
            if let Ok(p) = port.parse::<u16>() {
                config.server.port = p;
            }
        }

        Ok(config)
    }
}