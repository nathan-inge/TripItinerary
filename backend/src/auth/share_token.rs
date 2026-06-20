use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::RngCore;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

use crate::models::ShareLink;

/// 32 random bytes → URL-safe base64 with no padding (43 chars).
pub fn mint_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub fn hash_token(raw: &str) -> String {
    let mut h = Sha256::new();
    h.update(raw.as_bytes());
    let out = h.finalize();
    let mut hex = String::with_capacity(out.len() * 2);
    for b in out {
        use std::fmt::Write;
        let _ = write!(hex, "{:02x}", b);
    }
    hex
}

/// Look up an active share link by raw token. Returns None if missing or revoked.
/// Also stamps last_used_at on success.
pub async fn verify_token(pool: &SqlitePool, raw_token: &str) -> sqlx::Result<Option<ShareLink>> {
    let hash = hash_token(raw_token);
    let link: Option<ShareLink> = sqlx::query_as(
        "SELECT * FROM share_links WHERE token_hash = ?1 AND revoked_at IS NULL",
    )
    .bind(&hash)
    .fetch_optional(pool)
    .await?;

    if let Some(ref l) = link {
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query("UPDATE share_links SET last_used_at = ?1 WHERE id = ?2")
            .bind(&now)
            .bind(&l.id)
            .execute(pool)
            .await?;
    }
    Ok(link)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_43_chars() {
        let t = mint_token();
        assert_eq!(t.len(), 43);
        // URL-safe base64 chars only.
        assert!(t
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
    }

    #[test]
    fn hash_is_stable_and_64_chars() {
        let h1 = hash_token("hello");
        let h2 = hash_token("hello");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64);
    }
}
