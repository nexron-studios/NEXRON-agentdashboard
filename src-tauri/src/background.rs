//! Background work a session is still waiting for: shells, agents and forked skills started with
//! `run_in_background`. Claude Code's session file says only "idle" while they run, so without this
//! the dashboard shows "fertig" although a build or a review agent has not reported back yet.
//!
//! A launch is recognised by the tool result (`backgroundTaskId` for shells, `agentId` with
//! `async_launched` or `background` for agents and skills). It ends with a `<task-notification>`
//! that names the task id, and for shells also the tool use id. A task whose Claude process was
//! restarted or resumed never gets one; the caller drops launches older than the running process.
//!
//! Dev servers never end, so a shell whose command looks like one does not count (Jonas, 15.11.2026).

use serde_json::Value;
use std::collections::HashMap;

const TRACKED_TOOLS: [&str; 5] = ["Bash", "PowerShell", "Agent", "Task", "Skill"];
const SHELL_TOOLS: [&str; 2] = ["Bash", "PowerShell"];
/// A word that runs a project script or tool, followed by one of `SERVER_SCRIPTS`: `npm run dev`, `next dev`
const SCRIPT_RUNNERS: [&str; 9] = ["run", "pnpm", "yarn", "bun", "npx", "tauri", "next", "nuxt", "astro"];
const SERVER_SCRIPTS: [&str; 5] = ["dev", "serve", "preview", "start", "watch"];
/// Words that start a server or watcher wherever they appear
const SERVER_WORDS: [&str; 3] = ["--watch", "uvicorn", "http.server"];

fn words(command: &str) -> Vec<String> {
    command
        .split(|c: char| c.is_whitespace() || matches!(c, ';' | '&' | '|' | '"' | '\'' | '(' | ')'))
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

fn is_server_script(word: &str) -> bool {
    SERVER_SCRIPTS.contains(&word) || word.starts_with("dev:") || word.ends_with(":dev")
}

pub fn is_server_command(command: &str) -> bool {
    let words = words(command);
    words.iter().enumerate().any(|(i, word)| {
        let next = words.get(i + 1).map(String::as_str);
        let starts_script = SCRIPT_RUNNERS.contains(&word.as_str()) && next.is_some_and(is_server_script);
        // `vite` alone or with flags is the dev server; `vite build` is not
        let bare_vite = word == "vite" && next.is_none_or(|n| n.starts_with('-'));
        let follows = word == "tail" && next == Some("-f");
        starts_script || bare_vite || follows || SERVER_WORDS.contains(&word.as_str())
    })
}

#[derive(Default)]
pub struct Background {
    /// Tool uses that may start background work, with their start time
    candidates: HashMap<String, i64>,
    /// Task ids still running: the tool use that started them and when
    running: HashMap<String, (String, i64)>,
    ended_at: i64,
}

impl Background {
    pub fn tool_use(&mut self, id: &str, name: &str, input: &Value, t: i64) {
        if !TRACKED_TOOLS.contains(&name) {
            return;
        }
        let is_shell = SHELL_TOOLS.contains(&name);
        if is_shell && input["command"].as_str().is_some_and(is_server_command) {
            return;
        }
        self.candidates.insert(id.to_string(), t);
    }

    /// The tool result of `tool_use_id` arrived; `result` is the line's `toolUseResult`.
    pub fn tool_result(&mut self, tool_use_id: &str, result: &Value) {
        let Some(t) = self.candidates.remove(tool_use_id) else { return };
        let agent_in_background = result["status"] == "async_launched" || result["background"] == true;
        let task = result["backgroundTaskId"].as_str().or_else(|| result["agentId"].as_str().filter(|_| agent_in_background));
        if let Some(task) = task {
            self.running.insert(task.to_string(), (tool_use_id.to_string(), t));
        }
    }

    pub fn finished(&mut self, task_id: Option<&str>, tool_use_id: Option<&str>, t: i64) {
        let before = self.running.len();
        self.running.retain(|task, (tool, _)| Some(task.as_str()) != task_id && Some(tool.as_str()) != tool_use_id);
        if self.running.len() < before {
            self.ended_at = self.ended_at.max(t);
        }
    }

    /// When each running task was launched
    pub fn launches(&self) -> Vec<i64> {
        self.running.values().map(|(_, t)| *t).collect()
    }

    /// When the last background task reported back
    pub fn ended_at(&self) -> i64 {
        self.ended_at
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
        assert!(is_server_command("npx vite --port 5190 --strictPort"));
        assert!(is_server_command("python -m http.server 8000"));
        assert!(is_server_command("pnpm dev:phone"));
        assert!(!is_server_command("npm run tauri -- build"));
        assert!(!is_server_command("npx vite build"));
        assert!(!is_server_command("cargo test --lib"));
        assert!(!is_server_command("ssh nexron-server \"cd app && npm run build\""));
        assert!(!is_server_command("git merge preview; npm run build"));
        assert!(!is_server_command("Start-Process explorer.exe"));
    }

    #[test]
    fn counts_launches_until_their_notification() {
        let mut bg = Background::default();
        bg.tool_use("t1", "PowerShell", &json!({"command": "npm run build", "run_in_background": true}), 1);
        bg.tool_use("t2", "Agent", &json!({"run_in_background": true}), 2);
        bg.tool_use("t3", "Skill", &json!({"skill": "code-review"}), 3);
        bg.tool_use("t4", "Skill", &json!({"skill": "code-review"}), 4);
        bg.tool_result("t1", &json!({"stdout": "", "backgroundTaskId": "bjq2s7vtt"}));
        bg.tool_result("t2", &json!({"status": "async_launched", "agentId": "af6259e0d6cf70662"}));
        bg.tool_result("t3", &json!({"status": "forked", "background": true, "agentId": "aafec7054cdb0ed39"}));
        bg.tool_result("t4", &json!({"status": "forked", "agentId": "a0120ffac566f51b6"}));
        assert_eq!(bg.launches().len(), 3);
        bg.finished(Some("bjq2s7vtt"), Some("t1"), 10);
        bg.finished(Some("af6259e0d6cf70662"), None, 20);
        assert_eq!((bg.launches(), bg.ended_at()), (vec![3], 20));
    }

    #[test]
    fn a_dev_server_or_other_tool_is_not_waited_for() {
        let mut bg = Background::default();
        bg.tool_use("t1", "Bash", &json!({"command": "npm run dev", "run_in_background": true}), 1);
        bg.tool_use("t2", "Read", &json!({}), 2);
        bg.tool_result("t1", &json!({"backgroundTaskId": "b1"}));
        bg.tool_result("t2", &json!({"agentId": "x", "background": true}));
        assert!(bg.launches().is_empty());
    }
}
