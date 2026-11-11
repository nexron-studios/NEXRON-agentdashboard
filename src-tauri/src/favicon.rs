//! Favicons für die Quellen-Chips im Ablauf. Das Dashboard holt `https://<domain>/favicon.ico`
//! selbst und legt es im App-Cache ab, statt einen Favicon-Dienst zu fragen: so erfährt kein
//! Dritter, welche Seiten die Sessions aufrufen.

use base64::Engine;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, SystemTime};

const MAX_BYTES: &str = "200000";
const REQUEST_TIMEOUT_SECS: &str = "5";
/// Eine Seite ohne `/favicon.ico` wird erst nach einer Woche wieder gefragt.
const MISSING_RETRY: Duration = Duration::from_secs(7 * 24 * 3600);
const DOMAIN_MAX_LEN: usize = 253;
/// Hosts im eigenen Netz bekommen kein Favicon: keine Anfragen ins LAN aus Transkript-Inhalten.
const LOCAL_SUFFIXES: [&str; 4] = ["localhost", ".local", ".lan", ".internal"];

/// Das Favicon als `data:`-URL, oder `None`, wenn die Seite keins hat.
pub fn data_url(cache_dir: &Path, domain: &str) -> Option<String> {
    if !is_public_domain(domain) {
        return None;
    }
    let dir = cache_dir.join("favicons");
    let file = dir.join(domain);
    let bytes = match cached(&file) {
        Some(bytes) => bytes,
        None => {
            let fetched = fetch(domain).filter(|b| mime_of(b).is_some()).unwrap_or_default();
            // Auch „keins gefunden“ wird gespeichert (leere Datei), damit nicht jede Abfrage neu lädt.
            // Scheitert das Schreiben, wird beim nächsten Mal eben neu geladen.
            if let Err(e) = fs::create_dir_all(&dir).and_then(|_| fs::write(&file, &fetched)) {
                eprintln!("Favicon-Cache für {domain} nicht schreibbar: {e}");
            }
            fetched
        }
    };
    let mime = mime_of(&bytes)?;
    Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(&bytes)))
}

fn cached(file: &Path) -> Option<Vec<u8>> {
    let meta = fs::metadata(file).ok()?;
    let age = meta.modified().ok().and_then(|m| SystemTime::now().duration_since(m).ok()).unwrap_or_default();
    if meta.len() == 0 && age > MISSING_RETRY {
        return None;
    }
    fs::read(file).ok()
}

fn fetch(domain: &str) -> Option<Vec<u8>> {
    let url = format!("https://{domain}/favicon.ico");
    let out = Command::new("curl")
        .args(["-sfL", "--proto", "=https", "--proto-redir", "=https", "--max-redirs", "3"])
        .args(["--max-time", REQUEST_TIMEOUT_SECS, "--max-filesize", MAX_BYTES, &url])
        .output()
        .ok()?;
    out.status.success().then_some(out.stdout)
}

/// Erkennt das Bildformat an den ersten Bytes; manche Server liefern statt 404 eine HTML-Seite.
fn mime_of(bytes: &[u8]) -> Option<&'static str> {
    let head = &bytes[..bytes.len().min(256)];
    let text = String::from_utf8_lossy(head).to_ascii_lowercase();
    match head {
        [0, 0, 1, 0, ..] => Some("image/x-icon"),
        [0x89, b'P', b'N', b'G', ..] => Some("image/png"),
        [b'G', b'I', b'F', ..] => Some("image/gif"),
        [0xFF, 0xD8, ..] => Some("image/jpeg"),
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => Some("image/webp"),
        _ if text.trim_start().starts_with("<svg") || (text.starts_with("<?xml") && text.contains("<svg")) => Some("image/svg+xml"),
        _ => None,
    }
}

/// Nur öffentliche Hostnamen: Buchstaben, Ziffern, Punkt und Bindestrich, mindestens ein Punkt,
/// keine IP-Adresse. Der Name landet in einer URL für curl und als Dateiname im Cache.
fn is_public_domain(domain: &str) -> bool {
    let valid_chars = domain.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'.' || b == b'-');
    let is_ip = domain.bytes().all(|b| b.is_ascii_digit() || b == b'.');
    valid_chars
        && !is_ip
        && domain.len() <= DOMAIN_MAX_LEN
        && domain.contains('.')
        && !domain.contains("..")
        && !domain.starts_with(['.', '-'])
        && !domain.ends_with(['.', '-'])
        && !LOCAL_SUFFIXES.iter().any(|s| domain.ends_with(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_public_hostnames() {
        assert!(is_public_domain("prompt-kit.com"));
        assert!(is_public_domain("docs.github.com"));
        for bad in ["localhost", "echoride.local", "192.168.1.4", "a..b.com", "-x.com", "x.com/../etc", "EXAMPLE.com", "x.com -o /tmp/y"] {
            assert!(!is_public_domain(bad), "{bad}");
        }
    }

    #[test]
    fn rejects_html_instead_of_icon() {
        assert_eq!(mime_of(b"<!doctype html><html>"), None);
        assert_eq!(mime_of(&[0, 0, 1, 0, 1]), Some("image/x-icon"));
        assert_eq!(mime_of(b"<?xml version=\"1.0\"?><svg xmlns=\"\">"), Some("image/svg+xml"));
    }
}
