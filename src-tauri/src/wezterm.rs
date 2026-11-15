//! Tasten an herdr in WezTerm schicken (Windows), das Gegenstück zu Ghosttys AppleScript auf dem Mac.
//!
//! `wezterm cli` findet die laufende Oberfläche unter Windows nicht von selbst: Ohne
//! `WEZTERM_UNIX_SOCKET` versucht es den Socket-Namen als relativen Pfad und bricht ab. Innerhalb von
//! WezTerm erbt jedes Programm die Variable, die Dashboard-App aber nicht. Deshalb probiert dieses
//! Modul jede `gui-sock-<pid>` selbst; die einer beendeten WezTerm-Instanz scheitert sofort.
//!
//! Erkannt wird herdr am Fenstertitel. herdr schreibt dort `{hostname}: {workspace}` hin
//! (`window_title` in herdrs config.toml), vor der ersten Änderung zeigt WezTerm den Programmnamen.
//! Ausprobiert am 15.11.2026 mit herdr 0.9.5 und WezTerm 20261112: Strg-Zeichen kommen über
//! `send-text --no-paste` als Tastendruck an, herdr wechselt die Machine.

use std::io::Write;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
/// Wie in Ghostty: jeder Tastendruck einzeln, sonst gehen herdr Tasten verloren
const KEY_DELAY: Duration = Duration::from_millis(80);
/// So heißt ein herdr-Pane, solange herdr noch keinen eigenen Titel geschrieben hat
const PROCESS_TITLES: [&str; 2] = ["herdr.exe", "herdr"];

/// Ein WezTerm-Pane, in dem herdr läuft, samt der Oberfläche, zu der es gehört
pub struct Pane {
    socket: PathBuf,
    id: u64,
}

fn wezterm_exe() -> PathBuf {
    let path = std::env::var_os("PATH").unwrap_or_default();
    if let Some(found) = std::env::split_paths(&path).map(|d| d.join("wezterm.exe")).find(|p| p.is_file()) {
        return found;
    }
    let program_files = std::env::var_os("ProgramFiles").unwrap_or_else(|| "C:\\Program Files".into());
    PathBuf::from(program_files).join("WezTerm").join("wezterm.exe")
}

fn cli(socket: &Path) -> Command {
    let mut cmd = Command::new(wezterm_exe());
    cmd.env("WEZTERM_UNIX_SOCKET", socket)
        .args(["cli", "--no-auto-start"])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Sockets aller WezTerm-Oberflächen, die je liefen; die von beendeten bleiben liegen.
fn gui_sockets() -> Vec<PathBuf> {
    let Some(home) = std::env::var_os("USERPROFILE") else { return Vec::new() };
    let dir = PathBuf::from(home).join(".local").join("share").join("wezterm");
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    entries
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with("gui-sock-"))
        .map(|e| e.path())
        .collect()
}

fn is_herdr_title(title: &str, workspaces: &[String]) -> bool {
    if PROCESS_TITLES.contains(&title) {
        return true;
    }
    // Rechnernamen haben keine Leerzeichen; so passt „Administrator: Windows PowerShell“ nie
    title.split_once(": ").is_some_and(|(host, ws)| !host.is_empty() && !host.contains(' ') && workspaces.iter().any(|w| w == ws))
}

/// Alle Panes in laufenden WezTerm-Oberflächen, in denen herdr läuft
pub fn herdr_panes(workspaces: &[String]) -> Vec<Pane> {
    let mut found = Vec::new();
    for socket in gui_sockets() {
        let Ok(out) = cli(&socket).args(["list", "--format", "json"]).output() else { continue };
        let Ok(panes) = serde_json::from_slice::<Vec<serde_json::Value>>(&out.stdout) else { continue };
        for pane in panes {
            let (Some(id), Some(title)) = (pane["pane_id"].as_u64(), pane["title"].as_str()) else { continue };
            if is_herdr_title(title, workspaces) {
                found.push(Pane { socket: socket.clone(), id });
            }
        }
    }
    found
}

/// Tippt die Tasten in das Pane. Über stdin statt als Argument, damit Strg-Zeichen unverändert ankommen.
pub fn type_keys(pane: &Pane, keys: &[&str]) -> Result<(), String> {
    let id = pane.id.to_string();
    for key in keys {
        let mut child = cli(&pane.socket)
            .args(["send-text", "--no-paste", "--pane-id", &id])
            .stdin(Stdio::piped())
            .spawn()
            .map_err(|e| format!("wezterm ließ sich nicht starten: {e}"))?;
        child.stdin.take().ok_or("wezterm ohne stdin")?.write_all(key.as_bytes()).map_err(|e| e.to_string())?;
        let status = child.wait().map_err(|e| e.to_string())?;
        if !status.success() {
            return Err(format!("WezTerm-Pane {id} hat die Tasten nicht angenommen"));
        }
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
        assert!(!is_herdr_title("AS-PC: anderer-workspace", &workspaces));
        assert!(!is_herdr_title("powershell.exe", &workspaces));
        assert!(!is_herdr_title("✳ Claude Code", &workspaces));
    }
}
