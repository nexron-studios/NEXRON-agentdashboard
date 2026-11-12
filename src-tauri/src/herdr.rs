//! Zuordnung zu herdr (Terminal-Workspace-Manager): in welchem Workspace und Tab läuft eine Session?
//!
//! herdr kennt Tabs („claude-3“) und Panes, aber keine Claude-Session-IDs. Die Brücke ist die PID:
//! `herdr pane process-info` nennt den Prozess im Vordergrund des Panes, und dieselbe PID steht in
//! `sessions/<pid>.json` des Claude-Profils. Ohne herdr oder ohne laufenden herdr-Server bleibt alles leer.
//! Auf anderen Machines (remote.rs) gibt es keine PIDs zum Abgleichen; dort nennt herdr die Session-ID selbst.

use crate::remote::Remote;
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
    /// herdr-Machine, auf der der Tab liegt; `None` auf diesem Rechner. Aktionen gehen dorthin.
    pub machine: Option<String>,
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

/// Ein herdr-Befehl auf diesem Rechner oder, mit `remote`, über SSH auf einer anderen Machine.
fn call_on(remote: Option<&Remote>, args: &[&str]) -> Option<Value> {
    let out = match remote {
        Some(r) => r.herdr(args)?,
        None => run_quiet(binary().as_ref()?, args)?,
    };
    let v: Value = serde_json::from_str(&out).ok()?;
    Some(v["result"].clone())
}

fn call(args: &[&str]) -> Option<Value> {
    call_on(None, args)
}

/// Gespeicherte SSH-Machines aus `herdr machine list`, nur eingeschaltete: (id, label, ssh-Ziel)
pub fn saved_machines() -> Vec<(String, String, String)> {
    let Some(out) = binary().as_ref().and_then(|bin| run_quiet(bin, &["machine", "list", "--json"])) else { return Vec::new() };
    let list: Vec<Value> = serde_json::from_str(&out).unwrap_or_default();
    list.iter()
        .filter(|m| m["enabled"].as_bool().unwrap_or(true))
        .filter_map(|m| Some((m["id"].as_str()?.to_string(), m["label"].as_str()?.to_string(), m["target"].as_str()?.to_string())))
        .collect()
}

fn label_of(snap: &Value, list: &str, key: &str, id: &str) -> String {
    snap[list]
        .as_array()
        .and_then(|a| a.iter().find(|x| x[key] == id))
        .and_then(|x| x["label"].as_str())
        .unwrap_or("")
        .to_string()
}

fn place(snap: &Value, pane: &Value, machine: Option<&str>) -> HerdrPlace {
    let tab_id = pane["tab_id"].as_str().unwrap_or("");
    HerdrPlace {
        workspace: label_of(snap, "workspaces", "workspace_id", pane["workspace_id"].as_str().unwrap_or("")),
        tab: label_of(snap, "tabs", "tab_id", tab_id),
        tab_id: tab_id.to_string(),
        pane_id: pane["pane_id"].as_str().unwrap_or("").to_string(),
        title: pane["terminal_title_stripped"].as_str().unwrap_or("").to_string(),
        status: pane["agent_status"].as_str().unwrap_or("").to_string(),
        machine: machine.map(str::to_string),
    }
}

/// Ort jeder Claude-Session in einem herdr-Snapshot einer anderen Machine, nach Session-ID. Dort
/// gibt es keine PIDs zum Abgleichen, aber herdr nennt die Session-ID des Agents selbst.
pub fn places_by_session(snap: &Value, machine: &str) -> HashMap<String, HerdrPlace> {
    snap["panes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|pane| pane["agent"] == "claude")
        .filter_map(|pane| Some((pane["agent_session"]["value"].as_str()?.to_string(), place(snap, pane, Some(machine)))))
        .collect()
}

impl HerdrCache {
    /// Ort jeder Claude-PID in herdr. `live_pids`: PIDs, die gerade laufen; ein Terminal, dessen
    /// gemerkte PID nicht mehr dabei ist, wird neu abgefragt (z. B. nach `/exit` und neuem `claude`).
    pub fn places(&mut self, live_pids: &HashSet<u32>) -> HashMap<u32, HerdrPlace> {
        let Some(snapshot) = call(&["api", "snapshot"]) else { return HashMap::new() };
        let snap = &snapshot["snapshot"];

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
            out.insert(pid, place(snap, pane, None));
        }
        self.pid_by_terminal.retain(|t, _| seen.contains(t));
        out
    }
}

/// Zeilen des herdr-Navigators (`prefix+g`) für einen Server, ohne die Machine-Zeile darüber: je
/// Workspace eine Zeile (`None`), darunter seine Panes in Tab-Reihenfolge. Bildet `navigator_rows`
/// aus herdr 0.9.3 nach (src/client/shell/aggregate_navigation.rs), solange nichts gefiltert ist.
pub fn navigator_rows(snap: &Value) -> Vec<Option<String>> {
    let list = |key: &str| snap[key].as_array().cloned().unwrap_or_default();
    let (workspaces, tabs, panes) = (list("workspaces"), list("tabs"), list("panes"));
    let mut rows = Vec::new();
    for ws in &workspaces {
        rows.push(None);
        for tab in tabs.iter().filter(|t| t["workspace_id"] == ws["workspace_id"]) {
            for pane in panes.iter().filter(|p| p["tab_id"] == tab["tab_id"]) {
                rows.push(pane["pane_id"].as_str().map(str::to_string));
            }
        }
    }
    rows
}

/// Navigator-Zeilen des herdr-Servers auf diesem Rechner; `None`, wenn herdr nicht läuft
pub fn local_navigator_rows() -> Option<Vec<Option<String>>> {
    call(&["api", "snapshot"]).map(|v| navigator_rows(&v["snapshot"]))
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

pub fn focus_tab(remote: Option<&Remote>, tab_id: &str) -> Result<(), String> {
    call_on(remote, &["tab", "focus", tab_id]).map(|_| ()).ok_or_else(|| "herdr hat den Tab nicht fokussiert. Läuft herdr?".into())
}

/// Tippt `/clear` in den Chat des Panes, als hätte man es selbst eingegeben. Der Status wird hier noch
/// einmal frisch abgefragt, denn die Karte kann bis zu 2 s alt sein: Während Claude arbeitet, würde der
/// Befehl eingereiht und den Verlauf mitten in der Aufgabe löschen. Eine offene Freigabe-Frage weist
/// `agent prompt` selbst ab, bevor es etwas sendet.
pub fn clear_chat(remote: Option<&Remote>, pane_id: &str) -> Result<(), String> {
    let info = call_on(remote, &["agent", "get", pane_id]).ok_or("herdr findet diesen Chat nicht mehr. Läuft herdr?")?;
    let agent = &info["agent"];
    if agent["agent"] != "claude" {
        return Err("In diesem herdr-Tab läuft kein Claude mehr.".into());
    }
    if !matches!(agent["agent_status"].as_str(), Some("idle" | "done")) {
        return Err("Claude arbeitet gerade oder wartet auf eine Freigabe. Löschen geht erst, wenn er fertig ist.".into());
    }
    call_on(remote, &["agent", "prompt", pane_id, "/clear"])
        .map(|_| ())
        .ok_or_else(|| "herdr hat /clear nicht gesendet.".into())
}

/// Bricht die laufende Antwort ab wie Esc im Terminal. Nur während Claude arbeitet: Bei einer offenen
/// Freigabe-Frage würde Esc die Freigabe ablehnen, und im Leerlauf gibt es nichts zu stoppen.
pub fn stop_agent(remote: Option<&Remote>, pane_id: &str) -> Result<(), String> {
    let info = call_on(remote, &["agent", "get", pane_id]).ok_or("herdr findet diesen Chat nicht mehr. Läuft herdr?")?;
    let agent = &info["agent"];
    if agent["agent"] != "claude" {
        return Err("In diesem herdr-Tab läuft kein Claude mehr.".into());
    }
    if agent["agent_status"] != "working" {
        return Err("Claude arbeitet gerade nicht, es gibt nichts zu stoppen.".into());
    }
    call_on(remote, &["agent", "send-keys", pane_id, "esc"])
        .map(|_| ())
        .ok_or_else(|| "herdr hat Esc nicht gesendet.".into())
}
