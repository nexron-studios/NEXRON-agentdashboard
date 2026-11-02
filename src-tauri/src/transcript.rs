//! Wertet ein Claude-Code-Transkript (`<session>.jsonl`) aus.
//!
//! Transkripte werden mehrere zehn MB groß und wachsen, solange die Session läuft.
//! `Transcript` merkt sich deshalb, bis wohin gelesen wurde, und verarbeitet bei
//! jedem `refresh` nur die neu angehängten Zeilen.

use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const MAX_EVENTS: usize = 6000;
const MAX_ERRORS: usize = 200;
const MAX_PROMPTS: usize = 300;
const PROMPT_CHARS: usize = 4000;
/// Vom Dateianfang wird so viel gelesen, dass die erste Anfrage sicher dabei ist.
const HEAD_BYTES: u64 = 400_000;
const DEFAULT_WINDOW: u64 = 200_000;
const LARGE_WINDOW: u64 = 1_000_000;
/// Felder, die einen Tool-Aufruf in einer Zeile am besten beschreiben, in dieser Reihenfolge.
const INPUT_KEYS: [&str; 7] = ["description", "command", "file_path", "pattern", "url", "query", "prompt"];
/// Zeilentypen, die nichts für das Dashboard enthalten, aber den Großteil der Datei ausmachen.
const SKIPPED_PREFIXES: [&[u8]; 3] = [br#"{"type":"api-request"#, br#"{"type":"attachment""#, br#"{"type":"file-history"#];

#[derive(Serialize, Clone, Default, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write: u64,
    pub requests: u32,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    pub tokens: u64,
    /// Geschätzt: 1M, sobald die Session je mehr als 200K im Kontext hatte, sonst 200K.
    pub window: u64,
    pub model: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AgentRun {
    pub id: String,
    pub description: String,
    pub agent_type: String,
    pub model: String,
    pub background: bool,
    pub started: i64,
    pub finished: Option<i64>,
    /// running | done | failed | stopped | sonstiger Status aus der Benachrichtigung
    pub status: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ErrorItem {
    pub t: i64,
    pub source: String,
    pub message: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Event {
    pub t: i64,
    /// prompt | tool | error
    pub kind: &'static str,
    pub label: String,
    pub sidechain: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Prompt {
    pub t: i64,
    pub text: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Cost {
    pub total_usd: f64,
    pub by_model: BTreeMap<String, f64>,
    pub lines_added: u64,
    pub lines_removed: u64,
}

#[derive(Serialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub title: String,
    /// Die erste eigene Anfrage: sagt meist am besten, worum es in der Session geht.
    pub first_prompt: String,
    /// Aktuelles Thema: die letzte Anfrage mit Inhalt, bereinigt und auf einen Satz gekürzt.
    /// Claude Code vergibt den Titel nur einmal; wer im selben Chat weiterarbeitet, braucht das hier.
    pub current_task: String,
    /// Das aktuelle Thema stammt noch aus der ersten Anfrage, der Titel passt also noch.
    pub current_task_is_first: bool,
    pub last_prompt: String,
    pub last_action: String,
    pub last_model: String,
    pub first_t: i64,
    pub last_t: i64,
    pub cwd: String,
    pub git_branch: String,
    pub version: String,
    pub permission_mode: String,
    pub prompts: u32,
    pub turns: u32,
    pub tool_calls: u32,
    pub compactions: u32,
    pub models: BTreeMap<String, ModelUsage>,
    pub context: Option<Context>,
    pub tools: BTreeMap<String, u32>,
    pub agents: Vec<AgentRun>,
    pub errors: Vec<ErrorItem>,
    pub error_count: u32,
    pub events: Vec<Event>,
    /// Eigene Anfragen in voller Länge (bis 4000 Zeichen), älteste zuerst.
    pub prompt_log: Vec<Prompt>,
    /// Stand der letzten `cost-state`-Zeile, die Claude Code selbst berechnet hat.
    pub cost: Option<Cost>,
    pub file_bytes: u64,
}

#[derive(Default)]
pub struct Transcript {
    path: PathBuf,
    offset: u64,
    pub stats: Stats,
    /// Nutzung je API-Antwort: Claude Code schreibt pro Inhaltsblock eine Zeile mit derselben Message-ID.
    usage_by_message: HashMap<String, (String, [u64; 4])>,
    tool_names: HashMap<String, String>,
    agent_index: HashMap<String, usize>,
    max_context: u64,
}

impl Transcript {
    pub fn open(path: PathBuf) -> Self {
        Transcript { path, ..Default::default() }
    }

    /// Liest alles, was seit dem letzten Aufruf an die Datei angehängt wurde.
    pub fn refresh(&mut self) -> std::io::Result<()> {
        let len = fs::metadata(&self.path)?.len();
        if len < self.offset {
            *self = Transcript::open(self.path.clone());
        }
        if len == self.offset {
            return Ok(());
        }
        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(self.offset))?;
        let mut buf = Vec::with_capacity((len - self.offset) as usize);
        file.read_to_end(&mut buf)?;
        // Eine halb geschriebene letzte Zeile bleibt liegen, bis sie vollständig ist.
        let Some(end) = buf.iter().rposition(|&b| b == b'\n').map(|i| i + 1) else {
            return Ok(());
        };
        self.ingest(&buf[..end]);
        self.offset += end as u64;
        self.stats.file_bytes = self.offset;
        Ok(())
    }

    /// Schnelle Zusammenfassung aus dem Dateiende: Titel, letzte Anfrage, letzter Schritt, Kontext.
    pub fn tail(path: &Path, bytes: u64) -> std::io::Result<Stats> {
        let mut file = File::open(path)?;
        let len = file.metadata()?.len();
        file.seek(SeekFrom::Start(len.saturating_sub(bytes)))?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        let mut t = Transcript::open(path.to_path_buf());
        t.ingest(&buf); // die erste Zeile ist abgeschnitten und wird beim Parsen verworfen
        t.stats.file_bytes = len;
        if len > bytes {
            // Die erste Anfrage steht am Dateianfang, außerhalb des gelesenen Endes.
            let mut head = Transcript::open(path.to_path_buf());
            let mut buf = Vec::new();
            File::open(path)?.take(HEAD_BYTES.min(len - bytes)).read_to_end(&mut buf)?;
            head.ingest(&buf);
            t.stats.first_prompt = head.stats.first_prompt;
        }
        Ok(t.stats)
    }

    pub fn ingest(&mut self, data: &[u8]) {
        for line in data.split(|&b| b == b'\n') {
            if line.len() < 2 || SKIPPED_PREFIXES.iter().any(|p| line.starts_with(p)) {
                continue;
            }
            if let Ok(entry) = serde_json::from_slice::<Value>(line) {
                self.entry(&entry);
            }
        }
        let events = &mut self.stats.events;
        if events.len() > MAX_EVENTS {
            events.drain(..events.len() - MAX_EVENTS);
        }
        let errors = &mut self.stats.errors;
        if errors.len() > MAX_ERRORS {
            errors.drain(..errors.len() - MAX_ERRORS);
        }
        let prompts = &mut self.stats.prompt_log;
        if prompts.len() > MAX_PROMPTS {
            prompts.drain(..prompts.len() - MAX_PROMPTS);
        }
    }

    fn entry(&mut self, v: &Value) {
        let t = v["timestamp"].as_str().and_then(parse_time).unwrap_or(0);
        let s = &mut self.stats;
        if t > 0 {
            if s.first_t == 0 || t < s.first_t {
                s.first_t = t;
            }
            s.last_t = s.last_t.max(t);
        }
        set_if_present(&mut s.cwd, &v["cwd"]);
        set_if_present(&mut s.git_branch, &v["gitBranch"]);
        set_if_present(&mut s.version, &v["version"]);

        match v["type"].as_str().unwrap_or("") {
            "ai-title" => set_if_present(&mut s.title, &v["aiTitle"]),
            "last-prompt" => set_if_present(&mut s.last_prompt, &v["lastPrompt"]),
            "permission-mode" => set_if_present(&mut s.permission_mode, &v["permissionMode"]),
            "cost-state" => s.cost = Some(parse_cost(v)),
            "system" if v["subtype"] == "compact_boundary" => s.compactions += 1,
            "assistant" => self.assistant(v, t),
            "user" => self.user(v, t),
            _ => {}
        }
    }

    fn assistant(&mut self, v: &Value, t: i64) {
        let msg = &v["message"];
        let sidechain = v["isSidechain"].as_bool().unwrap_or(false);
        let content = msg["content"].as_array().map(Vec::as_slice).unwrap_or(&[]);

        if v["isApiErrorMessage"].as_bool() == Some(true) {
            let text = content.iter().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join(" ");
            self.error(t, "API".into(), text, sidechain);
            return;
        }

        let model = msg["model"].as_str().unwrap_or("").to_string();
        if let (Some(id), false) = (msg["id"].as_str(), model.is_empty() || model == "<synthetic>") {
            self.usage(id, &model, &msg["usage"], sidechain);
        }

        let mut action = None;
        for block in content {
            match block["type"].as_str() {
                Some("tool_use") => action = Some(self.tool_use(block, t, sidechain)),
                Some("text") if action.is_none() => {
                    if let Some(text) = block["text"].as_str().filter(|s| !s.trim().is_empty()) {
                        action = Some(format!("💬 {}", one_line(text)));
                    }
                }
                _ => {}
            }
        }
        if let (Some(a), false) = (action, sidechain) {
            self.stats.last_action = a;
        }
    }

    fn usage(&mut self, id: &str, model: &str, u: &Value, sidechain: bool) {
        let n = |k: &str| u[k].as_u64().unwrap_or(0);
        let new = [n("input_tokens"), n("output_tokens"), n("cache_read_input_tokens"), n("cache_creation_input_tokens")];
        let old = self.usage_by_message.insert(id.to_string(), (model.to_string(), new));
        let s = &mut self.stats;
        let m = s.models.entry(model.to_string()).or_default();
        let old = match old {
            Some((_, old)) => old,
            None => {
                m.requests += 1;
                s.turns += 1;
                [0; 4]
            }
        };
        for (field, i) in [(&mut m.input, 0), (&mut m.output, 1), (&mut m.cache_read, 2), (&mut m.cache_write, 3)] {
            *field = (*field + new[i]).saturating_sub(old[i]);
        }
        s.last_model = model.to_string();

        if !sidechain {
            let tokens = new[0] + new[2] + new[3];
            self.max_context = self.max_context.max(tokens);
            let window = if self.max_context > DEFAULT_WINDOW { LARGE_WINDOW } else { DEFAULT_WINDOW };
            s.context = Some(Context { tokens, window, model: model.to_string() });
        }
    }

    fn tool_use(&mut self, block: &Value, t: i64, sidechain: bool) -> String {
        let name = short_tool_name(block["name"].as_str().unwrap_or("?"));
        let id = block["id"].as_str().unwrap_or("").to_string();
        let input = &block["input"];
        let s = &mut self.stats;
        *s.tools.entry(name.clone()).or_default() += 1;
        s.tool_calls += 1;
        s.events.push(Event { t, kind: "tool", label: name.clone(), sidechain });

        if name == "Agent" || name == "Task" {
            self.agent_index.insert(id.clone(), s.agents.len());
            s.agents.push(AgentRun {
                id: id.clone(),
                description: input["description"].as_str().unwrap_or("").to_string(),
                agent_type: input["subagent_type"].as_str().unwrap_or("general-purpose").to_string(),
                model: input["model"].as_str().unwrap_or("").to_string(),
                background: input["run_in_background"].as_bool().unwrap_or(false),
                started: t,
                finished: None,
                status: "running".into(),
            });
        }
        self.tool_names.insert(id, name.clone());

        match INPUT_KEYS.iter().find_map(|k| input[*k].as_str().filter(|s| !s.is_empty())) {
            Some(detail) => format!("{name}: {}", one_line(detail)),
            None => name,
        }
    }

    fn user(&mut self, v: &Value, t: i64) {
        let content = &v["message"]["content"];
        let sidechain = v["isSidechain"].as_bool().unwrap_or(false);
        if let Some(text) = content.as_str() {
            self.user_text(text, v, t);
            return;
        }
        let Some(blocks) = content.as_array() else { return };
        let mut first_text = None;
        for b in blocks {
            match b["type"].as_str() {
                Some("tool_result") => self.tool_result(b, v, t, sidechain),
                Some("text") if first_text.is_none() => first_text = b["text"].as_str(),
                _ => {}
            }
        }
        if let Some(text) = first_text {
            self.user_text(text, v, t);
        }
    }

    fn tool_result(&mut self, b: &Value, v: &Value, t: i64, sidechain: bool) {
        let id = b["tool_use_id"].as_str().unwrap_or("");
        let failed = b["is_error"].as_bool().unwrap_or(false);
        if failed {
            let source = self.tool_names.get(id).cloned().unwrap_or_else(|| "Tool".into());
            self.error(t, source, text_of(&b["content"]), sidechain);
        }
        if let Some(&i) = self.agent_index.get(id) {
            let result = &v["toolUseResult"];
            let agent = &mut self.stats.agents[i];
            set_if_present(&mut agent.model, &result["resolvedModel"]);
            if result["status"] == "async_launched" {
                agent.background = true;
            } else {
                agent.finished = Some(t);
                agent.status = if failed { "failed" } else { "done" }.into();
            }
        }
    }

    fn user_text(&mut self, text: &str, v: &Value, t: i64) {
        let text = text.trim_start();
        if text.starts_with("<task-notification>") {
            let (Some(id), Some(status)) = (between(text, "<tool-use-id>", "</tool-use-id>"), between(text, "<status>", "</status>")) else {
                return;
            };
            if let Some(&i) = self.agent_index.get(id) {
                let agent = &mut self.stats.agents[i];
                agent.finished = Some(t);
                agent.status = match status {
                    "completed" => "done",
                    "failed" => "failed",
                    "killed" | "stopped" => "stopped",
                    other => other,
                }
                .into();
            }
            return;
        }
        if v["isMeta"].as_bool() == Some(true) || text.is_empty() || text.starts_with('<') {
            return; // Slash-Befehle, System-Hinweise, eingefügte Kontexte
        }
        let s = &mut self.stats;
        s.prompts += 1;
        let full = cut(text.trim(), PROMPT_CHARS);
        if s.first_prompt.is_empty() {
            s.first_prompt = full.clone();
        }
        if let Some(task) = task_of(text) {
            s.current_task = task;
            s.current_task_is_first = s.prompts == 1;
        }
        s.events.push(Event { t, kind: "prompt", label: cut(&one_line(text), 160), sidechain: false });
        s.prompt_log.push(Prompt { t, text: full });
    }

    fn error(&mut self, t: i64, source: String, message: String, sidechain: bool) {
        let s = &mut self.stats;
        s.error_count += 1;
        let message = cut(message.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim(), 300);
        s.events.push(Event { t, kind: "error", label: source.clone(), sidechain });
        s.errors.push(ErrorItem { t, source, message });
    }
}

/// Macht aus einer Anfrage eine Überschrift, oder `None`, wenn sie kein Thema trägt („ja“, „weiter“, nur ein Bild).
fn task_of(text: &str) -> Option<String> {
    // Platzhalter für eingefügte Bilder und Texte entfernen: [Image #3], [Pasted text #1 +20 lines]
    let mut clean = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('[') {
        clean.push_str(&rest[..start]);
        let after = &rest[start..];
        match after.find(']') {
            Some(end) if after.starts_with("[Image") || after.starts_with("[Pasted") => rest = &after[end + 1..],
            _ => {
                clean.push('[');
                rest = &after[1..];
            }
        }
    }
    clean.push_str(rest);
    let clean = clean.split_whitespace().collect::<Vec<_>>().join(" ");
    let clean = clean.trim_start_matches(['-', '>', '+', ' ']).trim();
    if clean.chars().count() < 20 || clean.split(' ').count() < 4 {
        return None;
    }
    // Erster Satz, wenn er lang genug ist; sonst die ersten 120 Zeichen
    let sentence_end = clean
        .char_indices()
        .skip(25)
        .find(|&(i, c)| matches!(c, '.' | '?' | '!') && clean[i + c.len_utf8()..].starts_with(' '))
        .map(|(i, c)| i + c.len_utf8());
    let sentence = sentence_end.map_or(clean, |end| &clean[..end]);
    let mut out = cut_words(sentence, 90);
    if let Some(first) = out.chars().next() {
        out = first.to_uppercase().collect::<String>() + &out[first.len_utf8()..];
    }
    Some(out)
}

/// Kürzt an der letzten Wortgrenze vor `max_chars`, damit kein Wort mitten abbricht.
fn cut_words(text: &str, max_chars: usize) -> String {
    let Some((limit, _)) = text.char_indices().nth(max_chars) else { return text.to_string() };
    let end = text[..limit].rfind(' ').filter(|&i| i > limit / 2).unwrap_or(limit);
    format!("{}…", text[..end].trim_end_matches([',', ';', ':', ' ']))
}

fn parse_time(s: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(s).ok().map(|d| d.timestamp_millis())
}

fn parse_cost(v: &Value) -> Cost {
    let by_model = v["modelUsage"]
        .as_object()
        .map(|m| m.iter().map(|(k, u)| (k.clone(), u["costUSD"].as_f64().unwrap_or(0.0))).collect())
        .unwrap_or_default();
    Cost {
        total_usd: v["totalCostUSD"].as_f64().unwrap_or(0.0),
        by_model,
        lines_added: v["totalLinesAdded"].as_u64().unwrap_or(0),
        lines_removed: v["totalLinesRemoved"].as_u64().unwrap_or(0),
    }
}

fn set_if_present(target: &mut String, value: &Value) {
    if let Some(s) = value.as_str().filter(|s| !s.is_empty()) {
        if target != s {
            *target = s.to_string();
        }
    }
}

/// `mcp__claude-in-chrome__navigate` → `navigate`
fn short_tool_name(name: &str) -> String {
    name.rsplit("__").next().unwrap_or(name).to_string()
}

fn text_of(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks.iter().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}

fn between<'a>(text: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let from = text.find(start)? + start.len();
    let to = text[from..].find(end)? + from;
    Some(text[from..to].trim())
}

fn one_line(text: &str) -> String {
    cut(&text.split_whitespace().collect::<Vec<_>>().join(" "), 400)
}

fn cut(text: &str, max_chars: usize) -> String {
    match text.char_indices().nth(max_chars) {
        Some((i, _)) => format!("{}…", &text[..i]),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn lines(entries: &[Value]) -> Vec<u8> {
        let mut out = b"{\"abgeschnitten".to_vec();
        for e in entries {
            out.push(b'\n');
            out.extend(serde_json::to_vec(e).unwrap());
        }
        out.push(b'\n');
        out
    }

    fn assistant(id: &str, t: &str, content: Value, output: u64) -> Value {
        json!({"type": "assistant", "timestamp": t, "isSidechain": false, "message": {
            "id": id, "model": "claude-opus-5-5", "content": content,
            "usage": {"input_tokens": 10, "output_tokens": output, "cache_read_input_tokens": 1000, "cache_creation_input_tokens": 200}}})
    }

    #[test]
    fn aggregates_session() {
        let data = lines(&[
            json!({"type": "ai-title", "aiTitle": "Login-Bug beheben"}),
            json!({"type": "user", "timestamp": "2026-11-02T10:00:00Z", "cwd": "C:\\me", "message": {"content": "fix den Login"}}),
            assistant("m1", "2026-11-02T10:00:05Z", json!([{"type": "text", "text": "Ich schaue nach."}]), 5),
            assistant("m1", "2026-11-02T10:00:06Z", json!([{"type": "tool_use", "id": "t1", "name": "Bash",
                "input": {"command": "npm test", "description": "Run tests"}}]), 40),
            json!({"type": "user", "timestamp": "2026-11-02T10:00:09Z", "message": {"content": [
                {"type": "tool_result", "tool_use_id": "t1", "is_error": true, "content": "Exit code 1\nFAIL login.spec"}]}}),
            assistant("m2", "2026-11-02T10:01:00Z", json!([{"type": "tool_use", "id": "a1", "name": "Agent",
                "input": {"description": "Tests reparieren", "subagent_type": "general-purpose", "prompt": "…"}}]), 7),
            json!({"type": "user", "timestamp": "2026-11-02T10:01:01Z", "toolUseResult": {"status": "async_launched", "resolvedModel": "claude-sonnet-5-5"},
                "message": {"content": [{"type": "tool_result", "tool_use_id": "a1", "content": "gestartet"}]}}),
            json!({"type": "user", "timestamp": "2026-11-02T10:05:00Z", "message": {"content":
                "<task-notification>\n<tool-use-id>a1</tool-use-id>\n<status>completed</status>\n</task-notification>"}}),
            json!({"type": "last-prompt", "lastPrompt": "fix den Login"}),
        ]);
        let mut t = Transcript::default();
        t.ingest(&data);
        let s = &t.stats;

        assert_eq!(s.title, "Login-Bug beheben");
        assert_eq!(s.first_prompt, "fix den Login");
        assert_eq!(s.prompt_log.len(), 1);
        assert_eq!(s.last_prompt, "fix den Login");
        assert_eq!(s.last_action, "Agent: Tests reparieren");
        assert_eq!((s.prompts, s.turns, s.tool_calls, s.error_count), (1, 2, 2, 1));
        // m1 kam zweimal; gezählt wird nur der letzte Stand
        assert_eq!(s.models["claude-opus-5-5"], ModelUsage { input: 20, output: 47, cache_read: 2000, cache_write: 400, requests: 2 });
        assert_eq!(s.context, Some(Context { tokens: 1210, window: 200_000, model: "claude-opus-5-5".into() }));
        assert_eq!(s.errors[0], ErrorItem { t: parse_time("2026-11-02T10:00:09Z").unwrap(), source: "Bash".into(), message: "Exit code 1".into() });
        let a = &s.agents[0];
        assert_eq!((a.status.as_str(), a.model.as_str(), a.background), ("done", "claude-sonnet-5-5", true));
        assert_eq!(a.finished, parse_time("2026-11-02T10:05:00Z"));
        assert_eq!(s.last_t - s.first_t, 5 * 60 * 1000);
    }

    /// `AGENTDASHBOARD_SAMPLE=<pfad.jsonl> cargo test --release -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn parses_real_transcript() {
        let path = PathBuf::from(std::env::var("AGENTDASHBOARD_SAMPLE").expect("AGENTDASHBOARD_SAMPLE setzen"));
        let started = std::time::Instant::now();
        let mut t = Transcript::open(path);
        t.refresh().unwrap();
        let s = &t.stats;
        println!("{} MB in {:?}", s.file_bytes / 1_000_000, started.elapsed());
        println!("{:?} | prompts {} turns {} tools {} errors {} agents {} events {}", s.title, s.prompts, s.turns, s.tool_calls, s.error_count, s.agents.len(), s.events.len());
        println!("models {:?}\ncontext {:?}\ncost {:?}", s.models, s.context, s.cost.as_ref().map(|c| c.total_usd));
        println!("agents {:?}", s.agents.iter().map(|a| (&a.description, &a.status, &a.model)).collect::<Vec<_>>());
    }

    #[test]
    fn current_task_skips_short_replies_and_placeholders() {
        assert_eq!(task_of("ja"), None);
        assert_eq!(cut_words("eins zwei drei vier", 12), "eins zwei…");
        assert_eq!(task_of("[Image #4] weiter"), None);
        assert_eq!(
            task_of("[Image #3] dieser erste bereich macht iwann keinen sinn mehr. checkst du den kontext?").as_deref(),
            Some("Dieser erste bereich macht iwann keinen sinn mehr.")
        );
        let mut t = Transcript::default();
        let prompt = |text: &str| json!({"type": "user", "timestamp": "2026-11-02T10:00:00Z", "message": {"content": text}});
        t.ingest(&lines(&[prompt("bau mir bitte ein login formular"), prompt("ok")]));
        assert_eq!((t.stats.current_task.as_str(), t.stats.current_task_is_first), ("Bau mir bitte ein login formular", true));
        t.ingest(&lines(&[prompt("jetzt etwas ganz anderes: das dashboard sortieren")]));
        assert_eq!((t.stats.current_task.as_str(), t.stats.current_task_is_first), ("Jetzt etwas ganz anderes: das dashboard sortieren", false));
    }

    #[test]
    fn refresh_reads_only_appended_lines() {
        let dir = std::env::temp_dir().join(format!("agentdashboard-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("s.jsonl");
        fs::write(&path, b"{\"type\":\"ai-title\",\"aiTitle\":\"Eins\"}\n{\"type\":\"last-pro").unwrap();
        let mut t = Transcript::open(path.clone());
        t.refresh().unwrap();
        assert_eq!((t.stats.title.as_str(), t.stats.last_prompt.as_str()), ("Eins", ""));

        let mut f = fs::OpenOptions::new().append(true).open(&path).unwrap();
        std::io::Write::write_all(&mut f, b"mpt\",\"lastPrompt\":\"weiter\"}\n").unwrap();
        t.refresh().unwrap();
        assert_eq!(t.stats.last_prompt, "weiter");
        fs::remove_dir_all(dir).unwrap();
    }
}
