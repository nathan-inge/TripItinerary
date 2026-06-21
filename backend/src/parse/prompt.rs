//! Prompt + schema shared by all parser backends. Identical instructions go
//! to Claude (via tool_use input_schema) and Ollama (via the structured-
//! outputs `format` field) so a deployment can flip between them without
//! retraining its expectations.

use anyhow::{anyhow, Result};
use serde_json::{json, Value};

use crate::models::{ItemSource, ItemType, Link, ParseStatus};

pub const SYSTEM_PROMPT: &str = r#"
You parse travel booking confirmations (forwarded emails or pasted text) into
structured itinerary items for a personal trip-itinerary app.

Rules:
- Output a single JSON object matching the schema (no commentary, no markdown).
- Pick `type` from: flight, lodging, transport, activity.
  - flight: airline ticket / boarding pass
  - lodging: hotel, Airbnb, vacation rental, hostel
  - transport: train, bus, intercity coach, car rental, prebooked transfer/limo
  - activity: museum tickets, tours, concerts, restaurant reservations, anything else
- Times must be ISO 8601 with the local timezone offset preserved
  (e.g. "2026-10-12T11:25:00-07:00"). Never normalize to UTC.
- For multi-leg flights, return ONLY the leg this confirmation is about (one item).
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
- The source may contain `[REDACTED:CARD]`, `[REDACTED:EMAIL]`, `[REDACTED:PHONE]`,
  `[REDACTED:SSN]`, `[REDACTED:NAME]` — treat these as intentionally stripped, do
  not invent replacements, and leave them in any field where they appear.
- Booking references appear as placeholders like `[REDACTED:CONF_1]`,
  `[REDACTED:CONF_2]`. When you emit `confirmation_no`, emit the placeholder verbatim
  (e.g. `"confirmation_no": "[REDACTED:CONF_1]"`); the server will substitute the
  real value back from a local map. If no `[REDACTED:CONF_*]` placeholder appears in
  the source for this item, set `confirmation_no` to null rather than guessing.
"#;

/// The JSON Schema both backends agree on. Returned as a serde_json::Value so
/// either side can wrap it appropriately (tool_use input_schema for Claude,
/// `format` field for Ollama structured outputs).
pub fn record_item_schema() -> Value {
    json!({
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
    })
}

#[derive(Debug, Clone)]
pub struct ParsedItem {
    pub item_type: ItemType,
    pub title: String,
    pub start_at: String,
    pub end_at: Option<String>,
    pub location: Option<String>,
    pub confirmation_no: Option<String>,
    pub notes: String,
    pub details: Value,
    pub links: Vec<Link>,
    pub parse_status: ParseStatus,
    pub source: ItemSource,
    pub review_reason: String,
}

/// Parse a JSON object matching `record_item_schema()` into a `ParsedItem`.
/// Used by both backends after they have a JSON object in hand.
pub fn parsed_item_from_value(value: &Value, source: ItemSource) -> Result<ParsedItem> {
    let obj = value
        .as_object()
        .ok_or_else(|| anyhow!("model output is not an object"))?;

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
    let end_at = obj.get("end_at").and_then(Value::as_str).map(str::to_string);
    let location = obj.get("location").and_then(Value::as_str).map(str::to_string);
    let confirmation_no = obj
        .get("confirmation_no")
        .and_then(Value::as_str)
        .map(str::to_string);
    let notes = obj.get("notes").and_then(Value::as_str).unwrap_or_default().to_string();
    let details = obj
        .get("details")
        .cloned()
        .unwrap_or_else(|| Value::Object(Default::default()));

    let mut links = Vec::new();
    if let Some(arr) = obj.get("links").and_then(Value::as_array) {
        for l in arr {
            let label = l.get("label").and_then(Value::as_str).unwrap_or_default().to_string();
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
        source,
        review_reason,
    })
}

/// Rough estimate used for the daily token cap reservation. Same regardless
/// of backend (cap is a budget; local LLM doesn't cost but it doesn't hurt
/// to track).
pub fn estimate_tokens(raw_text: &str) -> u64 {
    1500 + (raw_text.len() as u64 / 4) + 512
}
