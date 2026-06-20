use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde_json::{json, Value};

use crate::models::ItemSource;
use crate::parse::prompt::{
    parsed_item_from_value, record_item_schema, ParsedItem, SYSTEM_PROMPT,
};
use crate::parse::redact::redact;

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

#[derive(Clone)]
pub struct ClaudeClient {
    http: Client,
    api_key: String,
    model: String,
}

impl ClaudeClient {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            http: Client::new(),
            api_key,
            model,
        }
    }

    pub async fn parse_item(&self, raw_text: &str) -> Result<(ParsedItem, u64)> {
        // Strip PII before crossing the network boundary. The local raw_source
        // stored in the DB is unaffected.
        let redacted = redact(raw_text);

        let tool = json!({
            "name": "record_item",
            "description": "Record a single structured itinerary item extracted from a confirmation.",
            "input_schema": record_item_schema(),
        });

        let body = json!({
            "model": self.model,
            "max_tokens": 1024,
            "system": [{
                "type": "text",
                "text": SYSTEM_PROMPT,
                "cache_control": { "type": "ephemeral" }
            }],
            "tools": [tool],
            "tool_choice": { "type": "tool", "name": "record_item" },
            "messages": [{ "role": "user", "content": redacted }],
        });

        let resp = self
            .http
            .post(API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow!("anthropic API {}: {}", status, text));
        }

        let value: Value = resp.json().await?;
        let usage = value
            .get("usage")
            .and_then(|u| {
                let input = u.get("input_tokens").and_then(Value::as_u64).unwrap_or(0);
                let output = u.get("output_tokens").and_then(Value::as_u64).unwrap_or(0);
                let cache_create = u
                    .get("cache_creation_input_tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                let cache_read = u
                    .get("cache_read_input_tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                Some(input + output + cache_create + cache_read)
            })
            .unwrap_or(0);

        let content = value
            .get("content")
            .and_then(Value::as_array)
            .ok_or_else(|| anyhow!("missing content array"))?;
        let tool_input = content
            .iter()
            .find(|b| b.get("type").and_then(Value::as_str) == Some("tool_use"))
            .and_then(|b| b.get("input"))
            .ok_or_else(|| anyhow!("model did not call record_item"))?;

        let parsed = parsed_item_from_value(tool_input, ItemSource::Paste)
            .context("parsing record_item input")?;
        Ok((parsed, usage))
    }
}
