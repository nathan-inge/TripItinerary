//! PII redaction for outbound LLM bodies.
//!
//! The goal is "what leaves the server has the high-risk PII removed" — not
//! "what's stored locally is anonymized". Run this only on the text that's
//! about to be sent to an external service (Anthropic API). Local raw_source
//! storage keeps the original.
//!
//! Targets, in confidence order:
//!   1. Payment-card numbers (Luhn-validated to suppress false positives on
//!      flight numbers and PNRs).
//!   2. SSN-shaped strings (US format only — rare in confirmations but worth
//!      catching).
//!   3. Phone numbers (US/international).
//!   4. Naked email addresses that aren't part of a URL (URLs may carry the
//!      manage-booking link; the parser uses them).
//!
//! Deliberately NOT redacted (the parser needs them):
//!   - Traveler names, addresses, dates, flight numbers, conf #s, loyalty IDs.

use regex::Regex;
use std::sync::OnceLock;

/// Redact PII from `text`. Idempotent: redaction tokens contain `[REDACTED:`
/// which doesn't match any pattern, so running twice is a no-op.
pub fn redact(text: &str) -> String {
    let mut out = text.to_string();
    out = redact_emails_outside_urls(&out);
    out = redact_card_numbers(&out);
    out = redact_ssns(&out);
    out = redact_phones(&out);
    out
}

// ---------- card numbers ----------

fn card_regex() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        // 13–19 digit runs, optionally split into groups by single spaces or
        // hyphens. Three shapes cover Visa/MC/Discover (4-4-4-4), Amex (4-6-5),
        // and contiguous. Non-verbose to avoid any surprise about literal
        // spaces inside character classes.
        Regex::new(
            r"\b\d{4}[ -]\d{4}[ -]\d{4}[ -]\d{1,7}\b|\b\d{4}[ -]\d{6}[ -]\d{4,5}\b|\b\d{13,19}\b",
        )
        .unwrap()
    })
}

fn redact_card_numbers(text: &str) -> String {
    let r = card_regex();
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for m in r.find_iter(text) {
        let candidate = m.as_str();
        let digits: String = candidate.chars().filter(|c| c.is_ascii_digit()).collect();
        if (13..=19).contains(&digits.len()) && luhn_valid(&digits) {
            out.push_str(&text[last..m.start()]);
            out.push_str("[REDACTED:CARD]");
            last = m.end();
        }
    }
    out.push_str(&text[last..]);
    out
}

fn luhn_valid(digits: &str) -> bool {
    let mut sum = 0u32;
    let mut alt = false;
    for c in digits.chars().rev() {
        let d = match c.to_digit(10) {
            Some(d) => d,
            None => return false,
        };
        let v = if alt {
            let v = d * 2;
            if v > 9 {
                v - 9
            } else {
                v
            }
        } else {
            d
        };
        sum += v;
        alt = !alt;
    }
    sum % 10 == 0
}

// ---------- SSNs ----------

fn redact_ssns(text: &str) -> String {
    static R: OnceLock<Regex> = OnceLock::new();
    let r = R.get_or_init(|| Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").unwrap());
    r.replace_all(text, "[REDACTED:SSN]").into_owned()
}

// ---------- phone numbers ----------

fn redact_phones(text: &str) -> String {
    static R: OnceLock<Regex> = OnceLock::new();
    // Common US/international shapes. Requires at least one separator so that
    // bare 10-digit strings (e.g. order IDs) aren't caught.
    let r = R.get_or_init(|| {
        Regex::new(
            r"(?x)
            (?:\+?\d{1,3}[\s.\-])?            # optional country code
            \(?\d{3}\)?[\s.\-]                # area code (parens optional)
            \d{3}[\s.\-]\d{4}                 # 3-4 split
            \b
            ",
        )
        .unwrap()
    });
    r.replace_all(text, "[REDACTED:PHONE]").into_owned()
}

// ---------- naked emails ----------

fn url_regex() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r#"https?://[^\s<>"')]+|mailto:[^\s<>"')]+"#).unwrap()
    })
}

fn email_regex() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r"[A-Za-z0-9._%+\-]+@[A-Za-z0-9.\-]+\.[A-Za-z]{2,}").unwrap()
    })
}

fn redact_emails_outside_urls(text: &str) -> String {
    // First, find URL byte spans we don't want to touch.
    let url_spans: Vec<(usize, usize)> =
        url_regex().find_iter(text).map(|m| (m.start(), m.end())).collect();
    let in_url = |pos: usize| url_spans.iter().any(|(s, e)| pos >= *s && pos < *e);

    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for m in email_regex().find_iter(text) {
        if in_url(m.start()) {
            continue;
        }
        out.push_str(&text[last..m.start()]);
        out.push_str("[REDACTED:EMAIL]");
        last = m.end();
    }
    out.push_str(&text[last..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_visa_card() {
        // 4111 1111 1111 1111 is the canonical Visa test card; Luhn-valid.
        let r = redact("Charged to 4111 1111 1111 1111 today.");
        assert!(r.contains("[REDACTED:CARD]"));
        assert!(!r.contains("4111"));
    }

    #[test]
    fn redacts_amex() {
        // 3782 822463 10005 is Amex test, Luhn-valid.
        let r = redact("Amex 3782 822463 10005 on file.");
        assert!(r.contains("[REDACTED:CARD]"));
    }

    #[test]
    fn skips_non_luhn_long_digits() {
        // 16 digits but not a valid card. PNRs, order IDs, etc.
        let r = redact("Order 1234567890123456 ready.");
        assert_eq!(r, "Order 1234567890123456 ready.");
    }

    #[test]
    fn skips_flight_numbers_and_dates() {
        let r = redact("Flight JL043 on 2026-10-13 dep 11:25.");
        assert_eq!(r, "Flight JL043 on 2026-10-13 dep 11:25.");
    }

    #[test]
    fn skips_pnr() {
        let r = redact("PNR: ABC123 confirmation locator XYZ987.");
        assert_eq!(
            r,
            "PNR: ABC123 confirmation locator XYZ987."
        );
    }

    #[test]
    fn redacts_ssn() {
        let r = redact("ID 123-45-6789.");
        assert!(r.contains("[REDACTED:SSN]"));
    }

    #[test]
    fn redacts_phone_us() {
        let r = redact("Call (415) 555-1234 anytime.");
        assert!(r.contains("[REDACTED:PHONE]"));
        let r2 = redact("Tel: +1 415 555 1234");
        assert!(r2.contains("[REDACTED:PHONE]"));
    }

    #[test]
    fn redacts_naked_email() {
        let r = redact("Reply to nate@example.com soon.");
        assert!(r.contains("[REDACTED:EMAIL]"));
        assert!(!r.contains("nate@example.com"));
    }

    #[test]
    fn preserves_email_inside_url() {
        let r = redact("Manage: https://airline.com/r?email=nate%40x.com&pnr=ABC");
        assert!(r.contains("airline.com"));
        // The URL is left untouched; the literal email isn't redacted.
        assert!(!r.contains("[REDACTED:EMAIL]"));
    }

    #[test]
    fn preserves_locations_and_names() {
        let input = "Nathan Inge checking in at 1-1 Nishishinjuku, Shinjuku-ku, Tokyo.";
        let r = redact(input);
        assert_eq!(r, input);
    }

    #[test]
    fn idempotent() {
        let once = redact("Card 4111 1111 1111 1111, phone (415) 555-1234.");
        let twice = redact(&once);
        assert_eq!(once, twice);
    }
}
