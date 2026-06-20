use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ItemType {
    Flight,
    Lodging,
    Transport,
    Activity,
}

impl ItemType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ItemType::Flight => "flight",
            ItemType::Lodging => "lodging",
            ItemType::Transport => "transport",
            ItemType::Activity => "activity",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ItemSource {
    Paste,
    Email,
    #[default]
    Manual,
}

impl ItemSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            ItemSource::Paste => "paste",
            ItemSource::Email => "email",
            ItemSource::Manual => "manual",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ParseStatus {
    Parsed,
    Review,
    #[default]
    Manual,
}

impl ParseStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ParseStatus::Parsed => "parsed",
            ParseStatus::Review => "review",
            ParseStatus::Manual => "manual",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Link {
    pub label: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct Item {
    pub id: String,
    pub trip_id: String,
    #[serde(rename = "type")]
    pub item_type: ItemType,
    pub title: String,
    pub start_at: String,
    pub end_at: Option<String>,
    pub location: Option<String>,
    pub confirmation_no: Option<String>,
    pub notes: String,
    pub details: serde_json::Value,
    pub links: Vec<Link>,
    pub traveler_ids: Vec<String>,
    pub raw_source: Option<String>,
    pub source: ItemSource,
    pub parse_status: ParseStatus,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateItem {
    #[serde(rename = "type")]
    pub item_type: ItemType,
    pub title: String,
    pub start_at: String,
    pub end_at: Option<String>,
    pub location: Option<String>,
    pub confirmation_no: Option<String>,
    #[serde(default)]
    pub notes: String,
    #[serde(default = "serde_json::Value::default")]
    pub details: serde_json::Value,
    #[serde(default)]
    pub links: Vec<Link>,
    #[serde(default)]
    pub traveler_ids: Vec<String>,
    #[serde(default)]
    pub raw_source: Option<String>,
    #[serde(default)]
    pub source: ItemSource,
    #[serde(default)]
    pub parse_status: ParseStatus,
}

#[derive(Debug, Default, Deserialize, ToSchema)]
pub struct UpdateItem {
    #[serde(rename = "type")]
    pub item_type: Option<ItemType>,
    pub title: Option<String>,
    pub start_at: Option<String>,
    pub end_at: Option<Option<String>>,
    pub location: Option<Option<String>>,
    pub confirmation_no: Option<Option<String>>,
    pub notes: Option<String>,
    pub details: Option<serde_json::Value>,
    pub links: Option<Vec<Link>>,
    pub traveler_ids: Option<Vec<String>>,
    pub parse_status: Option<ParseStatus>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ParseTextRequest {
    pub raw_text: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ItemPreview {
    #[serde(rename = "type")]
    pub item_type: ItemType,
    pub title: String,
    pub start_at: String,
    pub end_at: Option<String>,
    pub location: Option<String>,
    pub confirmation_no: Option<String>,
    pub notes: String,
    pub details: serde_json::Value,
    pub links: Vec<Link>,
    pub raw_source: String,
    pub source: ItemSource,
    pub parse_status: ParseStatus,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ParseTextResponse {
    pub preview: ItemPreview,
    pub review_reason: String,
}
