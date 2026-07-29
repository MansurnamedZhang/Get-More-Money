use std::{env, net::SocketAddr};

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: SocketAddr,
    pub database_url: String,
    pub auth_required: bool,
    pub internal_api_token: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let bind_addr = env::var("APP_BIND")
            .unwrap_or_else(|_| "127.0.0.1:3001".to_owned())
            .parse::<SocketAddr>()
            .map_err(|error| format!("APP_BIND is invalid: {error}"))?;

        let database_url =
            env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite://data/investment.db".to_owned());
        let auth_required = env::var("AUTH_REQUIRED")
            .unwrap_or_else(|_| "true".to_owned())
            .parse::<bool>()
            .map_err(|error| format!("AUTH_REQUIRED is invalid: {error}"))?;
        let internal_api_token = env::var("INTERNAL_API_TOKEN")
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());
        if !auth_required && internal_api_token.is_none() {
            return Err(
                "INTERNAL_API_TOKEN is required when AUTH_REQUIRED=false; the core service must only accept gateway traffic"
                    .to_owned(),
            );
        }

        Ok(Self {
            bind_addr,
            database_url,
            auth_required,
            internal_api_token,
        })
    }
}
