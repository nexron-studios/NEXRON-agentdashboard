//! Kurztitel für das aktuelle Thema einer Session.
//!
//! Claude Code vergibt einen Titel nur einmal am Anfang. Wechselt die Arbeit im selben Chat das Thema,
//! fragt das Dashboard Haiku über `claude -p` nach einem neuen Titel: ohne API-Schlüssel, über das
//! Abo des Profils, dem die Session gehört. Ohne Tools, ohne MCP und ohne gespeichertes Transkript.
//! Die Titel landen in `%APPDATA%\com.nexron.agentdashboard\topics.json` und überleben Neustarts.

use crate::transcript::Stats;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Arbeitsordner der Titel-Aufrufe; Sessions mit diesem Ordner blendet die Übersicht aus.
pub const WORK_DIR_NAME: &str = "agentdashboard-titles";
/// Pro Session höchstens ein neuer Titel in diesem Abstand, auch nach Fehlern.
const MIN_INTERVAL: Duration = Duration::from_secs(120);
/// Höchstens so viele `claude -p` gleichzeitig.
const MAX_PARALLEL: usize = 2;
const TIMEOUT: Duration = Duration::from_secs(60);
/// Obergrenze der Datei: höchstens so viele Titel (die neuesten) und keiner älter als `MAX_AGE_SECS`.
/// Ein Eintrag hat unter 400 Byte, die Datei bleibt damit unter etwa 100 KB.
const MAX_ENTRIES: usize = 300;
const MAX_AGE_SECS: u64 = 30 * 24 * 3600;
const PROMPTS_FOR_TITLE: usize = 4;
const SYSTEM_PROMPT: &str = "Du benennst das aktuelle Thema einer Programmier-Session mit Claude Code. \
Antworte nur mit einem kurzen deutschen Titel aus 2 bis 6 Wörtern, ohne Anführungszeichen und ohne Punkt am Ende. \
Die letzte Anfrage zählt am meisten. Korrigiere Tippfehler.";

#[derive(Serialize, Deserialize, Clone)]
struct Entry {
    /// `current_task`, aus dem der Titel entstand; ändert es sich, ist der Titel veraltet
    source: String,
    title: String,
    /// Unix-Sekunden der Erzeugung (fehlt in Dateien der ersten Version, dann setzt `load` die Ladezeit)
    #[serde(default)]
    at: u64,
}

#[derive(Default)]
struct Inner {
    loaded: bool,
    entries: HashMap<String, Entry>,
    pending: HashSet<String>,
    last_try: HashMap<String, Instant>,
}

#[derive(Default, Clone)]
pub struct Topics(Arc<Mutex<Inner>>);

impl Topics {
    /// Titel des aktuellen Themas oder leer, solange die Session beim ersten Thema ist (dann passt
    /// Claudes eigener Titel). Ist der gespeicherte Titel veraltet und `generate` gesetzt, entsteht
    /// im Hintergrund ein neuer; bis dahin bleibt der alte stehen.
    pub fn topic(&self, session_id: &str, stats: &Stats, profile_dir: Option<&Path>, generate: bool) -> String {
        if stats.current_task.is_empty() || stats.current_task_is_first {
            return String::new();
        }
        let mut inner = self.0.lock().unwrap();
        inner.load();
        let entry = inner.entries.get(session_id).cloned();
        if entry.as_ref().is_some_and(|e| e.source == stats.current_task) {
            return entry.map(|e| e.title).unwrap_or_default();
        }
        let due = inner.last_try.get(session_id).is_none_or(|t| t.elapsed() >= MIN_INTERVAL);
        if generate && due && !inner.pending.contains(session_id) && inner.pending.len() < MAX_PARALLEL {
            inner.pending.insert(session_id.to_string());
            inner.last_try.insert(session_id.to_string(), Instant::now());
            let (topics, id, source) = (self.clone(), session_id.to_string(), stats.current_task.clone());
            let prompt = title_prompt(stats);
            let profile = profile_dir.map(Path::to_path_buf);
            std::thread::spawn(move || {
                let title = ask_haiku(&prompt, profile.as_deref());
                let mut inner = topics.0.lock().unwrap();
                inner.pending.remove(&id);
                if let Some(title) = title {
                    inner.entries.insert(id, Entry { source, title, at: unix_now() });
                    inner.save();
                }
            });
        }
        entry.map(|e| e.title).unwrap_or_default()
    }

    /// Läuft für diese Session gerade ein `claude -p`, der einen Titel liefern soll?
    pub fn pending(&self, session_id: &str) -> bool {
        self.0.lock().unwrap().pending.contains(session_id)
    }

    /// Nur gespeicherte Titel, ohne etwas zu erzeugen (für beendete Sessions).
    pub fn cached(&self, session_id: &str, stats: &Stats) -> String {
        if stats.current_task.is_empty() || stats.current_task_is_first {
            return String::new();
        }
        let mut inner = self.0.lock().unwrap();
        inner.load();
        inner.entries.get(session_id).map(|e| e.title.clone()).unwrap_or_default()
    }
}

impl Inner {
    fn load(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        if let Some(text) = store_path().and_then(|p| std::fs::read_to_string(p).ok()) {
            self.entries = serde_json::from_str(&text).unwrap_or_default();
        }
        // Einträge aus der ersten Version hatten noch keinen Zeitstempel
        let now = unix_now();
        for e in self.entries.values_mut().filter(|e| e.at == 0) {
            e.at = now;
        }
        if prune(&mut self.entries, now) {
            self.save();
        }
    }

    fn save(&mut self) {
        prune(&mut self.entries, unix_now());
        let Some(path) = store_path() else { return };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        // Erst in eine Nachbardatei, dann umbenennen: ein Absturz mitten im Schreiben lässt keine halbe Datei zurück
        let tmp = path.with_extension("json.tmp");
        if let Ok(text) = serde_json::to_string(&self.entries) {
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }
}

/// Wirft zu alte Einträge weg und behält höchstens `MAX_ENTRIES`, die neuesten. `true`, wenn sich etwas geändert hat.
fn prune(entries: &mut HashMap<String, Entry>, now: u64) -> bool {
    let before = entries.len();
    entries.retain(|_, e| now.saturating_sub(e.at) <= MAX_AGE_SECS);
    if entries.len() > MAX_ENTRIES {
        let mut by_age: Vec<(String, u64)> = entries.iter().map(|(k, e)| (k.clone(), e.at)).collect();
        by_age.sort_by_key(|(_, at)| std::cmp::Reverse(*at));
        for (k, _) in by_age.into_iter().skip(MAX_ENTRIES) {
            entries.remove(&k);
        }
    }
    entries.len() != before
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn store_path() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA").or_else(|| std::env::var_os("XDG_DATA_HOME"))?;
    Some(PathBuf::from(base).join("com.nexron.agentdashboard").join("topics.json"))
}

fn title_prompt(stats: &Stats) -> String {
    let recent: Vec<String> = stats
        .prompt_log
        .iter()
        .rev()
        .take(PROMPTS_FOR_TITLE)
        .rev()
        .map(|p| {
            let one_line = p.text.split_whitespace().collect::<Vec<_>>().join(" ");
            let short: String = one_line.chars().take(400).collect();
            format!("- {short}")
        })
        .collect();
    format!("Projektordner: {}\nAnfragen, neueste zuletzt:\n{}\n", stats.cwd, recent.join("\n"))
}

/// `claude.exe` direkt, nicht über den npm-Shim `claude.cmd`: so muss `cmd` keine Argumente neu zerlegen.
fn claude_binary() -> PathBuf {
    let path = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&path) {
        for candidate in [dir.join("claude.exe"), dir.join("node_modules/@anthropic-ai/claude-code/bin/claude.exe")] {
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    PathBuf::from("claude")
}

fn ask_haiku(prompt: &str, profile_dir: Option<&Path>) -> Option<String> {
    let work_dir = std::env::temp_dir().join(WORK_DIR_NAME);
    std::fs::create_dir_all(&work_dir).ok()?;
    let mut cmd = Command::new(claude_binary());
    cmd.args(["-p", "--model", "haiku", "--tools", "", "--no-session-persistence", "--strict-mcp-config", "--system-prompt", SYSTEM_PROMPT])
        .current_dir(&work_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    // Das Standardprofil (~/.claude) hat kein CLAUDE_CONFIG_DIR; die anderen brauchen ihren Ordner.
    match profile_dir {
        Some(dir) if dir.file_name().is_some_and(|n| n != ".claude") => cmd.env("CLAUDE_CONFIG_DIR", dir),
        _ => cmd.env_remove("CLAUDE_CONFIG_DIR"),
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn().ok()?;
    child.stdin.take()?.write_all(prompt.as_bytes()).ok()?;
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() > TIMEOUT => {
                let _ = child.kill();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(200)),
            Err(_) => return None,
        }
    }
    let out = child.wait_with_output().ok()?;
    out.status.success().then(|| clean_title(&String::from_utf8_lossy(&out.stdout))).flatten()
}

fn clean_title(raw: &str) -> Option<String> {
    let line = raw.lines().map(str::trim).find(|l| !l.is_empty())?;
    let title = line.trim_matches(|c: char| matches!(c, '"' | '\'' | '„' | '“' | '”' | '*' | '#' | '`' | '.') || c.is_whitespace());
    let len = title.chars().count();
    (2..=70).contains(&len).then(|| title.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prune_keeps_newest_and_drops_old() {
        let now = 10_000_000;
        let mut entries: HashMap<String, Entry> = (0..MAX_ENTRIES as u64 + 50)
            .map(|i| (format!("s{i}"), Entry { source: String::new(), title: String::new(), at: now - i }))
            .collect();
        entries.insert("alt".into(), Entry { source: String::new(), title: String::new(), at: now - MAX_AGE_SECS - 1 });
        assert!(prune(&mut entries, now));
        assert_eq!(entries.len(), MAX_ENTRIES);
        assert!(entries.contains_key("s0") && !entries.contains_key("alt") && !entries.contains_key(&format!("s{}", MAX_ENTRIES)));
        assert!(!prune(&mut entries, now));
    }

    #[test]
    fn cleans_model_output() {
        assert_eq!(clean_title("„Nexron Portfolio integrieren“.\n").as_deref(), Some("Nexron Portfolio integrieren"));
        assert_eq!(clean_title("\n**Login reparieren**"), Some("Login reparieren".into()));
        assert_eq!(clean_title(""), None);
    }

    /// `cargo test -- --ignored --nocapture ask_haiku_live`: echter Aufruf über das eigene Abo
    #[test]
    #[ignore]
    fn ask_haiku_live() {
        let profile = std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from);
        let t = ask_haiku("Anfragen, neueste zuletzt:\n- kannst du das dashboard nach tab namen sortieren\n", profile.as_deref());
        println!("{t:?}");
        assert!(t.is_some());
    }
}
