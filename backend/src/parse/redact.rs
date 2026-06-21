//! Redaction of high-risk fields before LLM bodies leave the host.
//!
//! Always redacted (no substitute-back):
//!   1. Payment-card numbers (Luhn-validated to suppress false positives on
//!      flight numbers and PNRs).
//!   2. SSN-shaped strings (US format).
//!   3. Phone numbers (US/international).
//!   4. Naked email addresses that aren't part of a URL.
//!   5. Traveler names from the trip's allow-list (case-insensitive,
//!      word-boundary match).
//!
//! Redacted *with* substitute-back via a returned map:
//!   6. Booking confirmation numbers detected by an explicit label
//!      (Booking / Confirmation / Reservation / Reference / Record locator /
//!      PNR / PIN, with optional code|number suffix). Replaced with a stable
//!      placeholder `[REDACTED:CONF_n]`; the LLM emits the placeholder as
//!      `confirmation_no` and we map back the real value locally after the
//!      response arrives. The LLM never sees the real value.
//!
//! Deliberately NOT redacted (the parser needs them):
//!   - Trip name, item title, dates, locations, flight numbers, loyalty IDs
//!     (loyalty IDs aren't in our redact list because they're typically not
//!     enough on their own to manage a booking, unlike the booking ref).

use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Result of `redact_for_remote`.
pub struct RedactOutput {
    /// Body to send to the LLM. PII stripped; conf #s and names placeholdered.
    pub text: String,
    /// Map of placeholder → real conf #. Used to restore `confirmation_no`
    /// on the response side. Caller should drop this when done.
    pub conf_map: HashMap<String, String>,
}

/// Full pre-LLM redaction. Applies every pass in §1–6 of the module docs.
pub fn redact_for_remote(text: &str, traveler_names: &[&str]) -> RedactOutput {
    let mut out = text.to_string();
    out = redact_emails_outside_urls(&out);
    out = redact_card_numbers(&out);
    out = redact_ssns(&out);
    out = redact_phones(&out);
    out = redact_names(&out, traveler_names);
    let (out, conf_map) = redact_booking_refs(&out);
    RedactOutput {
        text: out,
        conf_map,
    }
}

/// Backwards-compat wrapper for callers that don't have a trip context (none
/// in production after this commit; tests may use it).
#[cfg(test)]
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

// ---------- traveler names ----------

fn redact_names(text: &str, names: &[&str]) -> String {
    // Build the redaction set: each traveler name AS-WHOLE plus each
    // whitespace-delimited word. A traveler stored as "Nathan Inge" will
    // redact "Nathan Inge", "Nathan", and "Inge" — last names are at least
    // as sensitive as first names so we can't only match the full string.
    //
    // Skip parts < 3 chars: "Mo", "Al", "Ng" — too prone to false positives
    // against substrings of unrelated content.
    let mut parts: Vec<String> = Vec::new();
    for name in names {
        let trimmed = name.trim();
        if trimmed.len() >= 3 {
            parts.push(trimmed.to_string());
        }
        for word in trimmed.split_whitespace() {
            if word.len() >= 3 {
                parts.push(word.to_string());
            }
        }
    }
    // Process longer first so "Nathan Inge" matches before its component
    // words. Dedup preserves order (longest first).
    parts.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    parts.dedup();

    let mut out = text.to_string();
    for part in &parts {
        let escaped = regex::escape(part);
        let pattern = format!(r"(?i)\b{}\b", escaped);
        if let Ok(re) = Regex::new(&pattern) {
            out = re.replace_all(&out, "[REDACTED:NAME]").into_owned();
        }
    }
    out
}

// ---------- booking confirmation numbers ----------

fn booking_ref_regex() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        // Label is case-insensitive via `(?i:...)`. Value charset deliberately
        // permissive — it allows mixed case, slashes, equals, plus, dot, and
        // hyphen so password-style PINs like "86/==7cK" are caught. The
        // `looks_like_conf` heuristic below filters out English-word false
        // positives. `[\s:#]*` matches across newlines so multi-line shapes
        // like
        //     Booking code
        //         X57BBZ
        // get caught.
        Regex::new(concat!(
            r"(?i:\b(?:",
            r"booking(?:\s+code|\s+number|\s+reference|\s+ref(?:\.|\b)|\s+no\.?)?",
            r"|confirmation(?:\s+code|\s+number|\s+no\.?)?",
            r"|reservation(?:\s+code|\s+number|\s+no\.?)?",
            r"|reference(?:\s+number|\s+no\.?)?",
            r"|record\s*locator",
            r"|locator",
            r"|pnr",
            r"|conf\.?",
            r"|pin",
            r")\b)",
            r"[\s:#]*",
            r"([A-Za-z0-9][A-Za-z0-9/=\-]{3,19})",
        ))
        .unwrap()
    })
}

/// True if `s` looks like a booking ref/PIN rather than an English word.
/// English words are pure ASCII letters with at least one lowercase letter
/// (e.g. "processed", "confirmed"). Anything else — uppercase-only codes
/// like "ABCDEF", digit-containing codes like "ABC123" or "6214", and
/// password-style strings like "86/==7cK" — is accepted.
fn looks_like_conf(s: &str) -> bool {
    let all_letters = s.chars().all(|c| c.is_ascii_alphabetic());
    let has_lowercase = s.chars().any(|c| c.is_ascii_lowercase());
    !(all_letters && has_lowercase)
}

fn redact_booking_refs(text: &str) -> (String, HashMap<String, String>) {
    let mut map: HashMap<String, String> = HashMap::new();
    let mut value_to_placeholder: HashMap<String, String> = HashMap::new();
    let mut counter = 0;

    for caps in booking_ref_regex().captures_iter(text) {
        let value = caps.get(1).unwrap().as_str();
        if !looks_like_conf(value) {
            continue;
        }
        if value_to_placeholder.contains_key(value) {
            continue;
        }
        counter += 1;
        let placeholder = format!("[REDACTED:CONF_{}]", counter);
        value_to_placeholder.insert(value.to_string(), placeholder.clone());
        map.insert(placeholder, value.to_string());
    }

    let mut redacted = text.to_string();
    // Apply globally so the same value at multiple sites (header, body,
    // URL params, footer) all become the same placeholder.
    for (value, placeholder) in &value_to_placeholder {
        redacted = redacted.replace(value, placeholder);
    }

    (redacted, map)
}

/// Look up a real conf value for a placeholder the LLM emitted.
/// Returns None if `value` isn't a known placeholder — caller decides whether
/// to keep the raw value (lenient) or drop it (strict).
pub fn lookup_conf_placeholder<'a>(
    value: &str,
    map: &'a HashMap<String, String>,
) -> Option<&'a String> {
    map.get(value)
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

    // ---------- booking refs ----------

    #[test]
    fn redacts_labeled_conf_inline() {
        let (out, map) = redact_booking_refs("Confirmation: 6698495724 thanks.");
        assert!(out.contains("[REDACTED:CONF_1]"));
        assert!(!out.contains("6698495724"));
        assert_eq!(map.get("[REDACTED:CONF_1]"), Some(&"6698495724".to_string()));
    }

    #[test]
    fn redacts_booking_no_colon() {
        let (out, _) = redact_booking_refs("Booking 9734737915\nThanks");
        assert!(out.contains("[REDACTED:CONF_1]"));
    }

    #[test]
    fn redacts_booking_code_multiline_value() {
        // Label on one line, value on the next non-empty line.
        let src = "Booking code    \n    \n      X57BBZ\nNext line";
        let (out, map) = redact_booking_refs(src);
        assert!(
            out.contains("[REDACTED:CONF_1]"),
            "expected placeholder, got: {out:?}"
        );
        assert!(!out.contains("X57BBZ"));
        assert_eq!(map.get("[REDACTED:CONF_1]"), Some(&"X57BBZ".to_string()));
    }

    #[test]
    fn redacts_pin() {
        let (out, _) = redact_booking_refs("PIN: 6214 (Confidential)");
        assert!(out.contains("[REDACTED:CONF_1]"));
        assert!(out.contains("(Confidential)")); // parenthetical preserved
    }

    #[test]
    fn redacts_confirmation_code() {
        let (out, _) = redact_booking_refs("Confirmation code: HMF2JANSPW\nDetails");
        assert!(out.contains("[REDACTED:CONF_1]"));
        assert!(!out.contains("HMF2JANSPW"));
    }

    #[test]
    fn global_substitute_when_value_appears_twice() {
        // Value appears once labeled and once unlabeled (footer / URL).
        let src = "Booking ABC123. Manage: https://x.com/?ref=ABC123";
        let (out, _) = redact_booking_refs(src);
        // Both occurrences should be the same placeholder.
        let count = out.matches("[REDACTED:CONF_1]").count();
        assert_eq!(count, 2, "got: {out:?}");
        assert!(!out.contains("ABC123"));
    }

    #[test]
    fn skips_english_word_after_label() {
        // "Booking processed by..." — model would happily extract "processed"
        // if we didn't gate on the looks_like_conf heuristic.
        let (out, map) = redact_booking_refs("Booking processed by Sixt staff.");
        assert!(map.is_empty(), "got map: {map:?}");
        assert_eq!(out, "Booking processed by Sixt staff.");
    }

    #[test]
    fn skips_short_lowercase_value() {
        let (_out, map) = redact_booking_refs("Confirmation: yes");
        assert!(map.is_empty());
    }

    #[test]
    fn allows_short_all_digit_pin() {
        let (out, _) = redact_booking_refs("PIN: 6214 (Confidential)");
        assert!(out.contains("[REDACTED:CONF_1]"));
    }

    #[test]
    fn distinct_placeholders_for_distinct_values() {
        let (out, map) = redact_booking_refs("Booking ABC123. Confirmation: 999888777.");
        assert!(out.contains("[REDACTED:CONF_1]"));
        assert!(out.contains("[REDACTED:CONF_2]"));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn redacts_password_style_pin() {
        // Real-world example: a mixed-case PIN with slashes and equals.
        let src = "Booking reference\nABC123\nPIN\n86/==7cK)\nThanks";
        let (out, map) = redact_booking_refs(src);
        assert!(!out.contains("86/==7cK"), "got: {out:?}");
        assert!(!out.contains("ABC123"));
        // Two distinct values redacted.
        assert_eq!(map.len(), 2, "map: {map:?}");
        // The `)` is NOT part of the PIN — it should still appear in output.
        assert!(out.contains(")"));
    }

    #[test]
    fn pin_multi_line_value() {
        let (out, _) = redact_booking_refs("PIN\n86/==7cK");
        assert!(out.contains("[REDACTED:CONF_1]"));
    }

    #[test]
    fn rejects_pure_lowercase_after_label() {
        // looks_like_conf safety net: "code" follows the label "Booking" but
        // is an English word.
        let (out, map) = redact_booking_refs("Booking code is provided below.");
        assert!(map.is_empty(), "got: {map:?}");
        assert_eq!(out, "Booking code is provided below.");
    }

    // ---------- names ----------

    #[test]
    fn redacts_traveler_name_word_boundary() {
        let out = redact_names("Dear Nathan, your booking is confirmed.", &["Nathan"]);
        assert!(out.contains("[REDACTED:NAME]"));
        assert!(!out.contains("Nathan"));
    }

    #[test]
    fn redacts_name_case_insensitive() {
        let out = redact_names("welcome NATHAN inge", &["Nathan Inge"]);
        // "NATHAN inge" as a whole-word match against "Nathan Inge"
        assert!(out.contains("[REDACTED:NAME]"));
    }

    #[test]
    fn redacts_each_word_of_multi_word_name() {
        // "To: Nathan Inge <addr>" should redact both names.
        let out = redact_names("To: Nathan Inge <addr>", &["Nathan Inge"]);
        assert!(!out.contains("Nathan"));
        assert!(!out.contains("Inge"));
        let count = out.matches("[REDACTED:NAME]").count();
        assert_eq!(count, 1, "full string matched first: {out:?}");
    }

    #[test]
    fn redacts_last_name_alone() {
        // First name doesn't appear; last name should still be redacted.
        let out = redact_names("Mr. Inge will arrive at 10am.", &["Nathan Inge"]);
        assert!(out.contains("[REDACTED:NAME]"));
        assert!(!out.contains("Inge"));
    }

    #[test]
    fn redacts_first_name_alone() {
        let out = redact_names("Hi Nathan, thanks!", &["Nathan Inge"]);
        assert!(out.contains("[REDACTED:NAME]"));
        assert!(!out.contains("Nathan"));
    }

    #[test]
    fn skips_substring_within_other_word() {
        // "Nat" shouldn't redact "Nathan" via partial match.
        let out = redact_names("Nathan booked it", &["Nat"]);
        // Length < 3 → skipped entirely.
        assert!(out.contains("Nathan"));
        // But if we redact on "Nate" (>=3 chars), should NOT touch "Nathan".
        let out2 = redact_names("Nathan booked it", &["Nate"]);
        assert!(out2.contains("Nathan"));
    }

    // ---------- end-to-end ----------

    #[test]
    fn redact_for_remote_combines_all() {
        let src = "Dear Nathan Inge,\n\nBooking 9734737915 confirmed. \
                   Charge to card 4111 1111 1111 1111. Reach us at help@sixt.com.";
        let r = redact_for_remote(src, &["Nathan Inge"]);
        assert!(r.text.contains("[REDACTED:NAME]"));
        assert!(r.text.contains("[REDACTED:CARD]"));
        assert!(r.text.contains("[REDACTED:EMAIL]"));
        assert!(r.text.contains("[REDACTED:CONF_1]"));
        assert!(!r.text.contains("Nathan"));
        assert!(!r.text.contains("9734737915"));
        assert!(!r.text.contains("4111"));
        assert!(!r.text.contains("help@sixt.com"));
        assert_eq!(r.conf_map.get("[REDACTED:CONF_1]"), Some(&"9734737915".to_string()));
    }

    #[test]
    fn placeholder_lookup() {
        let r = redact_for_remote("Booking ABC123", &[]);
        assert_eq!(
            lookup_conf_placeholder("[REDACTED:CONF_1]", &r.conf_map),
            Some(&"ABC123".to_string())
        );
        assert!(lookup_conf_placeholder("[REDACTED:CONF_999]", &r.conf_map).is_none());
    }
}
