//! Backend-agnostic parser dispatch.

use anyhow::Result;

use crate::parse::claude::ClaudeClient;
use crate::parse::ollama::OllamaClient;
use crate::parse::prompt::ParsedItem;

#[derive(Clone)]
pub enum Parser {
    Claude(ClaudeClient),
    Ollama(OllamaClient),
}

impl Parser {
    pub async fn parse_item(&self, raw_text: &str) -> Result<(ParsedItem, u64)> {
        match self {
            Parser::Claude(c) => c.parse_item(raw_text).await,
            Parser::Ollama(o) => o.parse_item(raw_text).await,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Parser::Claude(_) => "anthropic",
            Parser::Ollama(_) => "ollama",
        }
    }
}
