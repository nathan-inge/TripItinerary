//! Local-LLM backend that keeps confirmation text on the host.
//!
//! Talks to an Ollama instance (default `http://localhost:11434`) and uses
//! Ollama's structured-outputs `format` field with our shared JSON schema so
//! the response is reliably parseable without a tool-call protocol.

use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde_json::{json, Value};

use crate::models::ItemSource;
use crate::parse::prompt::{
    parsed_item_from_value, record_item_schema, ParsedItem, SYSTEM_PROMPT,
};
use crate::parse::redact::redact;

#[derive(Clone)]
pub struct OllamaClient {
    http: Client,
    base_url: String,
    model: String,
}

impl OllamaClient {
    pub fn new(base_url: String, model: String) -> Self {
        // Tolerate trailing slash from configs like "http://localhost:11434/".
        let base = base_url.trim_end_matches('/').to_string();
        Self {
            http: Client::new(),
            base_url: base,
            model,
        }
    }

    pub async fn parse_item(&self, raw_text: &str) -> Result<(ParsedItem, u64)> {
        // Redact even though the request stays on-host — keeps `details_json`
        // and any downstream backups from carrying card #s and the like.
        let redacted = redact(raw_text);

        let body = json!({
            "model": self.model,
            "stream": false,
            "format": record_item_schema(),
            "messages": [
                { "role": "system", "content": SYSTEM_PROMPT },
                { "role": "user",   "content": redacted },
            ],
            // Lower temperature for extraction; the schema constrains shape
            // but the model still picks word forms.
            "options": { "temperature": 0 },
        });

        let url = format!("{}/api/chat", self.base_url);
        let resp = self
            .http
            .post(&url)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .with_context(|| format!("POST {url}"))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("ollama {}: {}", status, text));
        }

        let value: Value = resp.json().await?;
        let prompt_tokens = value
            .get("prompt_eval_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let output_tokens = value.get("eval_count").and_then(Value::as_u64).unwrap_or(0);

        let content_str = value
            .pointer("/message/content")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("ollama: missing message.content"))?;
        let item_json: Value = serde_json::from_str(content_str)
            .with_context(|| format!("ollama returned non-JSON content: {content_str:?}"))?;

        let parsed = parsed_item_from_value(&item_json, ItemSource::Paste)
            .context("ollama: schema-shaped JSON didn't deserialize")?;
        Ok((parsed, prompt_tokens + output_tokens))
    }
}
