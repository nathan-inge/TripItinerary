//! Logging helper for `TRIPIT_PARSE_BACKEND=dry-run`. Renders a multi-section
//! INFO log so operators can verify what the redactor does without spending
//! tokens (or letting any text out of the host).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use regex::Regex;

use crate::parse::redact::RedactOutput;

pub fn log_dry_run(label: &str, raw_text: &str, redacted: &RedactOutput) {
    let counts = count_placeholders(&redacted.text);

    // Build one multi-line string. journalctl folds it into a single entry
    // which is what we want — easier to grep, easier to diff.
    let mut out = String::with_capacity(raw_text.len() + redacted.text.len() + 1024);
    out.push_str("\n=== DRY RUN [");
    out.push_str(label);
    out.push_str("] ===\n");

    out.push_str("--- INPUT (");
    out.push_str(&raw_text.len().to_string());
    out.push_str(" bytes) ---\n");
    out.push_str(raw_text);
    if !raw_text.ends_with('\n') {
        out.push('\n');
    }

    out.push_str("--- REDACTED (");
    out.push_str(&redacted.text.len().to_string());
    out.push_str(" bytes — would be sent to LLM) ---\n");
    out.push_str(&redacted.text);
    if !redacted.text.ends_with('\n') {
        out.push('\n');
    }

    out.push_str("--- PLACEHOLDER COUNTS ---\n");
    if counts.is_empty() {
        out.push_str("(none)\n");
    } else {
        for (kind, n) in &counts {
            out.push_str(&format!("  {kind}: {n}\n"));
        }
    }

    out.push_str("--- CONF SUBSTITUTIONS (real values shown locally) ---\n");
    if redacted.conf_map.is_empty() {
        out.push_str("(none)\n");
    } else {
        // Sorted by placeholder so the log is stable across runs.
        let mut entries: Vec<_> = redacted.conf_map.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        for (placeholder, real) in entries {
            out.push_str(&format!("  {placeholder} → {real}\n"));
        }
    }
    out.push_str("=== END DRY RUN ===");

    tracing::info!("{}", out);
}

fn count_placeholders(text: &str) -> BTreeMap<&'static str, usize> {
    let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for kind in ["NAME", "CARD", "EMAIL", "PHONE", "SSN"] {
        let token = format!("[REDACTED:{kind}]");
        let n = text.matches(&token).count();
        if n > 0 {
            counts.insert(kind, n);
        }
    }
    let conf_n = conf_regex().find_iter(text).count();
    if conf_n > 0 {
        counts.insert("CONF", conf_n);
    }
    counts
}

fn conf_regex() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\[REDACTED:CONF_\d+\]").unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::redact::redact_for_remote;

    #[test]
    fn counts_distinct_categories() {
        let r = redact_for_remote(
            "Dear Nate, Booking ABC123. Phone (555) 123-4567. \
             help@x.com. Charge to 4111 1111 1111 1111.",
            &["Nate"],
        );
        let c = count_placeholders(&r.text);
        assert_eq!(c.get("NAME"), Some(&1));
        assert_eq!(c.get("CARD"), Some(&1));
        assert_eq!(c.get("EMAIL"), Some(&1));
        assert_eq!(c.get("PHONE"), Some(&1));
        assert_eq!(c.get("CONF"), Some(&1));
        assert!(c.get("SSN").is_none());
    }
}
