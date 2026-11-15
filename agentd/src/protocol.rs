//! The JSON-lines protocol between the dashboard and agentd: one request per
//! stdin line, one event per stdout line. The same stream runs locally and over
//! `ssh -T`, so it carries no paths or handles that only make sense on one side.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::chat::ChatMessage;

/// A request with an optional id, echoed back on the reply it causes.
#[derive(Debug, Deserialize)]
pub struct Envelope {
    pub id: Option<u64>,
    #[serde(flatten)]
    pub request: Request,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    Hello,
    Start(StartParams),
    /// `content` is a list of Anthropic content blocks (text, base64 image).
    Send { tab: String, content: Vec<Value> },
    Interrupt { tab: String },
    /// Answers a `can_use_tool` request; `decision` is `{behavior: allow|deny, ...}`.
    Permission { tab: String, request_id: String, decision: Value },
    /// Closes Claude's stdin: it finishes the current turn and exits.
    Stop { tab: String },
    History(HistoryParams),
}

#[derive(Debug, Clone, Deserialize)]
pub struct StartParams {
    pub tab: String,
    /// `CLAUDE_CONFIG_DIR` of the account; `None` is the default profile.
    pub profile: Option<PathBuf>,
    pub cwd: PathBuf,
    /// Continue this session instead of starting a new one.
    pub resume: Option<String>,
    pub model: Option<String>,
    pub permission_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct HistoryParams {
    pub profile: Option<PathBuf>,
    pub session_id: String,
    /// Return messages older than this uuid; `None` starts at the newest.
    pub before: Option<String>,
    pub limit: usize,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    Hello { version: &'static str, os: &'static str },
    Started { id: Option<u64>, tab: String, session_id: String },
    /// One stdout line of Claude, passed through untouched.
    Claude { tab: String, event: Value },
    Stderr { tab: String, text: String },
    Exited { tab: String, code: Option<i32> },
    History { id: Option<u64>, messages: Vec<ChatMessage>, has_more: bool },
    Error { id: Option<u64>, tab: Option<String>, message: String },
}
