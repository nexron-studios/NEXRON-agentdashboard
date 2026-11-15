//! Owns the Claude processes, one per tab, and turns requests into stdin
//! writes and stdout lines into events.

use std::collections::HashMap;
use std::io::{self, BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;

use serde_json::{json, Value};

use crate::chat;
use crate::claude;
use crate::protocol::{Envelope, Event, HistoryParams, Request, StartParams};

#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    #[error("tab {0} is already running")]
    TabTaken(String),
    #[error("no running tab {0}")]
    NoTab(String),
    #[error("transcript of session {0} not found")]
    NoTranscript(String),
    #[error("home folder unknown")]
    NoHome,
    #[error(transparent)]
    Claude(#[from] claude::ClaudeError),
    #[error(transparent)]
    Io(#[from] io::Error),
}

struct Session {
    child: Child,
    stdin: Option<ChildStdin>,
}

pub struct Daemon<W: Write + Send + 'static> {
    sessions: Mutex<HashMap<String, Session>>,
    out: Mutex<W>,
    next_request: AtomicU64,
}

impl<W: Write + Send + 'static> Daemon<W> {
    pub fn new(out: W) -> Arc<Self> {
        Arc::new(Self { sessions: Mutex::new(HashMap::new()), out: Mutex::new(out), next_request: AtomicU64::new(1) })
    }

    /// Reads requests until the input closes, then ends every session.
    pub fn serve(self: &Arc<Self>, input: impl BufRead) {
        for line in input.lines() {
            let Ok(line) = line else { break };
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<Envelope>(&line) {
                Ok(envelope) => self.handle(envelope),
                Err(err) => self.emit(&Event::Error { id: None, tab: None, message: format!("bad request: {err}") }),
            }
        }
        let tabs: Vec<String> = self.sessions().keys().cloned().collect();
        for tab in tabs {
            // Already gone means it exited on its own; nothing left to close.
            let _ = self.stop(&tab);
        }
    }

    fn handle(self: &Arc<Self>, Envelope { id, request }: Envelope) {
        let tab = tab_of(&request);
        let outcome = match request {
            Request::Hello => {
                self.emit(&Event::Hello { version: env!("CARGO_PKG_VERSION"), os: std::env::consts::OS });
                Ok(())
            }
            Request::Start(params) => self.start(id, params),
            Request::Send { tab, content } => {
                self.write(&tab, &json!({ "type": "user", "message": { "role": "user", "content": content } }))
            }
            Request::Interrupt { tab } => self.control(&tab, json!({ "subtype": "interrupt" })),
            Request::Permission { tab, request_id, decision } => self.write(&tab, &json!({
                "type": "control_response",
                "response": { "subtype": "success", "request_id": request_id, "response": decision },
            })),
            Request::Stop { tab } => self.stop(&tab),
            Request::History(params) => self.history(id, params),
        };
        if let Err(err) = outcome {
            self.emit(&Event::Error { id, tab, message: err.to_string() });
        }
    }

    fn start(self: &Arc<Self>, id: Option<u64>, params: StartParams) -> Result<(), DaemonError> {
        if self.sessions().contains_key(&params.tab) {
            return Err(DaemonError::TabTaken(params.tab));
        }
        let session_id = params.resume.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let mut child = claude::command(&claude::find_claude()?, &params, &session_id).spawn()?;
        let (stdout, stderr) = (child.stdout.take(), child.stderr.take());
        let tab = params.tab.clone();
        self.sessions().insert(tab.clone(), Session { stdin: child.stdin.take(), child });
        // Lists the commands and skills and makes Claude ready for control requests.
        self.control(&tab, json!({ "subtype": "initialize" }))?;
        self.emit(&Event::Started { id, tab: tab.clone(), session_id });
        if let Some(stderr) = stderr {
            let daemon = Arc::clone(self);
            let tab = tab.clone();
            thread::spawn(move || {
                for text in BufReader::new(stderr).lines().map_while(Result::ok) {
                    daemon.emit(&Event::Stderr { tab: tab.clone(), text });
                }
            });
        }
        if let Some(stdout) = stdout {
            let daemon = Arc::clone(self);
            thread::spawn(move || daemon.pump(&tab, stdout));
        }
        Ok(())
    }

    fn pump(&self, tab: &str, stdout: impl io::Read) {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let event = serde_json::from_str(&line).unwrap_or_else(|_| json!({ "type": "raw", "line": line }));
            self.emit(&Event::Claude { tab: tab.to_string(), event });
        }
        // Taken out first so the wait below does not hold the lock for other tabs.
        let session = self.sessions().remove(tab);
        let code = session.and_then(|mut session| session.child.wait().ok()).and_then(|status| status.code());
        self.emit(&Event::Exited { tab: tab.to_string(), code });
    }

    fn control(&self, tab: &str, request: Value) -> Result<(), DaemonError> {
        let request_id = format!("agentd-{}", self.next_request.fetch_add(1, Ordering::Relaxed));
        self.write(tab, &json!({ "type": "control_request", "request_id": request_id, "request": request }))
    }

    fn write(&self, tab: &str, message: &Value) -> Result<(), DaemonError> {
        let mut sessions = self.sessions();
        let stdin = sessions
            .get_mut(tab)
            .and_then(|session| session.stdin.as_mut())
            .ok_or_else(|| DaemonError::NoTab(tab.to_string()))?;
        writeln!(stdin, "{message}")?;
        stdin.flush()?;
        Ok(())
    }

    fn stop(&self, tab: &str) -> Result<(), DaemonError> {
        let mut sessions = self.sessions();
        let session = sessions.get_mut(tab).ok_or_else(|| DaemonError::NoTab(tab.to_string()))?;
        session.stdin = None;
        Ok(())
    }

    fn history(&self, id: Option<u64>, params: HistoryParams) -> Result<(), DaemonError> {
        let path = transcript_path(params.profile, &params.session_id)?;
        let messages = chat::parse_transcript(&std::fs::read_to_string(path)?);
        let (messages, has_more) = chat::page(&messages, params.before.as_deref(), params.limit);
        self.emit(&Event::History { id, messages, has_more });
        Ok(())
    }

    fn sessions(&self) -> MutexGuard<'_, HashMap<String, Session>> {
        // A panicked writer thread leaves the map intact; keep serving.
        self.sessions.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn emit(&self, event: &Event) {
        // Events are plain data; serialising them cannot fail.
        let Ok(line) = serde_json::to_string(event) else { return };
        let mut out = self.out.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        // If the dashboard is gone there is nobody left to tell.
        let _ = writeln!(out, "{line}").and_then(|()| out.flush());
    }
}

fn tab_of(request: &Request) -> Option<String> {
    match request {
        Request::Start(StartParams { tab, .. })
        | Request::Send { tab, .. }
        | Request::Interrupt { tab }
        | Request::Permission { tab, .. }
        | Request::Stop { tab } => Some(tab.clone()),
        Request::Hello | Request::History(_) => None,
    }
}

/// `<profile>/projects/<cwd-slug>/<session-id>.jsonl`; the slug is not needed
/// because the session id is unique across projects.
fn transcript_path(profile: Option<PathBuf>, session_id: &str) -> Result<PathBuf, DaemonError> {
    let profile = match profile {
        Some(profile) => profile,
        None => home().ok_or(DaemonError::NoHome)?.join(".claude"),
    };
    let file_name = format!("{session_id}.jsonl");
    std::fs::read_dir(profile.join("projects"))?
        .filter_map(Result::ok)
        .map(|project| project.path().join(&file_name))
        .find(|path| path.is_file())
        .ok_or_else(|| DaemonError::NoTranscript(session_id.to_string()))
}

fn home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}
