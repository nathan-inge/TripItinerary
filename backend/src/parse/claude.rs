use anyhow::{anyhow, Context, Result};
use reqwest::Client;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::models::{ItemSource, ItemType, Link, ParseStatus};

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

const SYSTEM_PROMPT: &str = r#"
You parse travel booking confirmations (forwarded emails or pasted text) into
structured itinerary items for a personal trip-itinerary app.

Rules:
- Always call the `record_item` tool exactly once.
- Pick `type` from: flight, lodging, transport, activity.
  - flight: airline ticket / boarding pass
  - lodging: hotel, Airbnb, vacation rental, hostel
  - transport: train, bus, intercity coach, car rental, prebooked transfer/limo
  - activity: museum tickets, tours, concerts, restaurant reservations, anything else
- Times must be ISO 8601 with the local timezone offset preserved
  (e.g. "2026-10-12T11:25:00-07:00"). Never normalize to UTC.
- For multi-leg flights, return ONLY the leg this confirmation is about (one item per call).
  If a single confirmation contains multiple legs, pick the first leg and put a note
  in `notes_for_review` so a human can split them.
- Extract URLs verbatim from the source. NEVER fabricate or guess URLs (no
  "https://airline.com/manage/<conf>"). If a manage-booking URL is in the source,
  label it "Manage booking". If a map link is in the source, label it "Open in Maps".
  If no URLs exist, return an empty links array.
- `confirmation_no` should be the carrier/property's booking reference, record locator,
  or reservation code — not the email-internal ID.
- Put carrier/property-specific details (airline code, flight number, room type, address,
  party size, etc) into `details` as a JSON object. Use snake_case keys.
- If the source is ambiguous, incomplete, or you're guessing key fields, add a short
  explanation to `notes_for_review` so a human can fix it before saving.
- Keep `notes` for content meant for the traveler (gate, terminal, dress code, etc),
  not for your own caveats.
"#;

fn tool_schema() -> Value {
    json!({
        "name": "record_item",
        "description": "Record a single structured itinerary item extracted from a confirmation.",
        "input_schema": {
            "type": "object",
            "required": ["type", "title", "start_at"],
            "properties": {
                "type": {
                    "type": "string",
                    "enum": ["flight", "lodging", "transport", "activity"],
                    "description": "Item category."
                },
                "title": {
                    "type": "string",
                    "description": "Short human title, e.g. 'JL043 SFO → HND', 'Park Hyatt Tokyo'."
                },
                "start_at": {
                    "type": "string",
                    "description": "ISO 8601 with offset. For flights: scheduled departure local time. For lodging: check-in local time."
                },
                "end_at": {
                    "type": ["string", "null"],
                    "description": "ISO 8601 with offset, or null. For flights: scheduled arrival local time. For lodging: check-out local time."
                },
                "location": {
                    "type": ["string", "null"],
                    "description": "Human location string (airport+terminal, property address, venue name)."
                },
                "confirmation_no": {
                    "type": ["string", "null"],
                    "description": "Booking reference / record locator from the source."
                },
                "notes": {
                    "type": "string",
                    "description": "Traveler-facing notes (gate, dress code, etc). Empty if nothing useful."
                },
                "details": {
                    "type": "object",
                    "description": "Type-specific structured fields. Free-form JSON. Use snake_case keys."
                },
                "links": {
                    "type": "array",
                    "description": "URLs that appear verbatim in the source, each with a short purpose label.",
                    "items": {
                        "type": "object",
                        "required": ["label", "url"],
                        "properties": {
                            "label": { "type": "string" },
                            "url":   { "type": "string" }
                        }
                    }
                },
                "notes_for_review": {
                    "type": "string",
                    "description": "Caveats for the human reviewer. If non-empty, the item is flagged for review."
                }
            }
        }
    })
}

#[derive(Debug, Clone, Deserialize)]
pub struct ParsedItem {
    pub item_type: ItemType,
    pub title: String,
    pub start_at: String,
    pub end_at: Option<String>,
    pub location: Option<String>,
    pub confirmation_no: Option<String>,
    pub notes: String,
    pub details: serde_json::Value,
    pub links: Vec<Link>,
    pub parse_status: ParseStatus,
    pub source: ItemSource,
    pub review_reason: String,
}

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
        let body = json!({
            "model": self.model,
            "max_tokens": 1024,
            "system": [{
                "type": "text",
                "text": SYSTEM_PROMPT,
                "cache_control": { "type": "ephemeral" }
            }],
            "tools": [tool_schema()],
            "tool_choice": { "type": "tool", "name": "record_item" },
            "messages": [{ "role": "user", "content": raw_text }],
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

        let parsed = parse_tool_input(tool_input).context("parsing record_item input")?;
        Ok((parsed, usage))
    }
}

fn parse_tool_input(value: &Value) -> Result<ParsedItem> {
    let obj = value
        .as_object()
        .ok_or_else(|| anyhow!("tool input not an object"))?;

    let type_str = obj
        .get("type")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing type"))?;
    let item_type = match type_str {
        "flight" => ItemType::Flight,
        "lodging" => ItemType::Lodging,
        "transport" => ItemType::Transport,
        "activity" => ItemType::Activity,
        other => return Err(anyhow!("unsupported type from model: {}", other)),
    };

    let title = obj
        .get("title")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing title"))?
        .to_string();
    let start_at = obj
        .get("start_at")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("missing start_at"))?
        .to_string();
    let end_at = obj
        .get("end_at")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let location = obj
        .get("location")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let confirmation_no = obj
        .get("confirmation_no")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let notes = obj
        .get("notes")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let details = obj
        .get("details")
        .cloned()
        .unwrap_or_else(|| Value::Object(Default::default()));

    let mut links = Vec::new();
    if let Some(arr) = obj.get("links").and_then(Value::as_array) {
        for l in arr {
            let label = l
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let url = match l.get("url").and_then(Value::as_str) {
                Some(u) if !u.is_empty() => u.to_string(),
                _ => continue,
            };
            links.push(Link { label, url });
        }
    }

    let review_reason = obj
        .get("notes_for_review")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    let parse_status = if review_reason.is_empty() {
        ParseStatus::Parsed
    } else {
        ParseStatus::Review
    };

    Ok(ParsedItem {
        item_type,
        title,
        start_at,
        end_at,
        location,
        confirmation_no,
        notes,
        details,
        links,
        parse_status,
        source: ItemSource::Paste,
        review_reason,
    })
}

/// Rough estimate used for the daily token cap reservation.
pub fn estimate_tokens(raw_text: &str) -> u64 {
    // System prompt + tool schema + a fudge factor + raw_text/4 chars.
    1500 + (raw_text.len() as u64 / 4) + 512
}
