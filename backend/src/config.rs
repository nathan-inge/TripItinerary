use anyhow::Result;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: String,
    pub owner_email: String,
    pub cf_access_team_domain: Option<String>,
    pub cf_access_aud: Option<String>,
    pub dev_bypass_auth: bool,
    pub anthropic_api_key: Option<String>,
    pub parse_model: String,
    pub daily_token_cap: u64,
    pub email_hmac_key: Option<String>,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let database_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| default_db_url());
        let bind_addr =
            std::env::var("BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:5050".to_string());
        let owner_email =
            std::env::var("OWNER_EMAIL").unwrap_or_else(|_| "nate8983@gmail.com".to_string());
        let cf_access_team_domain = std::env::var("CF_ACCESS_TEAM_DOMAIN").ok();
        let cf_access_aud = std::env::var("CF_ACCESS_AUD").ok();
        let dev_bypass_auth = std::env::var("DEV_BYPASS_AUTH")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let anthropic_api_key = std::env::var("ANTHROPIC_API_KEY").ok();
        let parse_model = std::env::var("TRIPIT_PARSE_MODEL")
            .unwrap_or_else(|_| "claude-sonnet-4-6".to_string());
        let daily_token_cap = std::env::var("TRIPIT_DAILY_TOKEN_CAP")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(200_000);
        let email_hmac_key = std::env::var("TRIPIT_EMAIL_HMAC_KEY").ok();

        if !dev_bypass_auth && (cf_access_team_domain.is_none() || cf_access_aud.is_none()) {
            tracing::warn!(
                "CF_ACCESS_TEAM_DOMAIN and CF_ACCESS_AUD are unset; \
                 set DEV_BYPASS_AUTH=1 for local development, or configure them in production."
            );
        }
        if anthropic_api_key.is_none() {
            tracing::warn!(
                "ANTHROPIC_API_KEY is unset — paste-text parsing will fail until it is set."
            );
        }

        Ok(Self {
            database_url,
            bind_addr,
            owner_email,
            cf_access_team_domain,
            cf_access_aud,
            dev_bypass_auth,
            anthropic_api_key,
            parse_model,
            daily_token_cap,
            email_hmac_key,
        })
    }
}

fn default_db_url() -> String {
    let dir = default_data_dir();
    let _ = std::fs::create_dir_all(&dir);
    format!("sqlite://{}?mode=rwc", dir.join("db.sqlite").display())
}

fn default_data_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join(".tripit"))
        .unwrap_or_else(|| PathBuf::from(".tripit"))
}
