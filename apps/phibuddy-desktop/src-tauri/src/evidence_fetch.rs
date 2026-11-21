//! Explicit, narrow HTTPS text-source acquisition. Never called by model output.
//! No redirects, arbitrary domains, HTML parsing, cookies, or user credentials.
use std::{
    io::Read,
    time::Duration,
};

use reqwest::{
    blocking::Client,
    header::CONTENT_TYPE,
    redirect::Policy,
    Url,
};
use sha2::{Digest, Sha256};

pub const MAX_BODY_BYTES: usize = 262_144;

#[derive(Debug)]
pub struct FetchedText {
    pub url: String,
    pub host: String,
    pub content_type: String,
    pub sha256: String,
    pub content: String,
}

pub fn validate_url(input: &str) -> Result<Url, String> {
    let input = input.trim();
    if input.is_empty() || input.len() > 2048 {
        return Err("source URL length must be 1-2048 bytes".into());
    }
    let parsed = Url::parse(input).map_err(|_| "invalid source URL")?;
    if parsed.scheme() != "https"
        || parsed.port().is_some()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        return Err("only credential-free HTTPS URLs without port/query/fragment are allowed".into());
    }
    let host = parsed.host_str().ok_or("missing source host")?;
    let path = parsed.path();
    if path.contains('%') {
        return Err("encoded path segments are not accepted in the source preview".into());
    }
    let allowed = match host {
        "raw.githubusercontent.com" => {
            let segments: Vec<_> = path.split('/').filter(|part| !part.is_empty()).collect();
            segments.len() >= 4 && !path.ends_with('/')
        }
        "www.rfc-editor.org" => {
            path.strip_prefix("/rfc/rfc")
                .and_then(|value| value.strip_suffix(".txt"))
                .is_some_and(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
        }
        _ => false,
    };
    if !allowed {
        return Err("source host/path not allowlisted; use GitHub Raw or an RFC Editor .txt URL".into());
    }
    Ok(parsed)
}

/// Fetch at most 256 KiB of UTF-8 plaintext. A response snapshot and its
/// SHA-256 hash prove byte consistency *within the local ledger*, not truth.
pub fn fetch_text(url: &str) -> Result<FetchedText, String> {
    let parsed = validate_url(url)?;
    let client = Client::builder()
        .redirect(Policy::none())
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(25))
        .user_agent("PhiBuddyEvidence/0.8 (explicit desktop fetch)")
        .no_proxy()
        .build()
        .map_err(|e| format!("cannot configure evidence fetch: {e}"))?;
    let response = client.get(parsed.clone())
        .send().map_err(|e| format!("source fetch failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("source returned HTTP {}; redirects are disabled", response.status()));
    }
    if response.content_length().is_some_and(|len| len > MAX_BODY_BYTES as u64) {
        return Err("source exceeds 256 KiB limit".into());
    }
    let content_type = response.headers()
        .get(CONTENT_TYPE).and_then(|value| value.to_str().ok())
        .unwrap_or("application/octet-stream")
        .split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    if !matches!(content_type.as_str(), "text/plain" | "text/markdown" | "application/octet-stream") {
        return Err("only UTF-8 plaintext/Markdown is accepted; HTML and binary content are rejected".into());
    }
    let mut bytes = Vec::new();
    response.take(MAX_BODY_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("source stream error: {e}"))?;
    if bytes.is_empty() || bytes.len() > MAX_BODY_BYTES {
        return Err("source is empty or exceeds 256 KiB limit".into());
    }
    let content = String::from_utf8(bytes).map_err(|_| "source must be valid UTF-8")?;
    if content.trim().is_empty() || content.chars().any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t')) {
        return Err("source must be readable text without embedded control bytes".into());
    }
    let sha256 = format!("{:x}", Sha256::digest(content.as_bytes()));
    Ok(FetchedText {
        url: parsed.to_string(),
        host: parsed.host_str().unwrap_or("").to_string(),
        content_type,
        sha256,
        content,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_allowlisted_https_source_documents_are_accepted() {
        assert!(validate_url("https://www.rfc-editor.org/rfc/rfc9111.txt").is_ok());
        assert!(validate_url("https://raw.githubusercontent.com/rust-lang/rust/master/README.md").is_ok());
        for bad in [
            "http://www.rfc-editor.org/rfc/rfc9111.txt",
            "https://www.rfc-editor.org.evil.test/rfc/rfc9111.txt",
            "https://127.0.0.1/rfc/rfc9111.txt",
            "https://localhost/rfc/rfc9111.txt",
            "https://raw.githubusercontent.com:8443/a/b/c/d.md",
            "https://user:secret@raw.githubusercontent.com/a/b/c/d.md",
            "https://raw.githubusercontent.com/a/b/c/d.md?token=secret",
            "https://raw.githubusercontent.com/a/b/c/d.md#section",
            "https://raw.githubusercontent.com/a/b/c/%2E%2E/d.md",
            "https://www.rfc-editor.org/rfc/rfc9111.html",
            "https://www.rfc-editor.org/rfc/rfcabc.txt",
        ] {
            assert!(validate_url(bad).is_err(), "must reject {bad}");
        }
    }
}
