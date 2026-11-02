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
    /// Startzeit des Prozesses (Windows-FILETIME), damit eine wiederverwendete PID nicht als laufend gilt
    #[serde(skip_serializing)]
    pub proc_start: Option<String>,
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
            s.account = p.label.clone();
            s.profile_dir = p.dir.clone();
            out.push(s);
        }
    }
    out
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
fn process_alive(pid: u32, _proc_start: Option<&str>) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
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
    all.truncate(limit);
    all
}

/// Startet ein Kommandozeilenprogramm ohne Konsolenfenster und liefert stdout bei Erfolg.
pub fn run_quiet(program: &Path, args: &[&str]) -> Option<String> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000; // sonst blitzt bei jedem Refresh ein Konsolenfenster auf
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = cmd.output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}
