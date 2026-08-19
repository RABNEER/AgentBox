use once_cell::sync::Lazy;
use regex::Regex;
use scraper::{Html, Selector};

// High-precision Regex patterns for OTP codes
static OTP_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        // "code is 123456", "verification code: 123456", "OTP: 123456", "Code: 123-456"
        Regex::new(r"(?i)(?:verification\s*code|security\s*code|login\s*code|otp|passcode|pin|confirmation\s*code)[\s:=—–-]+([0-9]{4,8}|[0-9]{3}-[0-9]{3})").unwrap(),
        // Standalone 6-digit or 4-8 digit numbers in strong context or subject
        Regex::new(r"(?i)\b(?:is|enter|use)\s+([0-9]{4,8})\b").unwrap(),
        // Generic 6-digit block surrounded by whitespace/delimiters
        Regex::new(r"\b([0-9]{6})\b").unwrap(),
    ]
});

// Regex patterns to identify verification / confirmation / magic links
static LINK_KEYWORD_PATTERNS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)(verify|confirm|activate|magic[-_]?link|login|session|auth|token|register)").unwrap()
});

// Extracted email credentials & metadata
#[derive(Debug, Clone, Default)]
pub struct ExtractedMetadata {
    pub otp: Option<String>,
    pub action_links: Vec<String>,
}

pub struct Extractor;

impl Extractor {
    pub fn extract(subject: Option<&str>, text_body: Option<&str>, html_body: Option<&str>) -> ExtractedMetadata {
        let mut meta = ExtractedMetadata::default();

        let full_text = format!(
            "{}\n{}\n{}",
            subject.unwrap_or_default(),
            text_body.unwrap_or_default(),
            html_body.unwrap_or_default()
        );

        // 1. Extract OTP
        for pattern in OTP_PATTERNS.iter() {
            if let Some(captures) = pattern.captures(&full_text) {
                if let Some(matched) = captures.get(1) {
                    let cleaned = matched.as_str().replace('-', "");
                    if cleaned.chars().all(|c| c.is_ascii_digit()) && cleaned.len() >= 4 && cleaned.len() <= 8 {
                        meta.otp = Some(cleaned);
                        break;
                    }
                }
            }
        }

        // 2. Extract Action Links from HTML (<a> tags)
        if let Some(html) = html_body {
            let document = Html::parse_document(html);
            if let Ok(selector) = Selector::parse("a[href]") {
                for element in document.select(&selector) {
                    if let Some(href) = element.value().attr("href") {
                        let link_text = element.text().collect::<Vec<_>>().join(" ");
                        let href_str = href.trim();

                        if href_str.starts_with("http://") || href_str.starts_with("https://") {
                            // Check if either the URL or the link text matches verification keywords
                            if LINK_KEYWORD_PATTERNS.is_match(href_str) || LINK_KEYWORD_PATTERNS.is_match(&link_text) {
                                if !meta.action_links.contains(&href_str.to_string()) {
                                    meta.action_links.push(href_str.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3. Extract Plain Text URLs
        let url_regex = Regex::new(r#"https?://[^\s<>'"()]+"#).unwrap();
        for mat in url_regex.find_iter(&full_text) {
            let url = mat.as_str();
            if LINK_KEYWORD_PATTERNS.is_match(url) && !meta.action_links.iter().any(|l| l == url) {
                meta.action_links.push(url.to_string());
            }
        }

        meta
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_github_otp_extraction() {
        let subject = "[GitHub] Please verify your device";
        let body = "Verification code: 849201. Please use this code to sign in to your GitHub account.";
        let extracted = Extractor::extract(Some(subject), Some(body), None);
        assert_eq!(extracted.otp, Some("849201".to_string()));
    }

    #[test]
    fn test_aws_verification_link() {
        let body = "Please click here to verify your email: https://signin.aws.amazon.com/verify?token=abc_123";
        let extracted = Extractor::extract(None, Some(body), None);
        assert!(!extracted.action_links.is_empty());
        assert_eq!(extracted.action_links[0], "https://signin.aws.amazon.com/verify?token=abc_123");
    }
}
