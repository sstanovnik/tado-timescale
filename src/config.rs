//! Minimal runtime configuration helpers.
//! Defaults align with docker-compose (localhost TimescaleDB).

pub const DEFAULT_DATABASE_URL: &str = "postgres://postgres:postgres@localhost:5432/tado";

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
}

impl Config {
    pub fn from_env() -> Self {
        let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string());
        Config { database_url }
    }
}

