//! „Öffnen“ für Sessions ohne herdr: holt das Fenster nach vorne, in dem der Claude-Prozess läuft.
//! Von der PID aus geht es die Elternprozesse hoch (claude → Shell → Windows Terminal …), bis einer
//! ein sichtbares Hauptfenster hat. Den genauen Tab im Terminal kann Windows nicht auswählen.
//! Auf dem Mac wird die App nach vorne geholt, zu der der erste Vorfahr gehört.

#[cfg(windows)]
pub fn focus_process_window(pid: u32) -> Result<(), String> {
    use std::collections::HashMap;

    #[repr(C)]
    struct ProcessEntry32W {
        size: u32,
        usage: u32,
        pid: u32,
        default_heap_id: usize,
        module_id: u32,
        threads: u32,
        parent_pid: u32,
        pri_class_base: i32,
        flags: u32,
        exe_file: [u16; 260],
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> isize;
        fn Process32FirstW(snapshot: isize, entry: *mut ProcessEntry32W) -> i32;
        fn Process32NextW(snapshot: isize, entry: *mut ProcessEntry32W) -> i32;
        fn CloseHandle(handle: isize) -> i32;
    }
    #[link(name = "user32")]
    extern "system" {
        fn EnumWindows(callback: extern "system" fn(isize, isize) -> i32, param: isize) -> i32;
        fn GetWindowThreadProcessId(hwnd: isize, pid: *mut u32) -> u32;
        fn IsWindowVisible(hwnd: isize) -> i32;
        fn GetWindow(hwnd: isize, cmd: u32) -> isize;
        fn GetWindowTextLengthW(hwnd: isize) -> i32;
        fn IsIconic(hwnd: isize) -> i32;
        fn ShowWindow(hwnd: isize, cmd: i32) -> i32;
        fn SetForegroundWindow(hwnd: isize) -> i32;
    }
    const TH32CS_SNAPPROCESS: u32 = 0x2;
    const GW_OWNER: u32 = 4;
    const SW_RESTORE: i32 = 9;

    // Alle Prozesse einmal einlesen: PID → (Eltern-PID, Programmname)
    let mut procs: HashMap<u32, (u32, String)> = HashMap::new();
    // SAFETY: Snapshot-Handle wird nur hier benutzt und am Ende geschlossen; die Struktur hat die
    // Größe, die Windows in `size` erwartet.
    unsafe {
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snap == -1 {
            return Err("Prozessliste ließ sich nicht lesen".into());
        }
        let mut e: ProcessEntry32W = std::mem::zeroed();
        e.size = std::mem::size_of::<ProcessEntry32W>() as u32;
        let mut ok = Process32FirstW(snap, &mut e);
        while ok != 0 {
            let len = e.exe_file.iter().position(|&c| c == 0).unwrap_or(e.exe_file.len());
            procs.insert(e.pid, (e.parent_pid, String::from_utf16_lossy(&e.exe_file[..len]).to_lowercase()));
            ok = Process32NextW(snap, &mut e);
        }
        CloseHandle(snap);
    }
    if !procs.contains_key(&pid) {
        return Err("Die Session läuft nicht mehr".into());
    }

    // Kette der Vorfahren, nächster zuerst. Beim Explorer aufhören: sonst springt ein Ordnerfenster auf.
    let mut chain = Vec::new();
    let mut cur = pid;
    while let Some((parent, exe)) = procs.get(&cur) {
        if exe == "explorer.exe" || chain.contains(&cur) || chain.len() > 12 {
            break;
        }
        chain.push(cur);
        cur = *parent;
    }

    struct Search<'a> {
        chain: &'a [u32],
        best: Option<(usize, isize)>,
    }
    extern "system" fn visit(hwnd: isize, param: isize) -> i32 {
        // SAFETY: `param` zeigt auf die `Search` unten, die während EnumWindows lebt.
        let search = unsafe { &mut *(param as *mut Search) };
        let mut owner_pid = 0u32;
        // SAFETY: reine Abfragen auf einem Fensterhandle, das Windows gerade übergibt.
        unsafe {
            if IsWindowVisible(hwnd) == 0 || GetWindow(hwnd, GW_OWNER) != 0 || GetWindowTextLengthW(hwnd) == 0 {
                return 1;
            }
            GetWindowThreadProcessId(hwnd, &mut owner_pid);
        }
        if let Some(rank) = search.chain.iter().position(|&p| p == owner_pid) {
            if search.best.is_none_or(|(r, _)| rank < r) {
                search.best = Some((rank, hwnd));
            }
        }
        1
    }

    let mut search = Search { chain: &chain, best: None };
    // SAFETY: Callback und Zeiger leben bis EnumWindows zurückkehrt.
    unsafe { EnumWindows(visit, &mut search as *mut Search as isize) };
    let (_, hwnd) = search.best.ok_or("Kein Fenster zu dieser Session gefunden")?;
    // SAFETY: gültiges Fensterhandle aus EnumWindows.
    unsafe {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        SetForegroundWindow(hwnd);
    }
    Ok(())
}

/// macOS: the first ancestor that lives inside an app bundle (Terminal, Ghostty, the Claude desktop
/// app …) is brought to the front with `open`. That activates the app, not the exact window: without
/// the accessibility permission macOS does not let one app raise another app's window.
#[cfg(target_os = "macos")]
pub fn focus_process_window(pid: u32) -> Result<(), String> {
    let output = std::process::Command::new("ps")
        .args(["-axo", "pid=,ppid=,comm="])
        .output()
        .map_err(|e| format!("Prozessliste ließ sich nicht lesen: {e}"))?;
    let procs = parse_process_list(&String::from_utf8_lossy(&output.stdout));
    if !procs.contains_key(&pid) {
        return Err("Die Session läuft nicht mehr".into());
    }
    let bundle = ancestor_app_bundle(pid, &procs).ok_or("Kein Programmfenster zu dieser Session gefunden")?;
    let status = std::process::Command::new("open")
        .arg(&bundle)
        .status()
        .map_err(|e| format!("{bundle} ließ sich nicht öffnen: {e}"))?;
    if !status.success() {
        return Err(format!("{bundle} ließ sich nicht öffnen"));
    }
    Ok(())
}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn focus_process_window(_pid: u32) -> Result<(), String> {
    Err("Fenster nach vorne holen geht hier nicht".into())
}

/// `ps -axo pid=,ppid=,comm=` → PID → (parent PID, executable path). The path may contain spaces.
#[cfg(any(target_os = "macos", test))]
fn parse_process_list(text: &str) -> std::collections::HashMap<u32, (u32, String)> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.trim_start().splitn(2, char::is_whitespace);
            let pid = parts.next()?.parse().ok()?;
            let mut rest = parts.next()?.trim_start().splitn(2, char::is_whitespace);
            let parent = rest.next()?.parse().ok()?;
            Some((pid, (parent, rest.next()?.trim().to_string())))
        })
        .collect()
}

/// Outermost `.app` bundle of an executable path, so a helper such as
/// `Visual Studio Code.app/Contents/Frameworks/Code Helper.app/…` still opens VS Code itself.
#[cfg(any(target_os = "macos", test))]
fn app_bundle_of(executable: &str) -> Option<&str> {
    executable.find(".app/Contents/").map(|end| &executable[..end + ".app".len()])
}

/// Starts at the parent: the desktop app ships the claude binary itself inside a `claude.app` bundle.
#[cfg(any(target_os = "macos", test))]
fn ancestor_app_bundle(pid: u32, procs: &std::collections::HashMap<u32, (u32, String)>) -> Option<String> {
    const MAX_DEPTH: usize = 12;
    let mut current = procs.get(&pid)?.0;
    for _ in 0..MAX_DEPTH {
        if current <= 1 {
            return None;
        }
        let (parent, executable) = procs.get(&current)?;
        if let Some(bundle) = app_bundle_of(executable) {
            return Some(bundle.to_string());
        }
        current = *parent;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const PS_OUTPUT: &str = "\
    1     0 /sbin/launchd
51003     1 /Applications/Claude.app/Contents/MacOS/Claude
90179 51003 /Applications/Claude.app/Contents/Helpers/disclaimer
90180 90179 /Users/me/Library/Application Support/Claude/claude-code/2.1.325/abc/claude.app/Contents/MacOS/claude
68096     1 /Applications/Visual Studio Code.app/Contents/MacOS/Electron
68132 68096 /Applications/Visual Studio Code.app/Contents/Frameworks/Code Helper.app/Contents/MacOS/Code Helper
70000 68132 /bin/zsh
70001 70000 /Users/me/.local/bin/claude
80000     1 /opt/homebrew/bin/tmux
80001 80000 /Users/me/.local/bin/claude
";

    #[test]
    fn parses_paths_with_spaces() {
        let procs = parse_process_list(PS_OUTPUT);
        assert_eq!(procs[&68132], (68096, "/Applications/Visual Studio Code.app/Contents/Frameworks/Code Helper.app/Contents/MacOS/Code Helper".to_string()));
    }

    #[test]
    fn desktop_session_opens_the_claude_app_not_its_cli_bundle() {
        let procs = parse_process_list(PS_OUTPUT);
        assert_eq!(ancestor_app_bundle(90180, &procs).as_deref(), Some("/Applications/Claude.app"));
    }

    #[test]
    fn helper_process_resolves_to_the_outer_app() {
        let procs = parse_process_list(PS_OUTPUT);
        assert_eq!(ancestor_app_bundle(70001, &procs).as_deref(), Some("/Applications/Visual Studio Code.app"));
    }

    #[test]
    fn session_without_app_ancestor_has_no_bundle() {
        let procs = parse_process_list(PS_OUTPUT);
        assert_eq!(ancestor_app_bundle(80001, &procs), None);
    }
}
