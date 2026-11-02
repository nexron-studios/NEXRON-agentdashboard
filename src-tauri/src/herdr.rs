//! Zuordnung zu herdr (Terminal-Workspace-Manager): in welchem Workspace und Tab läuft eine Session?
//!
//! herdr kennt Tabs („claude-3“) und Panes, aber keine Claude-Session-IDs. Die Brücke ist die PID:
//! `herdr pane process-info` nennt den Prozess im Vordergrund des Panes, und dieselbe PID steht in
//! `sessions/<pid>.json` des Claude-Profils. Ohne herdr oder ohne laufenden herdr-Server bleibt alles leer.

use crate::sessions::run_quiet;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::OnceLock;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HerdrPlace {
    pub workspace: String,
    pub tab: String,
    pub tab_id: String,
    pub pane_id: String,
    /// Terminal-Titel, den Claude Code setzt (meist der Chat-Titel)
    pub title: String,
    /// working | blocked | idle | done | unknown: herdr liest das am Terminal ab
    pub status: String,
}

/// PID je Terminal; eine Pane-Abfrage kostet einen Prozessstart, deshalb nur bei neuen Terminals.
#[derive(Default)]
pub struct HerdrCache {
    pid_by_terminal: HashMap<String, Option<u32>>,
}

fn binary() -> &'static Option<PathBuf> {
    static BIN: OnceLock<Option<PathBuf>> = OnceLock::new();
    BIN.get_or_init(|| {
        let installed = std::env::var_os("USERPROFILE")
            .map(|h| PathBuf::from(h).join(".herdr/packages/standalone/current/herdr.exe"))
            .filter(|p| p.is_file());
        installed.or_else(|| run_quiet(&PathBuf::from("herdr"), &["--version"]).map(|_| PathBuf::from("herdr")))
    })
}

fn call(args: &[&str]) -> Option<Value> {
    let bin = binary().as_ref()?;
    let out = run_quiet(bin, args)?;
    let v: Value = serde_json::from_str(&out).ok()?;
    Some(v["result"].clone())
}

impl HerdrCache {
    /// Ort jeder Claude-PID in herdr. `live_pids`: PIDs, die gerade laufen; ein Terminal, dessen
    /// gemerkte PID nicht mehr dabei ist, wird neu abgefragt (z. B. nach `/exit` und neuem `claude`).
    pub fn places(&mut self, live_pids: &HashSet<u32>) -> HashMap<u32, HerdrPlace> {
        let Some(snapshot) = call(&["api", "snapshot"]) else { return HashMap::new() };
        let snap = &snapshot["snapshot"];
        let label_of = |list: &str, key: &str, id: &str| {
            snap[list]
                .as_array()
                .and_then(|a| a.iter().find(|x| x[key] == id))
                .and_then(|x| x["label"].as_str())
                .unwrap_or("")
                .to_string()
        };

        let mut out = HashMap::new();
        let mut seen = HashSet::new();
        for pane in snap["panes"].as_array().into_iter().flatten() {
            if pane["agent"] != "claude" {
                continue;
            }
            let (Some(pane_id), Some(terminal)) = (pane["pane_id"].as_str(), pane["terminal_id"].as_str()) else { continue };
            seen.insert(terminal.to_string());
            let cached = self.pid_by_terminal.get(terminal).copied().flatten();
            let pid = match cached.filter(|p| live_pids.contains(p)) {
                Some(p) => Some(p),
                None => {
                    let p = pane_pid(pane_id);
                    self.pid_by_terminal.insert(terminal.to_string(), p);
                    p
                }
            };
            let Some(pid) = pid else { continue };
            let tab_id = pane["tab_id"].as_str().unwrap_or("");
            out.insert(
                pid,
                HerdrPlace {
                    workspace: label_of("workspaces", "workspace_id", pane["workspace_id"].as_str().unwrap_or("")),
                    tab: label_of("tabs", "tab_id", tab_id),
                    tab_id: tab_id.to_string(),
                    pane_id: pane_id.to_string(),
                    title: pane["terminal_title_stripped"].as_str().unwrap_or("").to_string(),
                    status: pane["agent_status"].as_str().unwrap_or("").to_string(),
                },
            );
        }
        self.pid_by_terminal.retain(|t, _| seen.contains(t));
        out
    }
}

fn pane_pid(pane_id: &str) -> Option<u32> {
    let info = call(&["pane", "process-info", "--pane", pane_id])?;
    let info = &info["process_info"];
    let claude = info["foreground_processes"]
        .as_array()
        .and_then(|ps| ps.iter().find(|p| p["name"].as_str().is_some_and(|n| n.starts_with("claude"))))
        .and_then(|p| p["pid"].as_u64());
    claude.or_else(|| info["foreground_process_group_id"].as_u64()).map(|p| p as u32)
}

pub fn focus_tab(tab_id: &str) -> Result<(), String> {
    call(&["tab", "focus", tab_id]).map(|_| ()).ok_or_else(|| "herdr hat den Tab nicht fokussiert. Läuft herdr?".into())
}
