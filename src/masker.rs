use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;

struct Patterns {
    api_key: Regex,
    gh_token: Regex,
    ipv4: Regex,
    ipv6: Regex,
    env_secret: Regex,
    aws_key: Regex,
    bearer: Regex,
}

static PATTERNS: Lazy<Patterns> = Lazy::new(|| Patterns {
    // sk-ant-..., sk-proj-..., and generic sk-... API keys
    api_key: Regex::new(r"(?i)\bsk-(?:ant-|proj-)?[a-zA-Z0-9_\-]{20,}").unwrap(),
    // GitHub tokens: ghp_, gho_, ghs_, ghu_, github_pat_
    gh_token: Regex::new(r"\b(?:ghp|gho|ghs|ghu)_[a-zA-Z0-9]{36,}|github_pat_[a-zA-Z0-9_]{59,}").unwrap(),
    ipv4: Regex::new(r"\b(?:\d{1,3}\.){3}\d{1,3}\b").unwrap(),
    // Simplified IPv6 — full addresses only (colons + hex groups)
    ipv6: Regex::new(r"\b(?:[0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}\b").unwrap(),
    // Common env var patterns: KEY=VALUE where KEY looks secret-y
    env_secret: Regex::new(r#"(?i)(?:password|secret|token|api[_\-]?key|access[_\-]?key|private[_\-]?key)\s*[=:]\s*\S+"#).unwrap(),
    // AWS access/secret keys
    aws_key: Regex::new(r"\b(?:AKIA|ASIA|AROA|AIDA|ANPA|ANVA|AIPA)[A-Z0-9]{16}\b").unwrap(),
    bearer: Regex::new(r"(?i)bearer\s+[a-zA-Z0-9\-_\.]+").unwrap(),
});

pub fn mask(event: &mut Value) {
    mask_value(event);
}

fn mask_value(v: &mut Value) {
    match v {
        Value::String(s) => *s = mask_string(s),
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                mask_value(item);
            }
        }
        Value::Object(map) => {
            for val in map.values_mut() {
                mask_value(val);
            }
        }
        _ => {}
    }
}

fn mask_string(s: &str) -> String {
    let s = PATTERNS.api_key.replace_all(s, "[REDACTED:api_key]");
    let s = PATTERNS.gh_token.replace_all(&s, "[REDACTED:gh_token]");
    let s = PATTERNS.aws_key.replace_all(&s, "[REDACTED:aws_key]");
    let s = PATTERNS.bearer.replace_all(&s, "[REDACTED:bearer]");
    let s = PATTERNS.env_secret.replace_all(&s, "[REDACTED:secret]");
    // IPv4/6 after secrets so sk- patterns are caught first
    let s = PATTERNS.ipv4.replace_all(&s, "[REDACTED:ipv4]");
    let s = PATTERNS.ipv6.replace_all(&s, "[REDACTED:ipv6]");
    s.into_owned()
}
