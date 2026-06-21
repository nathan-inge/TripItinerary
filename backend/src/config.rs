use anyhow::{anyhow, Result};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseBackend {
    /// Send confirmation text (PII-redacted) to the Anthropic API.
    Anthropic,
    /// Talk to a local Ollama instance — text stays on the host.
    Ollama,
    /// No automated parsing; users edit items manually after pasting/forwarding.
    None,
    /// Skip the LLM call but log the raw input, the post-redaction body, and
    /// the conf-substitution map. Useful for verifying the redaction layer
    /// without spending tokens or risking a leak during testing.
    DryRun,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub bind_addr: String,
    pub owner_email: String,
    pub cf_access_team_domain: Option<String>,
    pub cf_access_aud: Option<String>,
    pub dev_bypass_auth: bool,

    pub parse_backend: ParseBackend,

    pub anthropic_api_key: Option<String>,
    pub parse_model: String,
    pub daily_token_cap: u64,

    pub ollama_url: String,
    pub ollama_model: String,

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

        let ollama_url = std::env::var("TRIPIT_OLLAMA_URL")
            .unwrap_or_else(|_| "http://localhost:11434".to_string());
        let ollama_model = std::env::var("TRIPIT_OLLAMA_MODEL")
            .unwrap_or_else(|_| "qwen2.5:7b-instruct".to_string());

        let email_hmac_key = std::env::var("TRIPIT_EMAIL_HMAC_KEY").ok();

        // Backend selection. Explicit env wins; otherwise infer from what's
        // configured. Preference order: ollama (local-first) if its URL is
        // explicitly set, anthropic if an API key is set, none otherwise.
        let parse_backend = match std::env::var("TRIPIT_PARSE_BACKEND").ok().as_deref() {
            Some("anthropic") | Some("claude") => ParseBackend::Anthropic,
            Some("ollama") | Some("local") => ParseBackend::Ollama,
            Some("none") | Some("off") => ParseBackend::None,
            Some("dry-run") | Some("dry_run") | Some("dryrun") => ParseBackend::DryRun,
            Some(other) => {
                return Err(anyhow!(
                    "TRIPIT_PARSE_BACKEND={other:?} unrecognized; \
                     use anthropic|ollama|none|dry-run"
                ));
            }
            None => {
                // Default: prefer local Ollama if user explicitly set its URL
                // override; otherwise fall back to Anthropic if a key is set;
                // otherwise none.
                if std::env::var("TRIPIT_OLLAMA_URL").is_ok() {
                    ParseBackend::Ollama
                } else if anthropic_api_key.is_some() {
                    ParseBackend::Anthropic
                } else {
                    ParseBackend::None
                }
            }
        };

        if !dev_bypass_auth && (cf_access_team_domain.is_none() || cf_access_aud.is_none()) {
            tracing::warn!(
                "CF_ACCESS_TEAM_DOMAIN and CF_ACCESS_AUD are unset; \
                 set DEV_BYPASS_AUTH=1 for local development, or configure them in production."
            );
        }
        match parse_backend {
            ParseBackend::Anthropic if anthropic_api_key.is_none() => {
                tracing::warn!(
                    "TRIPIT_PARSE_BACKEND=anthropic but ANTHROPIC_API_KEY is unset — parsing will fail."
                );
            }
            ParseBackend::Ollama => {
                tracing::info!(
                    "parse backend = ollama at {} (model: {})",
                    ollama_url,
                    ollama_model
                );
            }
            ParseBackend::Anthropic => {
                tracing::info!("parse backend = anthropic (model: {})", parse_model);
            }
            ParseBackend::None => {
                tracing::warn!(
                    "parse backend = none — paste/forward will not auto-extract fields."
                );
            }
            ParseBackend::DryRun => {
                tracing::warn!(
                    "parse backend = DRY RUN — redactor runs, LLM is NOT called, \
                     input + redacted body + conf map are logged. Do not use in prod."
                );
            }
        }

        Ok(Self {
            database_url,
            bind_addr,
            owner_email,
            cf_access_team_domain,
            cf_access_aud,
            dev_bypass_auth,
            parse_backend,
            anthropic_api_key,
            parse_model,
            daily_token_cap,
            ollama_url,
            ollama_model,
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
