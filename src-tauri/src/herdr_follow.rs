//! herdr in Ghostty folgt dem Machine-Umschalter und „Zum Chat“ auf anderen Machines.
//!
//! herdr 0.9 hat keinen Befehl, der die gewählte Machine eines offenen Clients wechselt: Socket-API
//! und CLI (`--machine`) routen nur Befehle an einen Server, die Auswahl lebt im Client. Der einzige
//! Weg ist dessen Tastatur. Der Navigator (`prefix+g`) listet alle Machines, darunter ihre Workspaces
//! und Panes, immer in derselben Reihenfolge. Ctrl+U springt 8 Zeilen hoch und bleibt oben stehen,
//! danach führen genau N-mal `j` zur Ziel-Zeile, Enter wählt sie. Ghostty tippt das per AppleScript
//! direkt in das Terminal, in dem herdr läuft; das Fenster muss dafür nicht vorne sein.
//!
//! Ausprobiert am 12.11.2026 mit herdr 0.9.3: Jeder Tastendruck braucht einen eigenen Schreibvorgang.
//! Mehrere Pfeiltasten am Stück oder Ctrl+D kamen nicht an, und Text nach `/` landete nicht in der
//! Suche. Die Suche taugt ohnehin nicht: Sie trifft auch Session-Titel wie „Bug auf EchoRidePC“.

#[cfg(target_os = "macos")]
use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::process::Command;

/// herdrs Standardtasten; eigene Belegungen in `~/.config/herdr/config.toml` brechen das
const PREFIX: &str = "\\x02"; // ctrl+b
const OPEN_NAVIGATOR: &str = "g";
const PAGE_UP: &str = "\\x15"; // ctrl+u
const DOWN: &str = "j";
const ENTER: &str = "\\r";
const PAGE_ROWS: usize = 8;
/// Pause zwischen zwei Tasten, damit herdr jede als eigenes Ereignis liest
const KEY_DELAY_S: &str = "0.08";

/// Eine Zeile im Navigator und die Zeilenzahl aller Machines davor und danach
pub struct Target {
    pub row: usize,
    pub total_rows: usize,
    /// herdr-Profil-ID der Ziel-Machine; `None` für Local
    pub profile: Option<String>,
    /// Nur die Machine wechseln: Steht herdr schon dort, wird nichts getippt
    pub machine_only: bool,
}

/// Die Tasten, die von einer beliebigen Navigator-Zeile zur Ziel-Zeile führen
fn keys(target: &Target) -> Vec<&'static str> {
    let pages_up = target.total_rows.div_ceil(PAGE_ROWS) + 1;
    let mut keys = vec![PREFIX, OPEN_NAVIGATOR];
    keys.extend(std::iter::repeat_n(PAGE_UP, pages_up));
    keys.extend(std::iter::repeat_n(DOWN, target.row));
    keys.push(ENTER);
    keys
}

/// Welche Machine der herdr-Client zuletzt gewählt hat; herdr schreibt das bei jedem Wechsel
#[cfg(target_os = "macos")]
fn selected_profile() -> Option<Option<String>> {
    let home = std::env::var_os("HOME")?;
    let path = PathBuf::from(home).join(".local/state/herdr/client/endpoint-selection.json");
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
    Some(v["selected_profile"].as_str().map(str::to_string))
}

#[cfg(target_os = "macos")]
fn osascript(script: &str, args: &[String]) -> Result<String, String> {
    let out = Command::new("osascript")
        .arg("-e")
        .arg(script)
        .args(args)
        .output()
        .map_err(|e| format!("osascript ließ sich nicht starten: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Ghostty-Terminals, in deren Vordergrund ein herdr-Client läuft. Ohne laufendes Ghostty leer:
/// `tell application` würde es sonst starten.
#[cfg(target_os = "macos")]
fn herdr_terminals() -> Result<Vec<String>, String> {
    const LIST: &str = r#"
if application "Ghostty" is not running then return ""
tell application "Ghostty"
  set out to ""
  repeat with w in windows
    repeat with t in tabs of w
      repeat with term in terminals of t
        set out to out & (id of term) & tab & (pid of term) & linefeed
      end repeat
    end repeat
  end repeat
  return out
end tell"#;
    let listing = osascript(LIST, &[])?;
    Ok(listing
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .filter(|(_, pid)| is_herdr_process(pid.trim()))
        .map(|(id, _)| id.to_string())
        .collect())
}

#[cfg(target_os = "macos")]
fn is_herdr_process(pid: &str) -> bool {
    Command::new("ps")
        .args(["-o", "comm=", "-p", pid])
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).trim().rsplit('/').next() == Some("herdr"))
}

/// Tippt die Tasten in jeden herdr-Client in Ghostty. Läuft keiner, passiert nichts.
#[cfg(target_os = "macos")]
pub fn send(target: &Target) -> Result<(), String> {
    if target.machine_only && selected_profile().as_ref() == Some(&target.profile) {
        return Ok(());
    }
    let terminals = herdr_terminals()?;
    if terminals.is_empty() {
        return Ok(());
    }
    let steps: String = keys(target).iter().map(|k| format!("\"text:{k}\", ")).collect();
    let script = format!(
        r#"on run argv
  set steps to {{{steps}""}}
  tell application "Ghostty"
    repeat with w in windows
      repeat with t in tabs of w
        repeat with term in terminals of t
          if argv contains (id of term) then
            repeat with s in steps
              if (s as text) is not "" then
                perform action (s as text) on term
                delay {KEY_DELAY_S}
              end if
            end repeat
          end if
        end repeat
      end repeat
    end repeat
  end tell
end run"#
    );
    osascript(&script, &terminals).map(|_| ())
}

#[cfg(not(target_os = "macos"))]
pub fn send(_target: &Target) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_reach_the_row_from_anywhere() {
        let target = Target { row: 9, total_rows: 17, profile: None, machine_only: true };
        let keys = keys(&target);
        assert_eq!(&keys[..2], &[PREFIX, OPEN_NAVIGATOR]);
        // 17 Zeilen: drei Seiten hoch reichen von ganz unten bis oben, eine mehr zur Sicherheit
        assert_eq!(keys.iter().filter(|k| **k == PAGE_UP).count(), 4);
        assert_eq!(keys.iter().filter(|k| **k == DOWN).count(), 9);
        assert_eq!(keys.last(), Some(&ENTER));
    }
}
