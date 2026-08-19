use once_cell::sync::Lazy;
use regex::Regex;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use url::Url;

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
    Regex::new(r"(?i)(verify|confirm|activate|magic[-_]?link|login|session|auth|token|register|validate|signup)").unwrap()
});

// Suspicious open redirect query parameters
static REDIRECT_PARAMS: &[&str] = &["redirect", "redirect_to", "redirect_url", "url", "next", "dest", "target", "to", "return_to"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SafeLink {
    pub url: String,
    pub domain: String,
    pub is_safe: bool,
    pub has_open_redirect: bool,
    pub confidence: f32,
    pub reason: Option<String>,
}

// Extracted email credentials & metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractedMetadata {
    pub otp: Option<String>,
    pub action_links: Vec<SafeLink>,
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

        let mut candidate_urls: Vec<String> = Vec::new();

        // 2. Extract Action Links from HTML (<a> tags)
        if let Some(html) = html_body {
            let document = Html::parse_document(html);
            if let Ok(selector) = Selector::parse("a[href]") {
                for element in document.select(&selector) {
                    if let Some(href) = element.value().attr("href") {
                        let link_text = element.text().collect::<Vec<_>>().join(" ");
                        let href_str = href.trim();

                        if href_str.starts_with("http://") || href_str.starts_with("https://") {
                            // Check if either URL or link text matches verification keywords
                            if LINK_KEYWORD_PATTERNS.is_match(href_str) || LINK_KEYWORD_PATTERNS.is_match(&link_text) {
                                if !candidate_urls.contains(&href_str.to_string()) {
                                    candidate_urls.push(href_str.to_string());
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
            let url = mat.as_str().to_string();
            if LINK_KEYWORD_PATTERNS.is_match(&url) && !candidate_urls.contains(&url) {
                candidate_urls.push(url);
            }
        }

        // 4. Run Link Safety & Anti-Phishing Analysis
        for raw_url in candidate_urls {
            let safe_link = Self::analyze_link_safety(&raw_url);
            meta.action_links.push(safe_link);
        }

        meta
    }

    pub fn analyze_link_safety(raw_url: &str) -> SafeLink {
        let parsed = match Url::parse(raw_url) {
            Ok(u) => u,
            Err(_) => {
                return SafeLink {
                    url: raw_url.to_string(),
                    domain: "unknown".to_string(),
                    is_safe: false,
                    has_open_redirect: false,
                    confidence: 0.0,
                    reason: Some("Malformed URL syntax".to_string()),
                }
            }
        };

        let host_str = parsed.host_str().unwrap_or("").to_string();

        // Check for raw IP addresses
        let is_ip = host_str.split('.').count() == 4 && host_str.split('.').all(|p| p.parse::<u8>().is_ok());
        if is_ip {
            return SafeLink {
                url: raw_url.to_string(),
                domain: host_str,
                is_safe: false,
                has_open_redirect: false,
                confidence: 0.2,
                reason: Some("Raw IP address detected instead of domain".to_string()),
            };
        }

        // Check for Punycode homograph attack
        let has_punycode = host_str.contains("xn--");
        if has_punycode {
            return SafeLink {
                url: raw_url.to_string(),
                domain: host_str,
                is_safe: false,
                has_open_redirect: false,
                confidence: 0.3,
                reason: Some("Punycode homograph domain detected".to_string()),
            };
        }

        // Check for Open Redirects in query parameters
        let mut has_open_redirect = false;
        let mut redirect_target = None;
        for (k, v) in parsed.query_pairs() {
            if REDIRECT_PARAMS.contains(&k.to_lowercase().as_str()) {
                if v.starts_with("http://") || v.starts_with("https://") || v.starts_with("//") {
                    has_open_redirect = true;
                    redirect_target = Some(v.to_string());
                    break;
                }
            }
        }

        let is_safe = !has_open_redirect && parsed.scheme() == "https";
        let confidence = if is_safe { 0.98 } else if has_open_redirect { 0.40 } else { 0.70 };
        let reason = if has_open_redirect {
            Some(format!("Suspicious open redirect parameter targeting: {}", redirect_target.unwrap_or_default()))
        } else if parsed.scheme() != "https" {
            Some("Insecure HTTP protocol".to_string())
        } else {
            None
        };

        SafeLink {
            url: raw_url.to_string(),
            domain: host_str,
            is_safe,
            has_open_redirect,
            confidence,
            reason,
        }
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
        assert_eq!(extracted.action_links[0].url, "https://signin.aws.amazon.com/verify?token=abc_123");
        assert_eq!(extracted.action_links[0].domain, "signin.aws.amazon.com");
        assert!(extracted.action_links[0].is_safe);
        assert_eq!(extracted.action_links[0].confidence, 0.98);
    }

    #[test]
    fn test_open_redirect_safety_detection() {
        let malicious_link = "https://legit-service.com/login?redirect=https://evil-phishing-site.com/steal-session";
        let safe_link = Extractor::analyze_link_safety(malicious_link);
        assert!(!safe_link.is_safe);
        assert!(safe_link.has_open_redirect);
        assert!(safe_link.confidence < 0.5);
    }
}
