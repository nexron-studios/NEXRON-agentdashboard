//! Claude-Sessions auf anderen Rechnern: jede SSH-Machine, die herdr kennt (`herdr machine list`).
//!
//! Pro Machine hält ein Hintergrund-Thread eine SSH-Verbindung offen, über die `remote_sync.ps1` alle
//! 2 s schickt, was sich geändert hat. Transkripte landen als Spiegel im App-Cache (`machines/<id>/`):
//! laufende und die gerade geöffnete Session vollständig, beendete nur mit Anfang und Ende. Danach
//! liest der vorhandene Parser die Spiegel wie lokale Dateien. Bisher nur Windows-Machines.

use crate::herdr::{self, HerdrPlace};
use crate::sessions::{LiveSession, Profile};
use base64::Engine;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const SCRIPT: &str = include_str!("remote_sync.ps1");
/// Liegt in `%TEMP%` der Machine und nennt die Session, deren Detailansicht offen ist
const WANT_FILE: &str = "agentdashboard-want.txt";
/// So lange gilt die Datei; die offene Detailansicht erneuert sie vorher (`WANT_TOUCH`)
const WANT_TTL: Duration = Duration::from_secs(120);
const WANT_TOUCH: Duration = Duration::from_secs(45);
/// `herdr machine list` ändert sich selten
const MACHINES_TTL: Duration = Duration::from_secs(60);
/// Nach einem Abbruch (PC aus, WLAN weg) nicht im Sekundentakt neu verbinden
const RECONNECT_DELAY: Duration = Duration::from_secs(10);
/// Passt ein Stück nicht an den Spiegel, sofort neu verbinden: Die Machine schickt dann ab der echten Größe
const OUT_OF_SYNC: &str = "Spiegel aus dem Takt";
/// Höchstens so viele Bytes Transkript pro Takt, damit ein frischer Spiegel die Karten nicht blockiert
const BUDGET_BYTES: u64 = 4_000_000;
const RECENT_LIMIT: usize = 60;
const TAILS_PER_PASS: usize = 8;

/// Was die Übersicht von einer Machine braucht; der Sync-Thread schreibt es nach jedem Takt neu.
#[derive(Default, Clone)]
pub struct View {
    /// Noch keine Antwort und kein Fehler seit dem Start
    pub connecting: bool,
    pub connected: bool,
    pub error: Option<String>,
    pub os: String,
    pub profiles: Vec<Profile>,
    pub live: Vec<LiveSession>,
    pub recent: Vec<RemoteRecent>,
    /// Nach Session-ID, denn PIDs der Machine lassen sich hier nicht abgleichen
    pub places: HashMap<String, HerdrPlace>,
}

#[derive(Clone)]
pub struct RemoteRecent {
    pub session_id: String,
    pub account: String,
}

pub struct Remote {
    pub id: String,
    pub label: String,
    target: String,
    dir: PathBuf,
    view: Mutex<View>,
    stop: AtomicBool,
    /// Session der offenen Detailansicht und wann `WANT_FILE` zuletzt geschrieben wurde
    want: Mutex<Option<(String, Instant)>>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Frame {
    os: String,
    profiles: Option<Vec<RawProfile>>,
    live: Vec<RawLive>,
    recent: Option<Vec<RawRecent>>,
    chunks: Vec<Chunk>,
    tails: Vec<Tail>,
    herdr: Option<String>,
}

#[derive(Deserialize)]
struct RawProfile {
    label: String,
    dir: String,
    email: Option<String>,
}

#[derive(Deserialize)]
struct RawLive {
    raw: String,
    account: String,
}

#[derive(Deserialize)]
struct RawRecent {
    id: String,
    account: String,
}

#[derive(Deserialize)]
struct Chunk {
    id: String,
    offset: u64,
    data: String,
}

#[derive(Deserialize)]
struct Tail {
    id: String,
    len: u64,
    head: String,
    tail: String,
}

/// Alle Machines außer dieser; startet und beendet ihre Sync-Threads nach `herdr machine list`.
#[derive(Default)]
pub struct Machines {
    inner: Mutex<(Option<Instant>, Vec<Arc<Remote>>)>,
}

impl Machines {
    pub fn remotes(&self, cache_dir: &Path) -> Vec<Arc<Remote>> {
        let mut inner = self.inner.lock().unwrap();
        if inner.0.is_some_and(|t| t.elapsed() < MACHINES_TTL) {
            return inner.1.clone();
        }
        let saved = herdr::saved_machines();
        let mut next = Vec::new();
        for (id, label, target) in saved {
            match inner.1.iter().find(|r| r.id == id && r.target == target) {
                Some(r) => next.push(r.clone()),
                None => next.push(Remote::start(id.clone(), label, target, cache_dir.join("machines").join(&id))),
            }
        }
        for gone in inner.1.iter().filter(|r| !next.iter().any(|n| Arc::ptr_eq(n, r))) {
            gone.stop.store(true, Ordering::Relaxed);
        }
        *inner = (Some(Instant::now()), next.clone());
        next
    }
}

fn is_session_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

impl Remote {
    fn start(id: String, label: String, target: String, dir: PathBuf) -> Arc<Remote> {
        let remote = Arc::new(Remote {
            id,
            label,
            target,
            dir,
            view: Mutex::new(View { connecting: true, ..Default::default() }),
            stop: AtomicBool::new(false),
            want: Mutex::new(None),
        });
        let worker = remote.clone();
        std::thread::spawn(move || worker.run());
        remote
    }

    /// SSH-Ziel aus herdr, z. B. `echoride-win`
    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn view(&self) -> View {
        self.view.lock().unwrap().clone()
    }

    pub fn full_path(&self, session_id: &str) -> PathBuf {
        self.dir.join("full").join(format!("{session_id}.jsonl"))
    }

    pub fn tail_path(&self, session_id: &str) -> PathBuf {
        self.dir.join("tail").join(format!("{session_id}.jsonl"))
    }

    /// Die Detailansicht einer beendeten Session ist offen: Die Machine soll ihr Transkript vollständig schicken.
    pub fn want(&self, session_id: &str) {
        if !is_session_id(session_id) {
            return;
        }
        let mut want = self.want.lock().unwrap();
        if want.as_ref().is_some_and(|(id, t)| id == session_id && t.elapsed() < WANT_TOUCH) {
            return;
        }
        *want = Some((session_id.to_string(), Instant::now()));
        let path = self.full_path(session_id);
        if !path.exists() {
            // Leer anlegen, damit die Detailansicht schon öffnet und sich füllt, sobald Daten kommen
            let _ = fs::create_dir_all(path.parent().unwrap_or(&self.dir));
            let _ = fs::write(&path, b"");
        }
        let script = format!("Set-Content -Path (Join-Path $env:TEMP '{WANT_FILE}') -Value '{session_id}'");
        let target = self.target.clone();
        std::thread::spawn(move || run_ps(&target, &script));
    }

    fn wanted(&self) -> Option<String> {
        self.want.lock().unwrap().as_ref().filter(|(_, t)| t.elapsed() < WANT_TTL).map(|(id, _)| id.clone())
    }

    /// herdr-Befehl auf der Machine; stdout wie lokal
    pub fn herdr(&self, args: &[&str]) -> Option<String> {
        // Argumente sind IDs wie `w1:p2`, `/clear` oder `esc`; ein Hochkomma würde den Befehl aufbrechen
        if args.iter().any(|a| a.contains('\'')) {
            return None;
        }
        let quoted: Vec<String> = args.iter().map(|a| format!("'{a}'")).collect();
        run_ps(
            &self.target,
            &format!("$null | & (Join-Path $env:USERPROFILE '.herdr\\packages\\standalone\\current\\herdr.exe') {}", quoted.join(" ")),
        )
    }

    fn run(self: Arc<Self>) {
        while !self.stop.load(Ordering::Relaxed) {
            let error = match self.connect() {
                Ok(()) => "Verbindung beendet".to_string(),
                Err(e) if e == OUT_OF_SYNC => {
                    std::thread::sleep(Duration::from_secs(1));
                    continue;
                }
                Err(e) => e,
            };
            {
                let mut view = self.view.lock().unwrap();
                view.connecting = false;
                view.connected = false;
                view.live.clear();
                view.places.clear();
                view.error = Some(error);
            }
            std::thread::sleep(RECONNECT_DELAY);
        }
    }

    /// Eine Verbindung bis zu ihrem Ende. Bricht ab, wenn ein Stück nicht an den Spiegel passt;
    /// die nächste Verbindung startet dann mit den tatsächlichen Spiegelgrößen.
    fn connect(&self) -> Result<(), String> {
        let config = serde_json::json!({
            "offsets": self.mirror_sizes(),
            "intervalMs": 2000,
            "recentEverySeconds": 20,
            "profilesEverySeconds": 30,
            "recentLimit": RECENT_LIMIT,
            "budget": BUDGET_BYTES,
            "headBytes": crate::transcript::HEAD_BYTES,
            "tailBytes": crate::TAIL_BYTES,
            "tailsPerPass": TAILS_PER_PASS,
            "wantFile": WANT_FILE,
            "wantTtlSeconds": WANT_TTL.as_secs(),
        });
        let b64 = base64::engine::general_purpose::STANDARD;
        let script = SCRIPT.replace("__CONFIG__", &b64.encode(config.to_string()));
        let mut child = ssh(&self.target)
            .arg(powershell_command(&script))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("ssh ließ sich nicht starten: {e}"))?;
        let result = self.read_frames(&mut child);
        let _ = child.kill();
        let status = child.wait().ok();
        let mut stderr = String::new();
        if let Some(mut e) = child.stderr.take() {
            let _ = e.read_to_string(&mut stderr);
        }
        match result {
            Err(e) => Err(e),
            Ok(()) if status.is_some_and(|s| !s.success()) => Err(ssh_error(&stderr)),
            Ok(()) => Ok(()),
        }
    }

    fn read_frames(&self, child: &mut Child) -> Result<(), String> {
        let mut out = BufReader::new(child.stdout.take().ok_or("kein stdout")?);
        let mut header = String::new();
        while !self.stop.load(Ordering::Relaxed) {
            header.clear();
            if out.read_line(&mut header).map_err(|e| e.to_string())? == 0 {
                return Ok(());
            }
            let Ok(len) = header.trim().parse::<usize>() else {
                // PowerShell-Fehlermeldungen kommen als CLIXML auf stdout, nicht als Rahmen
                return Err(format!("Unerwartete Antwort: {}", header.trim().chars().take(120).collect::<String>()));
            };
            let mut buf = vec![0; len];
            out.read_exact(&mut buf).map_err(|e| e.to_string())?;
            let frame: Frame = serde_json::from_slice(&buf).map_err(|e| format!("Antwort nicht lesbar: {e}"))?;
            self.apply(frame)?;
        }
        Ok(())
    }

    fn apply(&self, frame: Frame) -> Result<(), String> {
        let b64 = base64::engine::general_purpose::STANDARD;
        for c in &frame.chunks {
            let data = b64.decode(&c.data).map_err(|e| e.to_string())?;
            self.append(&c.id, c.offset, &data)?;
        }
        for t in &frame.tails {
            let (head, tail) = (b64.decode(&t.head).unwrap_or_default(), b64.decode(&t.tail).unwrap_or_default());
            // Mit Lücke dazwischen trennt ein Zeilenumbruch die beiden angeschnittenen Zeilen
            let gap = t.len > head.len() as u64 + tail.len() as u64;
            let path = self.tail_path(&t.id);
            let _ = fs::create_dir_all(path.parent().unwrap_or(&self.dir));
            let _ = fs::write(&path, [head.as_slice(), if gap { b"\n" } else { b"" }, tail.as_slice()].concat());
        }

        let live: Vec<LiveSession> = frame
            .live
            .iter()
            .filter_map(|l| {
                let mut s: LiveSession = serde_json::from_str(&l.raw).ok()?;
                s.account = l.account.clone();
                Some(s)
            })
            .filter(|s| !s.cwd.as_deref().is_some_and(|c| c.trim_end_matches(['\\', '/']).ends_with(crate::topics::WORK_DIR_NAME)))
            .collect();

        let mut view = self.view.lock().unwrap();
        view.connecting = false;
        view.connected = true;
        view.error = None;
        view.os = frame.os;
        if let Some(p) = frame.profiles {
            view.profiles = p.into_iter().map(|p| Profile { label: p.label, dir: PathBuf::from(p.dir), email: p.email }).collect();
        }
        if let Some(r) = frame.recent {
            view.recent = r.into_iter().map(|r| RemoteRecent { session_id: r.id, account: r.account }).collect();
        }
        view.places = frame.herdr.as_deref().map(|s| herdr::places_by_session(s, &self.id)).unwrap_or_default();
        view.live = live;
        let keep_full: HashSet<String> = view.live.iter().map(|l| l.session_id.clone()).chain(self.wanted()).collect();
        let keep_tail: HashSet<String> = view.recent.iter().map(|r| r.session_id.clone()).collect();
        drop(view);
        self.prune("full", &keep_full);
        self.prune("tail", &keep_tail);
        Ok(())
    }

    fn append(&self, id: &str, offset: u64, data: &[u8]) -> Result<(), String> {
        if !is_session_id(id) {
            return Ok(());
        }
        let path = self.full_path(id);
        let _ = fs::create_dir_all(path.parent().unwrap_or(&self.dir));
        let have = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let mut file = match offset {
            0 => fs::File::create(&path),
            o if o == have => fs::OpenOptions::new().append(true).open(&path),
            _ => return Err(OUT_OF_SYNC.into()),
        }
        .map_err(|e| e.to_string())?;
        file.write_all(data).map_err(|e| e.to_string())
    }

    fn mirror_sizes(&self) -> HashMap<String, u64> {
        fs::read_dir(self.dir.join("full"))
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| Some((e.path().file_stem()?.to_str()?.to_string(), e.metadata().ok()?.len())))
            .filter(|(_, len)| *len > 0)
            .collect()
    }

    /// Spiegel, die niemand mehr braucht, damit der Cache nicht über Wochen wächst
    fn prune(&self, kind: &str, keep: &HashSet<String>) {
        for entry in fs::read_dir(self.dir.join(kind)).into_iter().flatten().flatten() {
            let path = entry.path();
            let id = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            if !keep.contains(id) {
                let _ = fs::remove_file(&path);
            }
        }
    }
}

fn ssh(target: &str) -> Command {
    let mut cmd = Command::new("ssh");
    cmd.args(["-o", "BatchMode=yes", "-o", "ConnectTimeout=5", "-o", "ServerAliveInterval=10", "-o", "ServerAliveCountMax=3"]);
    // Eine geteilte Verbindung macht Klicks wie „Zum Chat“ schnell; Windows-OpenSSH kann das nicht.
    #[cfg(not(windows))]
    if let Some(home) = std::env::var_os("HOME") {
        let path = PathBuf::from(home).join(".ssh/cm-agentdashboard-%C");
        cmd.args(["-o", "ControlMaster=auto", "-o", "ControlPersist=300", "-o"]).arg(format!("ControlPath={}", path.display()));
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.arg(target);
    cmd
}

/// Läuft unter cmd wie unter PowerShell als Standard-Shell des SSH-Servers. Kommentarzeilen fallen
/// weg, weil die Windows-Kommandozeile höchstens 32 767 Zeichen fasst.
fn powershell_command(script: &str) -> String {
    let code: String = script.lines().filter(|l| !l.trim_start().starts_with('#')).collect::<Vec<_>>().join("\n");
    let utf16: Vec<u8> = code.encode_utf16().flat_map(u16::to_le_bytes).collect();
    format!("powershell -NoProfile -NonInteractive -EncodedCommand {}", base64::engine::general_purpose::STANDARD.encode(utf16))
}

fn run_ps(target: &str, script: &str) -> Option<String> {
    let out = ssh(target).arg(powershell_command(script)).stdin(Stdio::null()).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Die letzte aussagekräftige Zeile von ssh, ohne den Hinweis auf fehlenden Post-Quanten-Schlüsseltausch
fn ssh_error(stderr: &str) -> String {
    stderr
        .lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty() && !l.starts_with("**") && !l.starts_with("#<"))
        .map(str::to_string)
        .unwrap_or_else(|| "Machine nicht erreichbar".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoded_command_fits_windows_command_line() {
        let cmd = powershell_command(&SCRIPT.replace("__CONFIG__", &"A".repeat(400)));
        assert!(cmd.len() < 32_000, "{} Zeichen", cmd.len());
    }

    #[test]
    fn only_session_ids_reach_a_remote_command() {
        assert!(is_session_id("4cdbbf4e-919a-4345-8f19-cdad43ed2f79"));
        assert!(!is_session_id("x'; Remove-Item"));
        assert!(!is_session_id(""));
    }

    /// `cargo test -- --ignored --nocapture syncs_saved_machines`: spiegelt jede herdr-Machine 20 s lang
    #[test]
    #[ignore]
    fn syncs_saved_machines() {
        let dir = std::env::temp_dir().join("agentdashboard-remote-test");
        let remotes = Machines::default().remotes(&dir);
        assert!(!remotes.is_empty(), "herdr kennt keine Machine");
        for _ in 0..10 {
            std::thread::sleep(Duration::from_secs(2));
            for r in &remotes {
                let v = r.view();
                println!("{} connected={} err={:?} live={} recent={} places={}", r.label, v.connected, v.error, v.live.len(), v.recent.len(), v.places.len());
            }
        }
        for r in &remotes {
            let v = r.view();
            r.want(&v.recent.last().expect("keine beendete Session").session_id);
            std::thread::sleep(Duration::from_secs(6));
            for l in &v.live {
                let mut t = crate::transcript::Transcript::open(r.full_path(&l.session_id));
                t.refresh().unwrap();
                let place = v.places.get(&l.session_id).map(|p| format!("{} {}", p.tab, p.status));
                println!("{} {:?} {:?} prompts={} {:?} | {}", l.session_id, l.status, place, t.stats.prompts, t.stats.title, t.stats.last_action);
            }
            for x in v.recent.iter().take(5) {
                let path = r.tail_path(&x.session_id);
                let stats = crate::transcript::Transcript::tail(&path, crate::TAIL_BYTES).unwrap_or_default();
                println!("recent {} {:?} first={:?}", x.session_id, stats.title, stats.first_prompt.chars().take(50).collect::<String>());
            }
            let wanted = &v.recent.last().unwrap().session_id;
            println!("wanted mirror bytes: {:?}", fs::metadata(r.full_path(wanted)).map(|m| m.len()));
        }
    }

    #[test]
    fn ssh_error_skips_the_post_quantum_notice() {
        let stderr = "** WARNING: connection is not using a post-quantum key exchange algorithm.\n** upgraded\nssh: connect to host x port 22: Operation timed out\n";
        assert_eq!(ssh_error(stderr), "ssh: connect to host x port 22: Operation timed out");
    }
}
