mod favicon;
mod herdr;
mod herdr_follow;
mod quota;
mod remote;
mod sessions;
mod steps;
mod topics;
mod transcript;
mod window;

use herdr::{HerdrCache, HerdrPlace};
use serde::Serialize;
use sessions::{LiveSession, Profile};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use transcript::{AgentRun, Context, Stats, Transcript};

/// Für beendete Sessions reicht das Dateiende: Titel, letzte Anfrage und letzter Schritt stehen dort.
const TAIL_BYTES: u64 = 300_000;
/// So viele Subagents bekommt eine Karte höchstens (laufende zuerst); ab vier Zeilen scrollt die Liste.
const CARD_AGENTS: usize = 20;
/// Profile und Logins ändern sich selten; `.claude.json` muss nicht alle 2 s gelesen werden.
const PROFILE_TTL: Duration = Duration::from_secs(30);
/// So viele beendete Sessions bleiben vollständig gelesen im Speicher (zuletzt geöffnete), laufende immer.
const DETAIL_KEEP: usize = 8;

#[derive(Default)]
struct AppState {
    tails: Mutex<HashMap<PathBuf, (u64, Stats)>>,
    details: Mutex<HashMap<String, Transcript>>,
    profiles: Mutex<Option<(Instant, Vec<Profile>)>>,
    herdr: Mutex<HerdrCache>,
    topics: topics::Topics,
    quota: quota::QuotaCache,
    /// Wann ein vollständig gelesenes Transkript zuletzt gebraucht wurde; zum Aufräumen von `details`
    detail_used: Mutex<HashMap<String, Instant>>,
    machines: remote::Machines,
    /// App-Cache; dort liegen die Spiegel der Transkripte anderer Machines
    cache_dir: OnceLock<PathBuf>,
}

/// ID dieses Rechners im Machine-Umschalter; die anderen tragen ihre herdr-Machine-ID
const LOCAL_MACHINE: &str = "local";

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
    current_task: String,
    current_task_is_first: bool,
    /// Von Haiku formulierter Titel des aktuellen Themas; leer, solange Claudes Titel noch passt
    topic: String,
    /// Ein Titel ist unterwegs (Claudes eigener oder ein Thema von Haiku); bis dahin keine Anfrage als Überschrift
    title_pending: bool,
    last_prompt: String,
    prompts: u32,
    last_action: String,
    last_model: String,
    last_t: i64,
    /// Wann Claude mit der letzten Aufgabe fertig wurde; nur sinnvoll, solange `status` idle/done ist
    finished_at: i64,
    context: Option<Context>,
    agents: Vec<AgentRun>,
    agents_total: usize,
    agents_running: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Overview {
    /// Die Machine, deren Sessions `live` und `recent` zeigen
    machine: String,
    machines: Vec<MachineSummary>,
    live: Vec<SessionRow>,
    recent: Vec<SessionRow>,
    profiles: Vec<Profile>,
    herdr_available: bool,
}

/// Ein Eintrag im Machine-Umschalter, mit den Zählern, die die Kopfzeile für die gezeigte Machine hat
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MachineSummary {
    id: String,
    label: String,
    /// Rechnername bzw. SSH-Ziel, für den Tooltip
    host: String,
    /// windows | macos | linux; leer, solange eine Machine noch nie geantwortet hat
    os: String,
    local: bool,
    connecting: bool,
    connected: bool,
    error: Option<String>,
    live: usize,
    approval: usize,
    busy: usize,
    waiting: usize,
}

impl MachineSummary {
    /// Gleiche Regeln wie die Zähler in der Kopfzeile (`format.ts`)
    fn count(mut self, rows: &[SessionRow]) -> Self {
        self.live = rows.len();
        for r in rows {
            match r.status.as_str() {
                "blocked" | "waiting" => self.approval += 1,
                "busy" | "shell" => self.busy += 1,
                "idle" | "done" if r.prompts > 0 => self.waiting += 1,
                _ => {}
            }
        }
        self
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Detail {
    session_id: String,
    live: Option<LiveSession>,
    status: String,
    account: String,
    herdr: Option<HerdrPlace>,
    topic: String,
    title_pending: bool,
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
        self.detail_used.lock().unwrap().insert(session_id.to_string(), Instant::now());
        let t = details.get_mut(session_id).expect("eben eingefügt");
        t.refresh().map_err(|e| format!("Transkript nicht lesbar: {e}"))?;
        Ok(f(&t.stats))
    }

    /// Hält die Zwischenspeicher klein, auch wenn die App tagelang läuft: Dateienden nur für die gerade
    /// gelistete „Zuletzt“-Auswahl, vollständige Transkripte für laufende plus die zuletzt geöffneten.
    fn evict(&self, live_ids: &HashSet<String>, recent_paths: &HashSet<PathBuf>) {
        self.tails.lock().unwrap().retain(|p, _| recent_paths.contains(p));
        let mut used = self.detail_used.lock().unwrap();
        let mut ended: Vec<(String, Instant)> =
            used.iter().filter(|(id, _)| !live_ids.contains(id.as_str())).map(|(id, t)| (id.clone(), *t)).collect();
        if ended.len() <= DETAIL_KEEP {
            return;
        }
        ended.sort_by_key(|(_, t)| std::cmp::Reverse(*t));
        let mut details = self.details.lock().unwrap();
        for (id, _) in ended.into_iter().skip(DETAIL_KEEP) {
            details.remove(&id);
            used.remove(&id);
        }
    }

    fn remotes(&self) -> Vec<Arc<remote::Remote>> {
        match self.cache_dir.get() {
            Some(dir) => self.machines.remotes(dir),
            None => Vec::new(),
        }
    }

    /// Die andere Machine mit dieser ID; `None` für diesen Rechner (und für unbekannte IDs)
    fn remote(&self, machine: Option<&str>) -> Option<Arc<remote::Remote>> {
        let id = machine.filter(|m| *m != LOCAL_MACHINE)?;
        self.remotes().into_iter().find(|r| r.id == id)
    }

    fn herdr_places(&self, live: &[LiveSession]) -> HashMap<u32, HerdrPlace> {
        let pids: HashSet<u32> = live.iter().map(|l| l.pid).collect();
        self.herdr.lock().unwrap().places(&pids)
    }
}

/// herdr liest den Zustand am Terminal ab und liegt damit näher an dem, was man sieht, als
/// `sessions/<pid>.json` (das z. B. nach einem Befehl noch „shell“ meldet, obwohl Claude fertig ist).
/// Ausnahme: Hat Claude Code selbst „idle“ geschrieben, ist der Zug sicher vorbei. herdr bleibt
/// manchmal auf „working“ hängen, dann stünde die Karte ewig auf „Arbeitet“.
fn status(live: Option<&LiveSession>, herdr: Option<&HerdrPlace>) -> String {
    let Some(live) = live else { return "ended".into() };
    match herdr.map(|h| h.status.as_str()) {
        Some("working") if live.status.as_deref() == Some("idle") => "idle".into(),
        Some("working") => "busy".into(),
        Some(s @ ("blocked" | "idle" | "done")) => s.into(),
        _ => live.status.clone().unwrap_or_else(|| "idle".into()),
    }
}

/// Der Statuswechsel auf „idle“ ist der genaue Zeitpunkt, an dem Claude fertig wurde. Meldet die
/// Session-Datei etwas anderes (herdr ist schneller), ist der letzte Transkript-Eintrag die beste Näherung.
fn finished_at(stats: &Stats, live: Option<&LiveSession>) -> i64 {
    match live {
        Some(l) if l.status.as_deref() == Some("idle") => l.status_updated_at.unwrap_or(stats.last_t),
        _ => stats.last_t,
    }
}

/// So lange nach der ersten Anfrage wird auf Claudes eigenen Titel gewartet; kommt keiner, steht danach die Anfrage da.
const TITLE_WAIT_MS: i64 = 30_000;

/// Kommt gleich ein Titel? Claude Code schreibt seinen Titel ein paar Sekunden nach der ersten Anfrage,
/// das Thema nach einem Wechsel liefert Haiku. Bis dahin soll die Überschrift nicht kurz die Anfrage zeigen.
fn title_pending(stats: &Stats, topic: &str, topics: &topics::Topics, session_id: &str) -> bool {
    let switched = !stats.current_task.is_empty() && !stats.current_task_is_first;
    if switched {
        return topic.is_empty() && topics.pending(session_id);
    }
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0);
    stats.title.is_empty() && stats.prompt_log.first().is_some_and(|p| now - p.t < TITLE_WAIT_MS)
}

fn card_agents(stats: &Stats) -> Vec<AgentRun> {
    let mut agents: Vec<AgentRun> = stats.agents.iter().rev().take(CARD_AGENTS).cloned().collect();
    agents.sort_by_key(|a| (a.status != "running", -a.started));
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
        current_task: stats.current_task.clone(),
        current_task_is_first: stats.current_task_is_first,
        topic: String::new(),
        title_pending: false,
        last_prompt: stats.last_prompt.clone(),
        prompts: stats.prompts,
        last_action: stats.last_action.clone(),
        last_model: stats.last_model.clone(),
        last_t: stats.last_t,
        finished_at: finished_at(stats, live),
        context: stats.context.clone(),
        agents: card_agents(stats),
        agents_total: stats.agents.len(),
        agents_running: stats.agents.iter().filter(|a| a.status == "running").count(),
    }
}

#[tauri::command]
async fn overview(recent_limit: usize, machine: Option<String>, state: tauri::State<'_, AppState>) -> Result<Overview, String> {
    let profiles = state.profiles();
    let live = sessions::live_sessions(&profiles);
    let mut places = state.herdr_places(&live);
    let herdr_available = !places.is_empty();
    let local_rows: Vec<SessionRow> = live
        .iter()
        .map(|l| {
            let place = places.remove(&l.pid);
            live_row(&state, l, place, Some(&l.profile_dir), || sessions::find_transcript(&l.profile_dir, &l.session_id))
        })
        .collect();

    // Alle Machines rechnen ihre Karten, damit der Umschalter auch für die verdeckten Freigaben zeigt.
    // Für andere Machines ist das billig: Ihre Spiegel werden wie lokale Transkripte nur fortgelesen.
    let mut summaries = vec![local_summary().count(&local_rows)];
    let mut live_ids: HashSet<String> = live.iter().map(|l| l.session_id.clone()).collect();
    let mut shown: Option<(Arc<remote::Remote>, remote::View, Vec<SessionRow>)> = None;
    for r in state.remotes() {
        let view = r.view();
        let rows: Vec<SessionRow> = view
            .live
            .iter()
            .map(|l| {
                let path = r.full_path(&l.session_id);
                live_row(&state, l, view.places.get(&l.session_id).cloned(), None, || path.is_file().then_some(path.clone()))
            })
            .collect();
        live_ids.extend(view.live.iter().map(|l| l.session_id.clone()));
        summaries.push(remote_summary(&r, &view).count(&rows));
        if machine.as_deref() == Some(r.id.as_str()) {
            shown = Some((r, view, rows));
        }
    }

    let (machine, live_rows, recent_rows, profiles, herdr_available, recent_paths) = match shown {
        Some((r, view, rows)) => {
            let (recent_rows, paths) = remote_recent_rows(&state, &r, &view, recent_limit);
            (r.id.clone(), rows, recent_rows, view.profiles, !view.places.is_empty(), paths)
        }
        None => {
            let (recent_rows, paths) = local_recent_rows(&state, &profiles, &live_ids, recent_limit);
            (LOCAL_MACHINE.to_string(), local_rows, recent_rows, profiles, herdr_available, paths)
        }
    };
    state.evict(&live_ids, &recent_paths);
    Ok(Overview { machine, machines: summaries, live: live_rows, recent: recent_rows, profiles, herdr_available })
}

/// Karte einer laufenden Session. `profile_dir` bestimmt das Abo für Haiku-Titel; ohne (andere
/// Machine) nimmt `claude -p` das Standardprofil dieses Rechners.
fn live_row(
    state: &AppState,
    l: &LiveSession,
    place: Option<HerdrPlace>,
    profile_dir: Option<&Path>,
    path: impl FnOnce() -> Option<PathBuf>,
) -> SessionRow {
    let id = l.session_id.clone();
    // Eine frisch gestartete Session hat noch kein Transkript.
    state
        .full(&id, path, |s| {
            let mut r = row(s, id.clone(), l.account.clone(), Some(l), place.clone());
            r.topic = state.topics.topic(&id, s, profile_dir, true);
            r.title_pending = title_pending(s, &r.topic, &state.topics, &id);
            r
        })
        .unwrap_or_else(|_| row(&Stats::default(), id.clone(), l.account.clone(), Some(l), place.clone()))
}

fn local_recent_rows(state: &AppState, profiles: &[Profile], live_ids: &HashSet<String>, limit: usize) -> (Vec<SessionRow>, HashSet<PathBuf>) {
    let recent: Vec<_> = sessions::recent_transcripts(profiles, limit + live_ids.len())
        .into_iter()
        .filter(|r| !live_ids.contains(&r.session_id))
        .take(limit)
        .collect();
    let paths = recent.iter().map(|r| r.path.clone()).collect();
    let rows = recent
        .into_iter()
        .map(|r| {
            let stats = state.tail(&r.path);
            let topic = state.topics.cached(&r.session_id, &stats);
            SessionRow { topic, ..row(&stats, r.session_id, r.account, None, None) }
        })
        .collect();
    (rows, paths)
}

/// Beendete Sessions einer anderen Machine aus ihren Anfang-und-Ende-Spiegeln. Was noch nicht
/// gespiegelt ist, fehlt kurz und kommt im nächsten Takt dazu.
fn remote_recent_rows(state: &AppState, r: &remote::Remote, view: &remote::View, limit: usize) -> (Vec<SessionRow>, HashSet<PathBuf>) {
    let live: HashSet<&str> = view.live.iter().map(|l| l.session_id.as_str()).collect();
    let recent: Vec<_> = view
        .recent
        .iter()
        .filter(|x| !live.contains(x.session_id.as_str()))
        .map(|x| (x, r.tail_path(&x.session_id)))
        .filter(|(_, path)| path.is_file())
        .take(limit)
        .collect();
    let paths = recent.iter().map(|(_, p)| p.clone()).collect();
    let rows = recent
        .into_iter()
        .map(|(x, path)| {
            let stats = state.tail(&path);
            let topic = state.topics.cached(&x.session_id, &stats);
            SessionRow { topic, ..row(&stats, x.session_id.clone(), x.account.clone(), None, None) }
        })
        .collect();
    (rows, paths)
}

fn local_summary() -> MachineSummary {
    let os = if cfg!(windows) { "windows" } else if cfg!(target_os = "macos") { "macos" } else { "linux" };
    MachineSummary {
        id: LOCAL_MACHINE.into(),
        label: match os {
            "macos" => "Dieser Mac",
            "windows" => "Dieser PC",
            _ => "Dieser Rechner",
        }
        .into(),
        host: computer_name().to_string(),
        os: os.into(),
        local: true,
        connecting: false,
        connected: true,
        error: None,
        live: 0,
        approval: 0,
        busy: 0,
        waiting: 0,
    }
}

fn remote_summary(r: &remote::Remote, view: &remote::View) -> MachineSummary {
    MachineSummary {
        id: r.id.clone(),
        label: r.label.clone(),
        host: r.target().to_string(),
        os: view.os.clone(),
        local: false,
        connecting: view.connecting,
        connected: view.connected,
        error: view.error.clone(),
        live: 0,
        approval: 0,
        busy: 0,
        waiting: 0,
    }
}

/// Name dieses Rechners, wie ihn das System zeigt („MacBook Pro von Jonas“)
fn computer_name() -> &'static str {
    static NAME: OnceLock<String> = OnceLock::new();
    NAME.get_or_init(|| {
        let from_env = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).ok();
        let mac = || sessions::run_quiet(Path::new("scutil"), &["--get", "ComputerName"]).map(|s| s.trim().to_string());
        from_env.or_else(mac).unwrap_or_default()
    })
}

#[tauri::command]
async fn session_detail(session_id: String, machine: Option<String>, state: tauri::State<'_, AppState>) -> Result<Detail, String> {
    if let Some(r) = state.remote(machine.as_deref()) {
        return remote_detail(&state, &r, session_id);
    }
    let profiles = state.profiles();
    let live = sessions::live_sessions(&profiles).into_iter().find(|s| s.session_id == session_id);
    let herdr = live.as_ref().and_then(|l| state.herdr_places(std::slice::from_ref(l)).remove(&l.pid));
    let found = sessions::find_transcript_any(&profiles, &session_id);
    let account = live.as_ref().map(|l| l.account.clone()).or_else(|| found.as_ref().map(|f| f.1.clone())).unwrap_or_default();
    let stats = state.full(&session_id, || found.map(|f| f.0), Stats::clone)?;
    let status = status(live.as_ref(), herdr.as_ref());
    let topic = state.topics.cached(&session_id, &stats);
    let title_pending = live.is_some() && title_pending(&stats, &topic, &state.topics, &session_id);
    Ok(Detail { session_id, live, status, account, herdr, topic, title_pending, stats })
}

/// Detail einer Session auf einer anderen Machine. Ist sie beendet, schickt die Machine ihr Transkript
/// erst jetzt vollständig; bis es da ist, füllt sich die Ansicht mit jedem Abruf weiter.
fn remote_detail(state: &AppState, r: &remote::Remote, session_id: String) -> Result<Detail, String> {
    let view = r.view();
    let live = view.live.iter().find(|l| l.session_id == session_id).cloned();
    if live.is_none() {
        r.want(&session_id);
    }
    let herdr = view.places.get(&session_id).cloned();
    let account = live
        .as_ref()
        .map(|l| l.account.clone())
        .or_else(|| view.recent.iter().find(|x| x.session_id == session_id).map(|x| x.account.clone()))
        .unwrap_or_default();
    let path = r.full_path(&session_id);
    let stats = state.full(&session_id, || path.is_file().then_some(path.clone()), Stats::clone)?;
    let status = status(live.as_ref(), herdr.as_ref());
    let topic = state.topics.cached(&session_id, &stats);
    let title_pending = live.is_some() && title_pending(&stats, &topic, &state.topics, &session_id);
    Ok(Detail { session_id, live, status, account, herdr, topic, title_pending, stats })
}

/// Plan quota per logged-in account. Blocking (keychain + curl), so off the async workers.
#[tauri::command]
async fn quota(app: tauri::AppHandle) -> Result<Vec<quota::AccountQuota>, String> {
    use tauri::Manager;
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state.quota.current(&state.profiles())
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn focus_herdr(tab_id: String, pane_id: String, machine: Option<String>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let remote = state.remote(machine.as_deref());
    if remote.is_some() {
        // Auf einer anderen Machine holt nur herdrs eigene Tastatur den Client dorthin (herdr_follow.rs).
        // Ohne herdr in Ghostty fokussiert wenigstens der Server den Tab.
        let target = navigator_target(&state, machine.as_deref(), Some(&pane_id))?;
        tauri::async_runtime::spawn_blocking(move || herdr_follow::send(&target)).await.map_err(|e| e.to_string())??;
    }
    herdr::focus_tab(remote.as_deref(), &tab_id)
}

/// Der herdr-Client in Ghostty wechselt auf dieselbe Machine wie der Umschalter im Dashboard.
#[tauri::command]
async fn follow_in_herdr(machine: String, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let target = navigator_target(&state, Some(&machine), None)?;
    tauri::async_runtime::spawn_blocking(move || herdr_follow::send(&target)).await.map_err(|e| e.to_string())?
}

/// Zeile im herdr-Navigator: die Machine selbst oder, mit `pane_id`, ein Pane darunter. Die Reihenfolge
/// ist die des herdr-Clients: Local, dann die gespeicherten Machines wie in `herdr machine list`.
fn navigator_target(state: &AppState, machine: Option<&str>, pane_id: Option<&str>) -> Result<herdr_follow::Target, String> {
    let profile = machine.filter(|m| *m != LOCAL_MACHINE).map(str::to_string);
    let local = herdr::local_navigator_rows().ok_or("herdr läuft auf diesem Rechner nicht")?;
    let mut endpoints = vec![(None, local)];
    endpoints.extend(state.remotes().iter().map(|r| (Some(r.id.clone()), r.view().navigator_rows)));
    let mut offset = 0;
    let mut row = None;
    for (id, rows) in &endpoints {
        if *id == profile {
            let within = match pane_id {
                None => Some(0),
                Some(p) => rows.iter().position(|r| r.as_deref() == Some(p)).map(|i| i + 1),
            };
            row = within.map(|w| offset + w);
        }
        offset += 1 + rows.len();
    }
    let row = row.ok_or("herdr kennt diese Machine oder diesen Tab nicht")?;
    Ok(herdr_follow::Target { row, total_rows: offset, profile, machine_only: pane_id.is_none() })
}

#[tauri::command]
async fn clear_herdr(pane_id: String, machine: Option<String>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    herdr::clear_chat(state.remote(machine.as_deref()).as_deref(), &pane_id)
}

#[tauri::command]
async fn stop_herdr(pane_id: String, machine: Option<String>, state: tauri::State<'_, AppState>) -> Result<(), String> {
    herdr::stop_agent(state.remote(machine.as_deref()).as_deref(), &pane_id)
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

/// Favicon einer Quelle als `data:`-URL; `None`, wenn die Seite keins hat. Blockiert (curl, Datei-Cache).
#[tauri::command]
async fn favicon(domain: String, app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri::Manager;
    let cache_dir = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || favicon::data_url(&cache_dir, &domain))
        .await
        .map_err(|e| e.to_string())
}

/// Öffnet eine Quelle im Standardbrowser. Nur http(s) und ohne Leerzeichen, damit aus dem
/// Transkript nie ein Programm oder eine lokale Datei gestartet wird.
#[tauri::command]
async fn open_url(url: String) -> Result<(), String> {
    let is_web = url.starts_with("https://") || url.starts_with("http://");
    if !is_web || url.chars().any(char::is_whitespace) {
        return Err(format!("Keine Web-Adresse: {url}"));
    }
    let opener = if cfg!(windows) { "explorer" } else if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    std::process::Command::new(opener)
        .arg(&url)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Browser ließ sich nicht öffnen: {e}"))
}

/// „Öffnen“ für Sessions ohne herdr-Tab. VS Code: `code <ordner>` holt genau das Fenster mit diesem
/// Projekt nach vorne (alle VS-Code-Fenster gehören demselben Prozess, das Fenster ließe sich sonst
/// nicht unterscheiden). Sonst das Terminal-Fenster, in dem der Claude-Prozess läuft.
#[tauri::command]
async fn open_session(pid: Option<u32>, cwd: String, entrypoint: String) -> Result<(), String> {
    if entrypoint == "claude-vscode" {
        let dir = PathBuf::from(if cfg!(windows) { cwd.replace('/', "\\") } else { cwd });
        if dir.is_dir() {
            let mut cmd = std::process::Command::new(if cfg!(windows) { "code.cmd" } else { "code" });
            cmd.arg(&dir);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                const CREATE_NO_WINDOW: u32 = 0x0800_0000;
                cmd.creation_flags(CREATE_NO_WINDOW);
            }
            if cmd.spawn().is_ok() {
                return Ok(());
            }
        }
    }
    window::focus_process_window(pid.ok_or("Keine Prozess-ID für diese Session")?)
}

/// Breite (logische px), ab der die Oberfläche in Originalgröße erscheint: drei Kartenspalten
/// brauchen 980 px Raster plus Rand und Scrollleiste. Schmalere Fenster zoomen heraus statt auf
/// zwei Spalten umzubrechen, damit das Dashboard klein neben dem Terminal dieselbe Übersicht zeigt.
const DESIGN_WIDTH: f64 = 1200.0;
/// Darunter wird die Schrift zu klein zum Lesen; dann bricht das Raster lieber um.
const MIN_ZOOM: f64 = 0.7;

fn zoom_to_width(win: &tauri::WebviewWindow, physical_width: u32) {
    let Ok(scale) = win.scale_factor() else { return };
    let zoom = (physical_width as f64 / scale / DESIGN_WIDTH).clamp(MIN_ZOOM, 1.0);
    // Schlägt das fehl, bleibt die Oberfläche nur unskaliert; das Dashboard funktioniert weiter.
    let _ = win.set_zoom((zoom * 100.0).round() / 100.0);
}

/// Startgröße kompakt für den Platz neben dem Terminal: so groß, dass bei herausgezoomter
/// Oberfläche sechs Karten ohne Scrollen passen, höchstens der Arbeitsbereich bis auf einen
/// schmalen Rand, mittig. Das Fenster ist bis hierhin unsichtbar (tauri.conf.json), damit es
/// nicht erst in der Standardgröße aufblitzt.
fn fit_to_monitor(win: &tauri::WebviewWindow) {
    const MAX_W: f64 = 960.0;
    const MAX_H: f64 = 760.0;
    const MARGIN: f64 = 16.0;
    let monitor = win.current_monitor().ok().flatten().or_else(|| win.primary_monitor().ok().flatten());
    if let (Some(m), Ok(outer), Ok(inner)) = (monitor, win.outer_size(), win.inner_size()) {
        let scale = m.scale_factor();
        let area = m.work_area();
        // Rahmen und Titelleiste gehören zur Außengröße, set_size setzt die Innengröße
        let frame_w = outer.width.saturating_sub(inner.width) as f64;
        let frame_h = outer.height.saturating_sub(inner.height) as f64;
        let w = (MAX_W * scale + frame_w).min(area.size.width as f64 - 2.0 * MARGIN * scale);
        let h = (MAX_H * scale + frame_h).min(area.size.height as f64 - 2.0 * MARGIN * scale);
        let x = area.position.x as f64 + (area.size.width as f64 - w) / 2.0;
        let y = area.position.y as f64 + (area.size.height as f64 - h) / 2.0;
        let _ = win.set_size(tauri::PhysicalSize::new((w - frame_w).round() as u32, (h - frame_h).round() as u32));
        let _ = win.show();
        let _ = win.set_position(tauri::PhysicalPosition::new(x.round() as i32, y.round() as i32));
    }
    let _ = win.show();
}

/// Aus Finder oder Dock gestartet erbt die App nur den launchd-PATH (`/usr/bin:/bin:…`), ohne `~/.local/bin` und
/// Homebrew. Dort liegen `claude`, `herdr` und `code`; ohne sie blieben Haiku-Titel und herdr-Zuordnung still leer.
#[cfg(target_os = "macos")]
fn extend_path_for_gui_launch() {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    let local_bin = std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/bin"));
    let extra = [local_bin, Some("/opt/homebrew/bin".into()), Some("/usr/local/bin".into())];
    for dir in extra.into_iter().flatten() {
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    if let Ok(joined) = std::env::join_paths(dirs) {
        std::env::set_var("PATH", joined);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Vor dem ersten Thread: set_var ist nur sicher, solange niemand parallel die Umgebung liest.
    #[cfg(target_os = "macos")]
    extend_path_for_gui_launch();
    tauri::Builder::default()
        .setup(|app| {
            use tauri::Manager;
            if let Ok(dir) = app.path().app_cache_dir() {
                let _ = app.state::<AppState>().cache_dir.set(dir);
            }
            if let Some(win) = app.get_webview_window("main") {
                fit_to_monitor(&win);
                if let Ok(size) = win.inner_size() {
                    zoom_to_width(&win, size.width);
                }
                let zoomed = win.clone();
                win.on_window_event(move |event| {
                    if let tauri::WindowEvent::Resized(size) = event {
                        zoom_to_width(&zoomed, size.width);
                    }
                });
            }
            Ok(())
        })
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![overview, session_detail, quota, focus_herdr, follow_in_herdr, clear_herdr, stop_herdr, open_folder, open_session, favicon, open_url])
        .run(tauri::generate_context!())
        .expect("Agent Dashboard konnte nicht starten");
}
