use anyhow::{anyhow, Result};
use axum::http::HeaderName;
use std::path::PathBuf;

/// How signed-in callers are identified. Share-token and email-intake routes
/// have their own auth and are unaffected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthMode {
    /// Verify the Cloudflare Access JWT (`Cf-Access-Jwt-Assertion`).
    CloudflareAccess,
    /// Trust an email header set by an authenticating reverse proxy
    /// (oauth2-proxy, Authelia, Tailscale Serve, ...). Only safe when the
    /// proxy is the sole way to reach the app.
    TrustedHeader(HeaderName),
    /// No auth: every caller is the primary owner. Local trial / dev only.
    None,
}

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
    /// Allow-list of owner emails, lowercased. Guaranteed non-empty.
    pub owner_emails: Vec<String>,
    pub auth_mode: AuthMode,
    pub cf_access_team_domain: Option<String>,
    pub cf_access_aud: Option<String>,

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
        let owner_emails = parse_owner_emails(std::env::var("OWNER_EMAIL").ok())?;
        let cf_access_team_domain = std::env::var("CF_ACCESS_TEAM_DOMAIN").ok();
        let cf_access_aud = std::env::var("CF_ACCESS_AUD").ok();
        let dev_bypass_auth = std::env::var("DEV_BYPASS_AUTH")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let auth_mode = parse_auth_mode(
            std::env::var("TRIPIT_AUTH_MODE").ok().as_deref(),
            std::env::var("TRIPIT_AUTH_HEADER").ok().as_deref(),
            dev_bypass_auth,
        )?;

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

        match &auth_mode {
            AuthMode::CloudflareAccess
                if cf_access_team_domain.is_none() || cf_access_aud.is_none() =>
            {
                tracing::warn!(
                    "CF_ACCESS_TEAM_DOMAIN and CF_ACCESS_AUD are unset; configure them, \
                     or pick another TRIPIT_AUTH_MODE (header | none)."
                );
            }
            AuthMode::CloudflareAccess => tracing::info!("auth mode = cloudflare access"),
            AuthMode::TrustedHeader(h) => tracing::warn!(
                "auth mode = trusted header {h:?} — make sure only your reverse proxy \
                 can reach this port, or anyone can claim any identity."
            ),
            AuthMode::None => tracing::warn!(
                "auth mode = NONE — every request acts as {}. Local use only.",
                owner_emails[0]
            ),
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
            owner_emails,
            auth_mode,
            cf_access_team_domain,
            cf_access_aud,
            parse_backend,
            anthropic_api_key,
            parse_model,
            daily_token_cap,
            ollama_url,
            ollama_model,
            email_hmac_key,
        })
    }

    /// True if `email` (any case) is in the configured owner allow-list.
    pub fn is_owner(&self, email: &str) -> bool {
        let e = email.to_ascii_lowercase();
        self.owner_emails.iter().any(|o| *o == e)
    }

    /// The first configured owner. Used as the injected identity when auth is
    /// bypassed for local development. Safe to index: `owner_emails` is
    /// guaranteed non-empty by [`parse_owner_emails`].
    pub fn primary_owner(&self) -> &str {
        &self.owner_emails[0]
    }
}

/// Resolve `TRIPIT_AUTH_MODE` (+ `TRIPIT_AUTH_HEADER`). Defaults to Cloudflare
/// Access so existing deploys keep their behavior. `DEV_BYPASS_AUTH=1` is the
/// legacy spelling of `none`.
fn parse_auth_mode(mode: Option<&str>, header: Option<&str>, dev_bypass: bool) -> Result<AuthMode> {
    if dev_bypass {
        return match mode {
            None | Some("none") => Ok(AuthMode::None),
            Some(other) => Err(anyhow!(
                "DEV_BYPASS_AUTH=1 conflicts with TRIPIT_AUTH_MODE={other:?}; unset one"
            )),
        };
    }
    match mode {
        None | Some("cloudflare") | Some("cf-access") => Ok(AuthMode::CloudflareAccess),
        Some("header") | Some("trusted-header") => {
            let name = header.unwrap_or("X-Forwarded-Email");
            let name = HeaderName::try_from(name)
                .map_err(|_| anyhow!("TRIPIT_AUTH_HEADER={name:?} is not a valid header name"))?;
            Ok(AuthMode::TrustedHeader(name))
        }
        Some("none") | Some("off") => Ok(AuthMode::None),
        Some(other) => Err(anyhow!(
            "TRIPIT_AUTH_MODE={other:?} unrecognized; use cloudflare|header|none"
        )),
    }
}

/// Parse the `OWNER_EMAIL` env var into a non-empty, lowercased allow-list.
/// Accepts a comma-separated list; fails closed if unset or empty so a
/// misconfigured deploy never silently falls back to a default owner.
fn parse_owner_emails(raw: Option<String>) -> Result<Vec<String>> {
    let raw = raw.ok_or_else(|| {
        anyhow!("OWNER_EMAIL must be set (comma-separated list of allowed owner emails)")
    })?;
    let emails: Vec<String> = raw
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect();
    if emails.is_empty() {
        return Err(anyhow!("OWNER_EMAIL is set but contains no valid addresses"));
    }
    Ok(emails)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_emails_required() {
        assert!(parse_owner_emails(None).is_err());
        assert!(parse_owner_emails(Some("".into())).is_err());
        assert!(parse_owner_emails(Some("  ,  ".into())).is_err());
    }

    #[test]
    fn owner_emails_parsed_trimmed_and_lowercased() {
        let e = parse_owner_emails(Some("  Owner@Example.COM ".into())).unwrap();
        assert_eq!(e, vec!["owner@example.com"]);
    }

    #[test]
    fn auth_mode_defaults_to_cloudflare() {
        assert_eq!(parse_auth_mode(None, None, false).unwrap(), AuthMode::CloudflareAccess);
    }

    #[test]
    fn auth_mode_header_default_and_custom() {
        assert_eq!(
            parse_auth_mode(Some("header"), None, false).unwrap(),
            AuthMode::TrustedHeader(HeaderName::from_static("x-forwarded-email"))
        );
        assert_eq!(
            parse_auth_mode(Some("header"), Some("Tailscale-User-Login"), false).unwrap(),
            AuthMode::TrustedHeader(HeaderName::from_static("tailscale-user-login"))
        );
        assert!(parse_auth_mode(Some("header"), Some("bad header"), false).is_err());
    }

    #[test]
    fn auth_mode_dev_bypass_is_none_and_conflicts_with_others() {
        assert_eq!(parse_auth_mode(None, None, true).unwrap(), AuthMode::None);
        assert!(parse_auth_mode(Some("header"), None, true).is_err());
        assert!(parse_auth_mode(Some("bogus"), None, false).is_err());
    }

    #[test]
    fn owner_emails_comma_separated_drops_blanks() {
        let e = parse_owner_emails(Some("a@x.com, ,B@X.com,".into())).unwrap();
        assert_eq!(e, vec!["a@x.com", "b@x.com"]);
    }
}
