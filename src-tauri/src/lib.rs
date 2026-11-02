mod herdr;
mod sessions;
mod transcript;

use herdr::{HerdrCache, HerdrPlace};
use serde::Serialize;
use sessions::{LiveSession, Profile};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use transcript::{AgentRun, Context, Stats, Transcript};

/// Für beendete Sessions reicht das Dateiende: Titel, letzte Anfrage und letzter Schritt stehen dort.
const TAIL_BYTES: u64 = 300_000;
/// So viele Subagents zeigt eine Karte höchstens (laufende zuerst).
const CARD_AGENTS: usize = 4;
/// Profile und Logins ändern sich selten; `.claude.json` muss nicht alle 2 s gelesen werden.
const PROFILE_TTL: Duration = Duration::from_secs(30);

#[derive(Default)]
struct AppState {
    tails: Mutex<HashMap<PathBuf, (u64, Stats)>>,
    details: Mutex<HashMap<String, Transcript>>,
    profiles: Mutex<Option<(Instant, Vec<Profile>)>>,
    herdr: Mutex<HerdrCache>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionRow {
    session_id: String,
    live: bool,
    /// busy | idle | blocked | … aus `sessions/<pid>.json`; bei beendeten Sessions "ended"
    status: String,
    name: String,
    pid: Option<u32>,
    entrypoint: String,
    account: String,
    herdr: Option<HerdrPlace>,
    cwd: String,
    started_at: i64,
    title: String,
    first_prompt: String,
    last_prompt: String,
    prompts: u32,
    last_action: String,
    last_model: String,
    last_t: i64,
    context: Option<Context>,
    agents: Vec<AgentRun>,
    agents_total: usize,
    agents_running: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Overview {
    live: Vec<SessionRow>,
    recent: Vec<SessionRow>,
    profiles: Vec<Profile>,
    herdr_available: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Detail {
    session_id: String,
    live: Option<LiveSession>,
    status: String,
    account: String,
    herdr: Option<HerdrPlace>,
    stats: Stats,
}

impl AppState {
    fn profiles(&self) -> Vec<Profile> {
        let mut cache = self.profiles.lock().unwrap();
        match &*cache {
            Some((at, p)) if at.elapsed() < PROFILE_TTL => p.clone(),
            _ => {
                let p = sessions::profiles();
                *cache = Some((Instant::now(), p.clone()));
                p
            }
        }
    }

    fn tail(&self, path: &Path) -> Stats {
        let len = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let mut cache = self.tails.lock().unwrap();
        if let Some((cached_len, stats)) = cache.get(path) {
            if *cached_len == len {
                return stats.clone();
            }
        }
        let stats = Transcript::tail(path, TAIL_BYTES).unwrap_or_default();
        cache.insert(path.to_path_buf(), (len, stats.clone()));
        stats
    }

    /// Vollständig ausgewertetes Transkript; nach dem ersten Lesen kommen nur noch neue Zeilen dazu.
    /// Laufende Sessions brauchen das: Bilder in Tool-Ergebnissen füllen sonst das ganze gelesene Dateiende.
    fn full<R>(&self, session_id: &str, path: impl FnOnce() -> Option<PathBuf>, f: impl FnOnce(&Stats) -> R) -> Result<R, String> {
        let mut details = self.details.lock().unwrap();
        if !details.contains_key(session_id) {
            let path = path().ok_or_else(|| format!("Kein Transkript für Session {session_id} gefunden"))?;
            details.insert(session_id.to_string(), Transcript::open(path));
        }
        let t = details.get_mut(session_id).expect("eben eingefügt");
        t.refresh().map_err(|e| format!("Transkript nicht lesbar: {e}"))?;
        Ok(f(&t.stats))
    }

    fn herdr_places(&self, live: &[LiveSession]) -> HashMap<u32, HerdrPlace> {
        let pids: HashSet<u32> = live.iter().map(|l| l.pid).collect();
        self.herdr.lock().unwrap().places(&pids)
    }
}

/// herdr liest den Zustand am Terminal ab und liegt damit näher an dem, was man sieht, als
/// `sessions/<pid>.json` (das z. B. nach einem Befehl noch „shell“ meldet, obwohl Claude fertig ist).
fn status(live: Option<&LiveSession>, herdr: Option<&HerdrPlace>) -> String {
    let Some(live) = live else { return "ended".into() };
    match herdr.map(|h| h.status.as_str()) {
        Some("working") => "busy".into(),
        Some(s @ ("blocked" | "idle" | "done")) => s.into(),
        _ => live.status.clone().unwrap_or_else(|| "idle".into()),
    }
}

fn card_agents(stats: &Stats) -> Vec<AgentRun> {
    let mut agents: Vec<AgentRun> = stats.agents.iter().rev().take(20).cloned().collect();
    agents.sort_by_key(|a| (a.status != "running", -a.started));
    agents.truncate(CARD_AGENTS);
    agents
}

fn row(stats: &Stats, session_id: String, account: String, live: Option<&LiveSession>, herdr: Option<HerdrPlace>) -> SessionRow {
    SessionRow {
        live: live.is_some(),
        status: status(live, herdr.as_ref()),
        name: live.and_then(|l| l.name.clone()).unwrap_or_default(),
        pid: live.map(|l| l.pid),
        entrypoint: live.and_then(|l| l.entrypoint.clone()).unwrap_or_default(),
        account,
        herdr,
        // Das Transkript kennt den aktuellen Ordner (nach `cd`), die Session-Datei nur den Startordner.
        cwd: if stats.cwd.is_empty() { live.and_then(|l| l.cwd.clone()).unwrap_or_default() } else { stats.cwd.clone() },
        started_at: live.and_then(|l| l.started_at).unwrap_or(stats.first_t),
        session_id,
        title: stats.title.clone(),
        first_prompt: stats.first_prompt.clone(),
        last_prompt: stats.last_prompt.clone(),
        prompts: stats.prompts,
        last_action: stats.last_action.clone(),
        last_model: stats.last_model.clone(),
        last_t: stats.last_t,
        context: stats.context.clone(),
        agents: card_agents(stats),
        agents_total: stats.agents.len(),
        agents_running: stats.agents.iter().filter(|a| a.status == "running").count(),
    }
}

#[tauri::command]
async fn overview(recent_limit: usize, state: tauri::State<'_, AppState>) -> Result<Overview, String> {
    let profiles = state.profiles();
    let live = sessions::live_sessions(&profiles);
    let mut places = state.herdr_places(&live);
    let herdr_available = !places.is_empty();
    let live_ids: HashSet<&str> = live.iter().map(|l| l.session_id.as_str()).collect();

    let live_rows = live
        .iter()
        .map(|l| {
            let id = l.session_id.clone();
            let place = places.remove(&l.pid);
            let path = || sessions::find_transcript(&l.profile_dir, &id);
            // Eine frisch gestartete Session hat noch kein Transkript.
            state
                .full(&id, path, |s| row(s, id.clone(), l.account.clone(), Some(l), place.clone()))
                .unwrap_or_else(|_| row(&Stats::default(), id.clone(), l.account.clone(), Some(l), place.clone()))
        })
        .collect();
    let recent_rows = sessions::recent_transcripts(&profiles, recent_limit + live.len())
        .into_iter()
        .filter(|r| !live_ids.contains(r.session_id.as_str()))
        .take(recent_limit)
        .map(|r| row(&state.tail(&r.path), r.session_id, r.account, None, None))
        .collect();

    Ok(Overview { live: live_rows, recent: recent_rows, profiles, herdr_available })
}

#[tauri::command]
async fn session_detail(session_id: String, state: tauri::State<'_, AppState>) -> Result<Detail, String> {
    let profiles = state.profiles();
    let live = sessions::live_sessions(&profiles).into_iter().find(|s| s.session_id == session_id);
    let herdr = live.as_ref().and_then(|l| state.herdr_places(std::slice::from_ref(l)).remove(&l.pid));
    let found = sessions::find_transcript_any(&profiles, &session_id);
    let account = live.as_ref().map(|l| l.account.clone()).or_else(|| found.as_ref().map(|f| f.1.clone())).unwrap_or_default();
    let stats = state.full(&session_id, || found.map(|f| f.0), Stats::clone)?;
    let status = status(live.as_ref(), herdr.as_ref());
    Ok(Detail { session_id, live, status, account, herdr, stats })
}

#[tauri::command]
async fn focus_herdr(tab_id: String) -> Result<(), String> {
    herdr::focus_tab(&tab_id)
}

/// Öffnet den Arbeitsordner einer Session im Datei-Explorer. Nur bestehende Ordner, damit aus
/// dem Pfad nie eine Datei oder ein Programm gestartet wird.
#[tauri::command]
async fn open_folder(path: String) -> Result<(), String> {
    // Explorer versteht nur Backslashes, mit "/" öffnet er stattdessen „Dokumente“
    let dir = PathBuf::from(if cfg!(windows) { path.replace('/', "\\") } else { path });
    if !dir.is_dir() {
        return Err(format!("Ordner gibt es nicht mehr: {}", dir.display()));
    }
    let opener = if cfg!(windows) { "explorer" } else if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    // Kein Blick auf den Exit-Code: explorer.exe meldet auch beim Erfolg 1
    std::process::Command::new(opener)
        .arg(&dir)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Explorer ließ sich nicht öffnen: {e}"))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![overview, session_detail, focus_herdr, open_folder])
        .run(tauri::generate_context!())
        .expect("Agent Dashboard konnte nicht starten");
}
