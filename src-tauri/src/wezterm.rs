//! Types keys into herdr running in WezTerm (Windows), the counterpart to Ghostty's AppleScript on the Mac.
//!
//! On Windows `wezterm cli` does not find the running GUI by itself: without `WEZTERM_UNIX_SOCKET` it
//! treats the socket name as a relative path and gives up. Programs inside WezTerm inherit the
//! variable, the dashboard app does not. So this module sets it to the `gui-sock-<pid>` of every
//! running `wezterm-gui.exe`; sockets of ended instances stay on disk and are skipped.
//!
//! herdr is recognised by its window title, `{hostname}: {workspace}` (`window_title` in herdr's
//! config.toml); before herdr's first change WezTerm shows the process name. A pane that runs herdr
//! on another machine over ssh carries the same title and would get the keys as well; WezTerm's CLI
//! does not name a pane's process, so this cannot be told apart.
//!
//! Tried on 15.11.2026 with herdr 0.9.5 and WezTerm 20261112: control characters sent with
//! `send-text --no-paste` arrive as key presses, and herdr switches the machine.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

/// One write per key, as in Ghostty; herdr drops keys that arrive together
const KEY_DELAY: Duration = Duration::from_millis(80);
/// A herdr pane's title until herdr writes its own
const PROCESS_TITLES: [&str; 2] = ["herdr.exe", "herdr"];
const GUI_PROCESS: &str = "wezterm-gui.exe";
const SOCKET_PREFIX: &str = "gui-sock-";

/// A WezTerm pane running herdr, and the GUI instance it belongs to
pub struct Pane {
    exe: PathBuf,
    socket: PathBuf,
    id: u64,
}

fn wezterm_exe() -> Result<PathBuf, String> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let program_files = std::env::var_os("ProgramFiles").map(|p| PathBuf::from(p).join("WezTerm"));
    std::env::split_paths(&path)
        .chain(program_files)
        .map(|dir| dir.join("wezterm.exe"))
        .find(|p| p.is_file())
        .ok_or_else(|| "wezterm.exe nicht gefunden".to_string())
}

fn quiet(program: &Path) -> Command {
    let mut cmd = Command::new(program);
    crate::sessions::hide_console(cmd.stdout(Stdio::piped()).stderr(Stdio::piped()));
    cmd
}

fn cli(exe: &Path, socket: &Path) -> Command {
    let mut cmd = quiet(exe);
    cmd.env("WEZTERM_UNIX_SOCKET", socket).args(["cli", "--no-auto-start"]);
    cmd
}

fn running_gui_pids() -> Result<Vec<String>, String> {
    let filter = format!("IMAGENAME eq {GUI_PROCESS}");
    let out = quiet(Path::new("tasklist"))
        .args(["/FI", &filter, "/FO", "CSV", "/NH"])
        .output()
        .map_err(|e| format!("tasklist ließ sich nicht starten: {e}"))?;
    // "wezterm-gui.exe","23652","Console","1","123.456 K"; without a match tasklist prints an info line
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| line.split("\",\"").nth(1).map(str::to_string))
        .collect())
}

fn live_sockets() -> Result<Vec<PathBuf>, String> {
    let home = std::env::var_os("USERPROFILE").ok_or("USERPROFILE fehlt")?;
    let dir = PathBuf::from(home).join(".local").join("share").join("wezterm");
    Ok(running_gui_pids()?
        .into_iter()
        .map(|pid| dir.join(format!("{SOCKET_PREFIX}{pid}")))
        .filter(|socket| socket.exists())
        .collect())
}

fn is_hostname(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_'))
}

/// "user@host: ~/src" fails the hostname check, "Administrator: Windows PowerShell" the workspace check
fn is_herdr_title(title: &str, workspaces: &[String]) -> bool {
    if PROCESS_TITLES.contains(&title) {
        return true;
    }
    title.split_once(": ").is_some_and(|(host, ws)| is_hostname(host) && workspaces.iter().any(|w| w == ws))
}

fn panes_of(exe: &Path, socket: &Path, workspaces: &[String]) -> Result<Vec<Pane>, String> {
    let out = cli(exe, socket).args(["list", "--format", "json"]).output().map_err(|e| format!("wezterm ließ sich nicht starten: {e}"))?;
    if !out.status.success() {
        return Err(format!("wezterm cli list: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let panes: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).map_err(|e| format!("wezterm cli list: {e}"))?;
    Ok(panes
        .iter()
        .filter(|p| p["title"].as_str().is_some_and(|t| is_herdr_title(t, workspaces)))
        .filter_map(|p| p["pane_id"].as_u64())
        .map(|id| Pane { exe: exe.to_path_buf(), socket: socket.to_path_buf(), id })
        .collect())
}

/// Every pane in a running WezTerm that runs herdr; empty when WezTerm is not running
pub fn herdr_panes(workspaces: &[String]) -> Result<Vec<Pane>, String> {
    let sockets = live_sockets()?;
    if sockets.is_empty() {
        return Ok(Vec::new());
    }
    let exe = wezterm_exe()?;
    let mut found = Vec::new();
    let mut errors = Vec::new();
    // One broken WezTerm instance must not keep herdr in the others from following
    for socket in sockets {
        match panes_of(&exe, &socket, workspaces) {
            Ok(panes) => found.extend(panes),
            Err(e) => errors.push(e),
        }
    }
    if found.is_empty() && !errors.is_empty() {
        return Err(errors.join("; "));
    }
    Ok(found)
}

fn send_key(pane: &Pane, key: &str) -> Result<(), String> {
    let id = pane.id.to_string();
    let mut child = cli(&pane.exe, &pane.socket)
        .args(["send-text", "--no-paste", "--pane-id", &id])
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("wezterm ließ sich nicht starten: {e}"))?;
    // Through stdin rather than as an argument, so control characters arrive unchanged
    let written = child.stdin.take().map(|mut stdin| stdin.write_all(key.as_bytes()));
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if !out.status.success() || !matches!(written, Some(Ok(()))) {
        return Err(format!("WezTerm-Pane {id} hat die Tasten nicht angenommen: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(())
}

pub fn type_keys(pane: &Pane, keys: &[&str]) -> Result<(), String> {
    for key in keys {
        send_key(pane, key)?;
        std::thread::sleep(KEY_DELAY);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_herdr_by_its_window_title() {
        let workspaces = vec!["main-pc".to_string(), "nexron-server".to_string()];
        assert!(is_herdr_title("AS-PC: main-pc", &workspaces));
        assert!(is_herdr_title("NEXRON-SERVER: nexron-server", &workspaces));
        assert!(is_herdr_title("herdr.exe", &workspaces));
        assert!(!is_herdr_title("Administrator: Windows PowerShell", &workspaces));
        assert!(!is_herdr_title("jonas@as-pc: main-pc", &workspaces));
        assert!(!is_herdr_title("AS-PC: anderer-workspace", &workspaces));
        assert!(!is_herdr_title("powershell.exe", &workspaces));
        assert!(!is_herdr_title("✳ Claude Code", &workspaces));
    }

    /// Lists the herdr panes without typing anything. Needs WezTerm with herdr.
    #[test]
    #[ignore]
    fn finds_herdr_panes() {
        let panes = herdr_panes(&["main-pc".to_string()]).unwrap();
        println!("{:?}", panes.iter().map(|p| (p.id, p.socket.display().to_string())).collect::<Vec<_>>());
        assert!(!panes.is_empty());
    }
}
