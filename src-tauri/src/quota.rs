//! Claude plan quota (5-hour and 7-day windows) per logged-in profile.
//!
//! Same source as NEXRON-TokenScope: the undocumented `oauth/usage` endpoint, called with the
//! token the CLI keeps in the macOS keychain or in `.credentials.json`. The dashboard asks on its
//! own so the header works without TokenScope running.
//!
//! The token is never refreshed here. Only the CLI that issued it may rotate it; an expired token
//! just keeps the last known numbers on screen until the next `claude` run renews it.

use crate::sessions::Profile;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
const OAUTH_BETA: &str = "oauth-2025-04-20";
const KEYCHAIN_SERVICE: &str = "Claude Code-credentials";
const CREDENTIALS_FILE: &str = ".credentials.json";
const REQUEST_TIMEOUT_SECS: &str = "10";
/// The endpoint answers 429 quickly, and TokenScope may be polling it at the same time.
const MIN_FETCH_INTERVAL: Duration = Duration::from_secs(90);
const RATE_LIMIT_COOLDOWN: Duration = Duration::from_secs(300);
/// Windows the header shows, in this order. The response also carries internal codename buckets.
const SHOWN_WINDOWS: [&str; 2] = ["five_hour", "seven_day"];

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct QuotaWindow {
    pub key: String,
    pub used_percent: f64,
    /// RFC 3339, as the endpoint sends it
    pub resets_at: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AccountQuota {
    pub label: String,
    pub email: Option<String>,
    pub windows: Vec<QuotaWindow>,
    /// When `windows` were fetched (ms); 0 if never
    pub fetched_at: i64,
    /// Why the numbers may be stale; `None` when the last request succeeded
    pub error: Option<String>,
}

struct Entry {
    checked: Instant,
    blocked_until: Option<Instant>,
    quota: AccountQuota,
}

#[derive(Default)]
pub struct QuotaCache {
    entries: Mutex<HashMap<PathBuf, Entry>>,
}

struct Credential {
    token: String,
    expires_at_ms: Option<i64>,
}

impl QuotaCache {
    pub fn current(&self, profiles: &[Profile]) -> Vec<AccountQuota> {
        let mut seen_emails = Vec::new();
        profiles
            .iter()
            .filter_map(|p| self.for_profile(p))
            .filter(|q| match &q.email {
                // Two config dirs on one account share one quota
                Some(email) if seen_emails.contains(email) => false,
                Some(email) => {
                    seen_emails.push(email.clone());
                    true
                }
                None => true,
            })
            .collect()
    }

    fn for_profile(&self, profile: &Profile) -> Option<AccountQuota> {
        let credential = read_credential(&profile.dir)?;
        let now = Instant::now();
        let previous = {
            let entries = self.entries.lock().unwrap();
            match entries.get(&profile.dir) {
                Some(e) if e.blocked_until.is_some_and(|until| now < until) => return Some(e.quota.clone()),
                Some(e) if now.duration_since(e.checked) < MIN_FETCH_INTERVAL => return Some(e.quota.clone()),
                Some(e) => Some(e.quota.clone()),
                None => None,
            }
        };

        let mut quota = previous.unwrap_or_else(|| AccountQuota {
            label: profile.label.clone(),
            email: profile.email.clone(),
            windows: Vec::new(),
            fetched_at: 0,
            error: None,
        });
        let mut blocked_until = None;
        match fetch(&credential) {
            Ok(windows) => {
                quota.windows = windows;
                quota.fetched_at = chrono::Utc::now().timestamp_millis();
                quota.error = None;
            }
            Err(FetchError::RateLimited) => {
                blocked_until = Some(now + RATE_LIMIT_COOLDOWN);
                quota.error = Some("Zu viele Abfragen, nächster Versuch in 5 Min.".into());
            }
            Err(FetchError::Other(message)) => quota.error = Some(message),
        }
        self.entries.lock().unwrap().insert(
            profile.dir.clone(),
            Entry { checked: now, blocked_until, quota: quota.clone() },
        );
        Some(quota)
    }
}

enum FetchError {
    RateLimited,
    Other(String),
}

fn fetch(credential: &Credential) -> Result<Vec<QuotaWindow>, FetchError> {
    if credential.expires_at_ms.is_some_and(|exp| exp <= chrono::Utc::now().timestamp_millis()) {
        return Err(FetchError::Other("Token abgelaufen, erneuert sich beim nächsten claude-Start".into()));
    }
    let (status, body) = curl_get(USAGE_URL, &credential.token).map_err(FetchError::Other)?;
    if status == 429 {
        return Err(FetchError::RateLimited);
    }
    if status != 200 {
        return Err(FetchError::Other(format!("Anthropic antwortet mit HTTP {status}")));
    }
    let payload: Value =
        serde_json::from_str(&body).map_err(|_| FetchError::Other("Antwort war kein JSON".into()))?;
    let windows: Vec<QuotaWindow> = SHOWN_WINDOWS
        .iter()
        .filter_map(|key| {
            let window = payload.get(key)?;
            Some(QuotaWindow {
                key: (*key).to_string(),
                used_percent: window.get("utilization")?.as_f64()?,
                resets_at: window.get("resets_at").and_then(Value::as_str).map(str::to_string),
            })
        })
        .collect();
    if windows.is_empty() {
        return Err(FetchError::Other("Keine Limitfenster in der Antwort, Format hat sich geändert".into()));
    }
    Ok(windows)
}

/// GET through the system curl. The headers go in on stdin (`-H @-`) so the bearer token never
/// shows up in the process list.
fn curl_get(url: &str, token: &str) -> Result<(u16, String), String> {
    let mut child = Command::new("curl")
        .args(["-s", "--max-time", REQUEST_TIMEOUT_SECS, "-H", "@-", "-w", "\n%{http_code}", url])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("curl nicht startbar: {e}"))?;
    let headers = format!(
        "Authorization: Bearer {token}\nanthropic-beta: {OAUTH_BETA}\nAccept: application/json\n"
    );
    child
        .stdin
        .take()
        .ok_or("curl ohne stdin")?
        .write_all(headers.as_bytes())
        .map_err(|e| format!("curl stdin: {e}"))?;
    let output = child.wait_with_output().map_err(|e| format!("curl: {e}"))?;
    let text = String::from_utf8_lossy(&output.stdout);
    let (body, status) = text.rsplit_once('\n').ok_or("Anthropic nicht erreichbar")?;
    let status: u16 = status.trim().parse().map_err(|_| "Anthropic nicht erreichbar".to_string())?;
    if status == 0 {
        return Err("Anthropic nicht erreichbar".into());
    }
    Ok((status, body.to_string()))
}

fn read_credential(dir: &Path) -> Option<Credential> {
    let raw = read_keychain(&keychain_service(dir))
        .or_else(|| std::fs::read_to_string(dir.join(CREDENTIALS_FILE)).ok())?;
    let payload: Value = serde_json::from_str(raw.trim()).ok()?;
    let oauth = payload.get("claudeAiOauth").unwrap_or(&payload);
    Some(Credential {
        token: oauth.get("accessToken")?.as_str()?.to_string(),
        expires_at_ms: oauth.get("expiresAt").and_then(Value::as_i64),
    })
}

/// The CLI stores the default `~/.claude` login under the plain service name, every other
/// config dir under the name plus the first 8 hex chars of the dir's SHA-256.
fn keychain_service(dir: &Path) -> String {
    if dir.file_name().is_some_and(|n| n == ".claude") {
        return KEYCHAIN_SERVICE.into();
    }
    let hash = Sha256::digest(dir.to_string_lossy().as_bytes());
    let hex: String = hash.iter().take(4).map(|b| format!("{b:02x}")).collect();
    format!("{KEYCHAIN_SERVICE}-{hex}")
}

#[cfg(target_os = "macos")]
fn read_keychain(service: &str) -> Option<String> {
    let output = Command::new("security")
        .args(["find-generic-password", "-s", service, "-w"])
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok().filter(|s| !s.trim().is_empty())
}

#[cfg(not(target_os = "macos"))]
fn read_keychain(_service: &str) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_dir_uses_plain_keychain_name() {
        assert_eq!(keychain_service(Path::new("/Users/x/.claude")), KEYCHAIN_SERVICE);
    }

    #[test]
    fn other_dirs_get_eight_hex_chars() {
        let name = keychain_service(Path::new("/Users/x/.claude-work"));
        let suffix = name.strip_prefix("Claude Code-credentials-").unwrap();
        assert_eq!(suffix.len(), 8);
        assert!(suffix.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
