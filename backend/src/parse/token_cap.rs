use std::sync::Arc;

use chrono::{NaiveDate, Utc};
use tokio::sync::Mutex;

/// Fail-closed daily token budget. Resets at UTC midnight.
#[derive(Clone, Default)]
pub struct TokenCap {
    inner: Arc<Mutex<Inner>>,
}

#[derive(Default)]
struct Inner {
    day: Option<NaiveDate>,
    used: u64,
}

impl TokenCap {
    pub async fn check_and_reserve(&self, est_tokens: u64, cap: u64) -> Result<(), String> {
        let today = Utc::now().date_naive();
        let mut g = self.inner.lock().await;
        if g.day != Some(today) {
            g.day = Some(today);
            g.used = 0;
        }
        if g.used.saturating_add(est_tokens) > cap {
            return Err(format!(
                "daily token cap reached ({} used + {} reserved > {})",
                g.used, est_tokens, cap
            ));
        }
        g.used = g.used.saturating_add(est_tokens);
        Ok(())
    }

    /// Reconcile reservation against actual usage reported by the API.
    pub async fn settle(&self, reserved: u64, actual: u64) {
        let mut g = self.inner.lock().await;
        // Add the delta (positive or negative) without going below zero.
        if actual >= reserved {
            g.used = g.used.saturating_add(actual - reserved);
        } else {
            g.used = g.used.saturating_sub(reserved - actual);
        }
    }
}
