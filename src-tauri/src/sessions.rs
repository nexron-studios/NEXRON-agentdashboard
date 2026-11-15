//! Claude-Profile, laufende Sessions und Transkripte auf der Platte.
//!
//! Jedes Profil ist ein eigener Konfigurationsordner (`~/.claude`, `~/.claude-<name>`, `CLAUDE_CONFIG_DIR`)
//! mit eigenem Login. Laufende Sessions stehen dort als `sessions/<pid>.json`; `claude agents`
//! zeigt nur die des eigenen Profils, deshalb liest das Dashboard die Dateien selbst.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// „Standard“ für `~/.claude`, sonst der Teil nach `.claude-`
    pub label: String,
    pub dir: PathBuf,
    /// E-Mail des eingeloggten Accounts, zur Laufzeit aus `.claude.json` gelesen
    pub email: Option<String>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct LiveSession {
    pub session_id: String,
    pub pid: u32,
    pub cwd: Option<String>,
    pub kind: Option<String>,
    pub entrypoint: Option<String>,
    pub started_at: Option<i64>,
    pub name: Option<String>,
    pub status: Option<String>,
    /// Wann `status` zuletzt gewechselt hat (ms); bei „idle“ also, wann Claude fertig wurde
    pub status_updated_at: Option<i64>,
    /// Startzeit des Prozesses (Windows-FILETIME), damit eine wiederverwendete PID nicht als laufend gilt
    #[serde(skip_serializing)]
    pub proc_start: Option<String>,
    /// Nur bei Hintergrund-Jobs: Ordnername unter `jobs/`
    #[serde(skip_serializing)]
    pub job_id: Option<String>,
    /// Warum die Session steht, z. B. Claudes Meldung zum Nutzungslimit
    #[serde(default)]
    pub note: Option<String>,
    #[serde(skip_deserializing)]
    pub account: String,
    #[serde(skip)]
    pub profile_dir: PathBuf,
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")).unwrap_or_default())
}

pub fn profiles() -> Vec<Profile> {
    let home = home();
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&home)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            (name == ".claude" || name.starts_with(".claude-")) && p.join("projects").is_dir()
        })
        .collect();
    if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|d| !d.is_empty()).map(PathBuf::from) {
        if !dirs.iter().any(|d| same_dir(d, &dir)) && dir.join("projects").is_dir() {
            dirs.push(dir);
        }
    }
    dirs.sort();
    dirs.into_iter()
        .map(|dir| {
            let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
            let is_default = name == ".claude";
            let label = match name.strip_prefix(".claude-") {
                Some(rest) => rest.to_string(),
                None if is_default => "Standard".into(),
                None => name.clone(),
            };
            // Das Standardprofil legt seine .claude.json neben den Ordner, die anderen hinein.
            let state = if is_default { home.join(".claude.json") } else { dir.join(".claude.json") };
            Profile { label, email: account_email(&state), dir }
        })
        .collect()
}

fn same_dir(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().eq_ignore_ascii_case(&b.to_string_lossy())
}

fn account_email(state_file: &Path) -> Option<String> {
    let text = std::fs::read_to_string(state_file).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    v["oauthAccount"]["emailAddress"].as_str().map(str::to_string)
}

pub fn live_sessions(profiles: &[Profile]) -> Vec<LiveSession> {
    let mut out = Vec::new();
    for p in profiles {
        for entry in std::fs::read_dir(p.dir.join("sessions")).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let Ok(mut s) = serde_json::from_str::<LiveSession>(&text) else { continue };
            if !process_alive(s.pid, s.proc_start.as_deref()) {
                continue; // Datei einer abgestürzten Session
            }
            if s.cwd.as_deref().is_some_and(|c| c.trim_end_matches(['\\', '/']).ends_with(crate::topics::WORK_DIR_NAME)) {
                continue; // eigener Titel-Aufruf (`claude -p`) des Dashboards
            }
            s.account = p.label.clone();
            s.profile_dir = p.dir.clone();
            if s.kind.as_deref() == Some("bg") {
                background_job(&mut s, &p.dir);
            }
            out.push(s);
        }
    }
    // Wird eine Session in einem zweiten Terminal fortgesetzt, laufen zwei Prozesse auf demselben
    // Transkript. Eine Karte reicht; die zuletzt gestartete ist die, mit der gerade gearbeitet wird.
    out.sort_by_key(|s| std::cmp::Reverse(s.started_at.unwrap_or(0)));
    let mut seen = std::collections::HashSet::new();
    out.retain(|s| seen.insert(s.session_id.clone()));
    out
}

/// Hintergrund-Job (`kind: "bg"`): So legt Claude Code eine Session ab, die ins Nutzungslimit gelaufen
/// ist. Die Session-Datei bleibt dann auf ihrem letzten Status stehen (oft „shell“), den echten Stand
/// hat nur `jobs/<id>/state.json`. Der Name ist Claudes Titel; er stünde sonst doppelt auf der Karte.
fn background_job(s: &mut LiveSession, profile_dir: &Path) {
    s.name = Some("Hintergrund".into());
    let id = s.job_id.clone().unwrap_or_else(|| s.session_id.chars().take(8).collect());
    let Ok(text) = std::fs::read_to_string(profile_dir.join("jobs").join(id).join("state.json")) else { return };
    let Ok(job) = serde_json::from_str::<Value>(&text) else { return };
    if job["tempo"] == "blocked" || job["state"] == "blocked" {
        s.status = Some("limit".into());
        s.note = job["detail"].as_str().map(str::to_string);
    }
}

#[cfg(windows)]
fn process_alive(pid: u32, proc_start: Option<&str>) -> bool {
    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
        fn GetExitCodeProcess(process: isize, code: *mut u32) -> i32;
        fn GetProcessTimes(process: isize, c: *mut FileTime, e: *mut FileTime, k: *mut FileTime, u: *mut FileTime) -> i32;
        fn CloseHandle(handle: isize) -> i32;
    }
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    const STILL_ACTIVE: u32 = 259;
    // SAFETY: Nur Abfragen auf einem eigenen Handle, das am Ende geschlossen wird.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h == 0 {
            return false;
        }
        let mut code = 0u32;
        let mut alive = GetExitCodeProcess(h, &mut code) != 0 && code == STILL_ACTIVE;
        if alive {
            let (mut c, mut e, mut k, mut u) = Default::default();
            if GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u) != 0 {
                let created = ((c.high as u64) << 32) | c.low as u64;
                if let Some(expected) = proc_start.and_then(|s| s.parse::<u64>().ok()) {
                    // Gleiche PID, anderer Prozess: um mehr als eine Sekunde abweichende Startzeit
                    alive = created.abs_diff(expected) < 10_000_000;
                }
            }
        }
        CloseHandle(h);
        alive
    }
}

#[cfg(not(windows))]
fn process_alive(pid: u32, proc_start: Option<&str>) -> bool {
    extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    const EPERM: i32 = 1;
    // `/proc` gibt es nur unter Linux; Signal 0 prüft überall nur, ob die PID existiert.
    // SAFETY: Signal 0 wird nicht zugestellt.
    let exists = unsafe { kill(pid as i32, 0) } == 0
        || std::io::Error::last_os_error().raw_os_error() == Some(EPERM);
    if !exists {
        return false;
    }
    let Some(expected) = proc_start else { return true };
    // Claude Code schreibt die Startzeit wie `ps -o lstart` in UTC („Wed Oct 28 10:11:40 2026“).
    let Ok(out) = Command::new("ps")
        .args(["-o", "lstart=", "-p", &pid.to_string()])
        .env("LC_ALL", "C")
        .env("TZ", "UTC")
        .output()
    else {
        return true;
    };
    let words = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
    // Gleiche PID, anderer Prozess: abweichende Startzeit
    words(&String::from_utf8_lossy(&out.stdout)) == words(expected)
}

pub fn find_transcript(profile_dir: &Path, session_id: &str) -> Option<PathBuf> {
    let file = format!("{session_id}.jsonl");
    project_dirs(profile_dir).map(|d| d.join(&file)).find(|p| p.is_file())
}

/// Sucht in allen Profilen; liefert Pfad und Account-Label.
pub fn find_transcript_any(profiles: &[Profile], session_id: &str) -> Option<(PathBuf, String)> {
    profiles.iter().find_map(|p| find_transcript(&p.dir, session_id).map(|path| (path, p.label.clone())))
}

fn project_dirs(profile_dir: &Path) -> impl Iterator<Item = PathBuf> {
    std::fs::read_dir(profile_dir.join("projects"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
}

pub struct RecentTranscript {
    pub session_id: String,
    pub path: PathBuf,
    pub account: String,
    pub modified: SystemTime,
}

/// Die zuletzt geänderten Transkripte aller Profile, neueste zuerst.
pub fn recent_transcripts(profiles: &[Profile], limit: usize) -> Vec<RecentTranscript> {
    let mut all: Vec<RecentTranscript> = profiles
        .iter()
        .flat_map(|p| project_dirs(&p.dir).map(move |d| (p, d)))
        .flat_map(|(p, d)| std::fs::read_dir(d).into_iter().flatten().flatten().map(move |e| (p, e)))
        .filter_map(|(p, e)| {
            let path = e.path();
            if path.extension()? != "jsonl" {
                return None;
            }
            let meta = e.metadata().ok()?;
            // Leere Sessions (nur geöffnet, nichts gefragt) sind kaum größer als 2 KB.
            (meta.len() > 4096).then_some(())?;
            Some(RecentTranscript {
                session_id: path.file_stem()?.to_str()?.to_string(),
                account: p.label.clone(),
                modified: meta.modified().ok()?,
                path,
            })
        })
        .collect();
    all.sort_by(|a, b| b.modified.cmp(&a.modified));
    // `claude-takealong` kopiert ein Transkript in ein anderes Profil; dieselbe Session liegt dann
    // zweimal auf der Platte. Es zählt die zuletzt geschriebene Kopie.
    let mut seen = std::collections::HashSet::new();
    all.retain(|r| seen.insert(r.session_id.clone()));
    all.truncate(limit);
    all
}

/// On Windows every console program opens a visible console unless told otherwise. The dashboard
/// starts programs on every refresh, so without this a console would flash each time.
pub fn hide_console(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Startet ein Kommandozeilenprogramm ohne Konsolenfenster und liefert stdout bei Erfolg.
pub fn run_quiet(program: &Path, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new(program);
    hide_console(cmd.args(args));
    let out = cmd.output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;

    fn own_start() -> String {
        let out = Command::new("ps")
            .args(["-o", "lstart=", "-p", &std::process::id().to_string()])
            .env("LC_ALL", "C")
            .env("TZ", "UTC")
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    #[test]
    fn own_process_counts_as_alive() {
        assert!(process_alive(std::process::id(), None));
        assert!(process_alive(std::process::id(), Some(&own_start())));
    }

    #[test]
    fn reused_pid_with_other_start_time_is_dead() {
        assert!(!process_alive(std::process::id(), Some("Thu Jan  1 00:00:00 1970")));
    }

    /// `cargo test -- --ignored --nocapture lists_live_sessions`: zeigt, was das Dashboard gerade als laufend sieht
    #[test]
    #[ignore]
    fn lists_live_sessions() {
        for s in live_sessions(&profiles()) {
            println!("{} {} {:?} {:?}", s.account, s.pid, s.name, s.status);
        }
    }
}

#[cfg(test)]
mod job_tests {
    use super::*;

    fn bg_session(id: &str) -> LiveSession {
        serde_json::from_value(serde_json::json!({
            "sessionId": format!("{id}-0000-0000"), "pid": 1, "kind": "bg", "jobId": id,
            "name": "Forge auf server.nexron einrichten", "status": "shell",
        }))
        .unwrap()
    }

    #[test]
    fn background_job_at_limit_reports_limit() {
        let dir = std::env::temp_dir().join(format!("agentdashboard-test-{}", std::process::id()));
        let job = dir.join("jobs").join("89a2c8a4");
        std::fs::create_dir_all(&job).unwrap();
        std::fs::write(job.join("state.json"), r#"{"state":"working","tempo":"blocked","detail":"You've hit your limit"}"#).unwrap();

        let mut s = bg_session("89a2c8a4");
        background_job(&mut s, &dir);
        assert_eq!((s.status.as_deref(), s.note.as_deref(), s.name.as_deref()), (Some("limit"), Some("You've hit your limit"), Some("Hintergrund")));

        // Ohne Job-Datei bleibt der Status aus der Session-Datei stehen
        let mut other = bg_session("ffffffff");
        background_job(&mut other, &dir);
        assert_eq!(other.status.as_deref(), Some("shell"));
        std::fs::remove_dir_all(&dir).ok();
    }
}
