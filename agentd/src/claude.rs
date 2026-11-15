//! Finding the `claude` binary and building its headless command line.

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::protocol::StartParams;

/// Overrides the lookup, e.g. for tests or an unusual install.
const CLAUDE_OVERRIDE_VAR: &str = "AGENTD_CLAUDE";
const CONFIG_DIR_VAR: &str = "CLAUDE_CONFIG_DIR";

#[derive(Debug, thiserror::Error)]
pub enum ClaudeError {
    #[error("claude not found on PATH (set {CLAUDE_OVERRIDE_VAR} to its full path)")]
    NotFound,
}

pub fn find_claude() -> Result<PathBuf, ClaudeError> {
    if let Some(path) = env::var_os(CLAUDE_OVERRIDE_VAR) {
        return Ok(PathBuf::from(path));
    }
    let path = env::var_os("PATH").ok_or(ClaudeError::NotFound)?;
    env::split_paths(&path).find_map(|dir| claude_in(&dir)).ok_or(ClaudeError::NotFound)
}

/// The npm install on Windows puts a `claude.cmd` shim on PATH; the real
/// binary sits in node_modules next to it. Running the .exe directly keeps
/// `cmd` (and its quoting) out of the pipe.
fn claude_in(dir: &Path) -> Option<PathBuf> {
    let exe = dir.join(if cfg!(windows) { "claude.exe" } else { "claude" });
    if exe.is_file() {
        return Some(exe);
    }
    let npm_exe = dir.join("node_modules/@anthropic-ai/claude-code/bin/claude.exe");
    (cfg!(windows) && dir.join("claude.cmd").is_file() && npm_exe.is_file()).then_some(npm_exe)
}

pub fn args(params: &StartParams, session_id: &str) -> Vec<String> {
    let mut args: Vec<String> = [
        "-p", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose",
        "--include-partial-messages", "--permission-prompt-tool", "stdio",
    ]
    .map(String::from)
    .to_vec();
    match &params.resume {
        Some(id) => args.extend(["--resume".into(), id.clone()]),
        None => args.extend(["--session-id".into(), session_id.to_string()]),
    }
    if let Some(model) = &params.model {
        args.extend(["--model".into(), model.clone()]);
    }
    if let Some(mode) = &params.permission_mode {
        args.extend(["--permission-mode".into(), mode.clone()]);
    }
    args
}

pub fn command(program: &Path, params: &StartParams, session_id: &str) -> Command {
    let mut command = Command::new(program);
    command
        .args(args(params, session_id))
        .current_dir(&params.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // agentd may itself run inside a Claude session (or one of its tools); its
    // CLAUDE* variables would make the child report to that parent session.
    for (key, _) in env::vars_os() {
        if key.to_string_lossy().starts_with("CLAUDE") {
            command.env_remove(key);
        }
    }
    if let Some(profile) = &params.profile {
        command.env(CONFIG_DIR_VAR, profile);
    }
    hide_console(&mut command);
    command
}

#[cfg(windows)]
fn hide_console(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console(_command: &mut Command) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(resume: Option<&str>) -> StartParams {
        StartParams {
            tab: "t1".into(),
            profile: Some(PathBuf::from("/profiles/.claude-mackone")),
            cwd: PathBuf::from("."),
            resume: resume.map(String::from),
            model: Some("haiku".into()),
            permission_mode: None,
        }
    }

    #[test]
    fn new_session_gets_the_chosen_id_and_stdio_permissions() {
        let args = args(&params(None), "abc");
        assert_eq!(args.join(" "), "-p --input-format stream-json --output-format stream-json --verbose \
            --include-partial-messages --permission-prompt-tool stdio --session-id abc --model haiku");
    }

    #[test]
    fn resume_replaces_the_session_id() {
        let args = args(&params(Some("old")), "ignored");
        assert!(args.windows(2).any(|pair| pair == ["--resume", "old"]));
        assert!(!args.contains(&"--session-id".to_string()));
    }

    #[test]
    fn command_sets_profile_and_drops_parent_claude_vars() {
        env::set_var("CLAUDE_CODE_SESSION_ID", "parent");
        let command = command(Path::new("claude"), &params(None), "abc");
        let envs: Vec<_> = command.get_envs().collect();
        assert!(envs.contains(&("CLAUDE_CODE_SESSION_ID".as_ref(), None)));
        assert!(envs.contains(&(CONFIG_DIR_VAR.as_ref(), Some("/profiles/.claude-mackone".as_ref()))));
    }
}
