//! Der Ablauf in der Detailansicht: eigene Anfragen, Claudes Zwischenschritte (was Claude zwischen
//! zwei Tool-Aufrufen schreibt) und Tool-Aufrufe mit Eingaben, Ergebnis und Quellen.
//!
//! Claudes eigentliches Denken steht fast nie lesbar im Transkript: Claude Code speichert die
//! Thinking-Blöcke meist nur mit Signatur. Die Zwischennachrichten sind deshalb die Gedankenkette.
//!
//! Die Detailansicht holt den Ablauf alle zwei Sekunden komplett, darum ist alles knapp gekürzt.

use crate::transcript::{cut, one_line};
use serde::Serialize;
use serde_json::Value;

/// So viele Zeichen behält ein Zwischenschritt; die Oberfläche zeigt zwei Zeilen und klappt den Rest auf.
const THOUGHT_CHARS: usize = 600;
/// Die einzeilige Zusammenfassung eines Tool-Aufrufs neben dem Tool-Namen
const DETAIL_CHARS: usize = 240;
const INPUT_FIELDS: usize = 8;
const INPUT_CHARS: usize = 300;
const OUTPUT_CHARS: usize = 600;
const SOURCES: usize = 8;
const SOURCE_DESCRIPTION_CHARS: usize = 240;
/// Felder, die einen Tool-Aufruf in einer Zeile am besten beschreiben, in dieser Reihenfolge.
/// Sie stehen auch in der aufgeklappten Ansicht oben.
pub const INPUT_KEYS: [&str; 7] = ["description", "command", "file_path", "pattern", "url", "query", "prompt"];

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Field {
    pub key: String,
    pub value: String,
}

/// Eine Webseite, die ein Tool gelesen oder gefunden hat.
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Source {
    pub url: String,
    /// Host ohne `www.`; danach sucht die Oberfläche das Favicon.
    pub domain: String,
    /// Seitentitel; nur WebSearch liefert einen.
    pub title: String,
    /// Bei WebFetch Claudes Zusammenfassung der Seite
    pub description: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct Step {
    pub t: i64,
    /// prompt | thought | tool
    pub kind: &'static str,
    /// Tool-Name, Anfragetext oder Claudes Zwischenschritt
    pub label: String,
    pub detail: String,
    pub sidechain: bool,
    pub failed: bool,
    /// Das Ergebnis des Tool-Aufrufs ist da.
    pub done: bool,
    pub input: Vec<Field>,
    pub output: String,
    pub sources: Vec<Source>,
    #[serde(skip)]
    tool_use_id: String,
}

impl Step {
    fn new(t: i64, kind: &'static str, label: String, sidechain: bool) -> Self {
        Step {
            t,
            kind,
            label,
            detail: String::new(),
            sidechain,
            failed: false,
            done: false,
            input: Vec::new(),
            output: String::new(),
            sources: Vec::new(),
            tool_use_id: String::new(),
        }
    }

    pub fn prompt(t: i64, text: &str) -> Self {
        Step::new(t, "prompt", cut(&one_line(text), DETAIL_CHARS), false)
    }

    /// Nur aus der Hauptsession; Subagents schreiben viel Text für ihren Auftraggeber, nicht für den Menschen davor.
    pub fn thought(t: i64, text: &str) -> Self {
        Step::new(t, "thought", cut(&one_line(&plain(text)), THOUGHT_CHARS), false)
    }

    pub fn tool(t: i64, name: &str, id: &str, input: &Value, sidechain: bool) -> Self {
        Step {
            detail: cut(&summary(input).unwrap_or_default(), DETAIL_CHARS),
            input: input_fields(input),
            sources: input_source(input).into_iter().collect(),
            tool_use_id: id.to_string(),
            ..Step::new(t, "tool", name.to_string(), sidechain)
        }
    }
}

/// Markdown-Zeichen, die in einer einzeiligen Vorschau nur stören: Fett, Code, Überschriften.
/// `__` bleibt, weil es in Tool-Namen wie `mcp__blender__…` steht.
fn plain(text: &str) -> String {
    text.lines()
        .map(|line| line.trim_start().trim_start_matches('#').trim_start())
        .collect::<Vec<_>>()
        .join("\n")
        .replace("**", "")
        .replace('`', "")
}

/// Die eine Zeile, die einen Tool-Aufruf am besten beschreibt, z. B. der Befehl oder der Dateipfad.
pub fn summary(input: &Value) -> Option<String> {
    INPUT_KEYS.iter().find_map(|k| input[*k].as_str().filter(|s| !s.is_empty())).map(one_line)
}

/// Trägt das Ergebnis in den passenden Tool-Schritt ein.
pub fn finish(steps: &mut [Step], tool_use_id: &str, failed: bool, content: &Value, result: &Value) {
    let Some(step) = steps.iter_mut().rev().find(|st| st.tool_use_id == tool_use_id) else { return };
    step.done = true;
    step.failed = failed;
    step.output = cut(text_of(content).trim(), OUTPUT_CHARS);
    match step.label.as_str() {
        "WebSearch" => step.sources = search_sources(result),
        "WebFetch" => {
            let summary = cut(&one_line(result["result"].as_str().unwrap_or("")), SOURCE_DESCRIPTION_CHARS);
            if let Some(source) = step.sources.first_mut() {
                source.description = summary;
            }
        }
        _ => {}
    }
}

pub fn text_of(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks.iter().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n"),
        _ => String::new(),
    }
}

/// Die Zusammenfassungsfelder zuerst, dann der Rest alphabetisch (serde_json sortiert die Schlüssel).
fn input_fields(input: &Value) -> Vec<Field> {
    let Some(map) = input.as_object() else { return Vec::new() };
    let first = INPUT_KEYS.iter().filter_map(|k| map.get_key_value(*k));
    let rest = map.iter().filter(|(k, _)| !INPUT_KEYS.contains(&k.as_str()));
    first
        .chain(rest)
        .filter_map(|(key, value)| {
            let value = match value {
                Value::String(s) => s.trim().to_string(),
                Value::Null => return None,
                other => other.to_string(),
            };
            (!value.is_empty()).then(|| Field { key: key.clone(), value: cut(&value, INPUT_CHARS) })
        })
        .take(INPUT_FIELDS)
        .collect()
}

fn input_source(input: &Value) -> Option<Source> {
    let url = input["url"].as_str()?;
    Some(Source { url: url.to_string(), domain: domain_of(url)?, title: String::new(), description: String::new() })
}

/// WebSearch legt die Treffer in `toolUseResult.results[].content[]` ab, zwischen Textblöcken.
/// Eine Domain zählt nur einmal: Der Chip zeigt nur sie, drei Seiten von bestbuy.com sähen wie ein Fehler aus.
fn search_sources(result: &Value) -> Vec<Source> {
    let hits = result["results"].as_array().into_iter().flatten().filter_map(|r| r["content"].as_array()).flatten();
    let mut out: Vec<Source> = Vec::new();
    for hit in hits {
        let (Some(url), title) = (hit["url"].as_str(), hit["title"].as_str().unwrap_or("")) else { continue };
        let Some(domain) = domain_of(url) else { continue };
        if out.iter().any(|s| s.domain == domain) {
            continue;
        }
        out.push(Source { url: url.to_string(), domain, title: title.to_string(), description: String::new() });
        if out.len() == SOURCES {
            break;
        }
    }
    out
}

/// `https://www.figma.com/docs?x` → `figma.com`; nur http(s)
pub fn domain_of(url: &str) -> Option<String> {
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"))?;
    let host = rest.split(['/', '?', '#']).next()?.rsplit('@').next()?.split(':').next()?;
    let host = host.to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    (!host.is_empty()).then(|| host.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn tool_step_lists_summary_fields_first() {
        let input = json!({"timeout": 5000, "command": "npm test", "description": "Run tests", "run_in_background": null});
        let step = Step::tool(1, "Bash", "t1", &input, false);
        let keys: Vec<_> = step.input.iter().map(|f| (f.key.as_str(), f.value.as_str())).collect();
        assert_eq!(keys, [("description", "Run tests"), ("command", "npm test"), ("timeout", "5000")]);
        assert_eq!(step.detail, "Run tests");
    }

    #[test]
    fn web_search_collects_unique_sources() {
        let mut steps = vec![Step::tool(1, "WebSearch", "s1", &json!({"query": "prompt kit"}), false)];
        let result = json!({"query": "prompt kit", "results": [
            {"tool_use_id": "x", "content": [
                {"title": "prompt-kit", "url": "https://www.prompt-kit.com/docs"},
                {"title": "Source", "url": "https://prompt-kit.com/docs/source"},
                {"title": "Repo", "url": "https://github.com/ibelick/prompt-kit"}]},
            "Zusammenfassung als Text"]});
        finish(&mut steps, "s1", false, &json!("Web search results"), &result);
        let sources: Vec<_> = steps[0].sources.iter().map(|s| (s.domain.as_str(), s.title.as_str())).collect();
        assert_eq!(sources, [("prompt-kit.com", "prompt-kit"), ("github.com", "Repo")]);
        assert!(steps[0].done && !steps[0].failed);
    }

    #[test]
    fn web_fetch_describes_its_source() {
        let mut steps = vec![Step::tool(1, "WebFetch", "f1", &json!({"url": "https://seenebula.com/eu", "prompt": "Maße?"}), false)];
        finish(&mut steps, "f1", false, &json!("Keine Maße"), &json!({"result": "Die Seite nennt\nkeine Maße."}));
        let s = &steps[0].sources[0];
        assert_eq!((s.domain.as_str(), s.description.as_str()), ("seenebula.com", "Die Seite nennt keine Maße."));
        assert_eq!(steps[0].output, "Keine Maße");
    }

    #[test]
    fn thought_drops_markdown_but_keeps_tool_names() {
        let step = Step::thought(1, "## Stand\n**Fertig**: `mcp__blender__execute` lief");
        assert_eq!(step.label, "Stand Fertig: mcp__blender__execute lief");
    }

    #[test]
    fn domain_of_strips_scheme_port_and_www() {
        assert_eq!(domain_of("https://WWW.Figma.com:443/x?y").as_deref(), Some("figma.com"));
        assert_eq!(domain_of("http://user@host.de#a").as_deref(), Some("host.de"));
        assert_eq!(domain_of("file:///etc/passwd"), None);
    }
}
