//! Background work a session is still waiting for: shells, agents and forked skills started with
//! `run_in_background`. Claude Code's session file says only "idle" while they run, so without this
//! the dashboard shows "fertig" although a build or a review agent has not reported back yet.
//!
//! A launch is recognised by the tool result (`backgroundTaskId` for shells, `agentId` with
//! `async_launched` or `background` for agents and skills). It ends with a `<task-notification>`
//! that names the task id, and for shells also the tool use id.
//!
//! Dev servers never end, so a shell whose command looks like one does not count (Jonas, 15.11.2026).

use serde_json::Value;
use std::collections::{HashMap, HashSet};

/// Lower-case fragments of commands that start a long-running server or watcher
const SERVER_HINTS: [&str; 16] = [
    "run dev", "pnpm dev", "yarn dev", "bun dev", "tauri dev", ":dev", "npm start", "run start", "serve", "preview",
    "--watch", "watch ", "uvicorn", "http.server", "compose up", "tail -f",
];
const SHELL_TOOLS: [&str; 2] = ["Bash", "PowerShell"];

pub fn is_server_command(command: &str) -> bool {
    let command = command.to_lowercase();
    SERVER_HINTS.iter().any(|hint| command.contains(hint))
}

#[derive(Default)]
pub struct Background {
    /// Tool uses that started a server; their launch is ignored
    servers: HashSet<String>,
    /// Task ids still running, with the tool use that started them
    running: HashMap<String, String>,
}

impl Background {
    pub fn tool_use(&mut self, id: &str, name: &str, input: &Value) {
        let is_shell = SHELL_TOOLS.contains(&name);
        if is_shell && input["command"].as_str().is_some_and(is_server_command) {
            self.servers.insert(id.to_string());
        }
    }

    /// The tool result of `tool_use_id` arrived; `result` is the line's `toolUseResult`.
    pub fn tool_result(&mut self, tool_use_id: &str, result: &Value) {
        if self.servers.contains(tool_use_id) {
            return;
        }
        let agent_in_background = result["status"] == "async_launched" || result["background"] == true;
        let task = result["backgroundTaskId"].as_str().or_else(|| result["agentId"].as_str().filter(|_| agent_in_background));
        if let Some(task) = task {
            self.running.insert(task.to_string(), tool_use_id.to_string());
        }
    }

    pub fn finished(&mut self, task_id: Option<&str>, tool_use_id: Option<&str>) {
        self.running.retain(|task, tool| Some(task.as_str()) != task_id && Some(tool.as_str()) != tool_use_id);
    }

    pub fn count(&self) -> u32 {
        self.running.len() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn recognises_dev_servers_but_not_builds() {
        assert!(is_server_command("cd frontend && npm run dev -- --port 5181"));
        assert!(is_server_command("npm run tauri dev"));
        assert!(is_server_command("python -m http.server 8000"));
        assert!(!is_server_command("npm run tauri -- build"));
        assert!(!is_server_command("cargo test --lib"));
        assert!(!is_server_command("Start-Process explorer.exe"));
    }

    #[test]
    fn counts_launches_until_their_notification() {
        let mut bg = Background::default();
        bg.tool_use("t1", "PowerShell", &json!({"command": "npm run build", "run_in_background": true}));
        bg.tool_result("t1", &json!({"stdout": "", "backgroundTaskId": "bjq2s7vtt"}));
        bg.tool_result("t2", &json!({"status": "async_launched", "agentId": "af6259e0d6cf70662"}));
        bg.tool_result("t3", &json!({"status": "forked", "background": true, "agentId": "aafec7054cdb0ed39"}));
        bg.tool_result("t4", &json!({"status": "forked", "agentId": "a0120ffac566f51b6"}));
        assert_eq!(bg.count(), 3);
        bg.finished(Some("bjq2s7vtt"), Some("t1"));
        bg.finished(Some("af6259e0d6cf70662"), None);
        assert_eq!(bg.count(), 1);
    }

    #[test]
    fn a_dev_server_is_not_waited_for() {
        let mut bg = Background::default();
        bg.tool_use("t1", "Bash", &json!({"command": "npm run dev", "run_in_background": true}));
        bg.tool_result("t1", &json!({"backgroundTaskId": "b1"}));
        assert_eq!(bg.count(), 0);
    }
}
