//! „Öffnen“ für Sessions ohne herdr: holt das Fenster nach vorne, in dem der Claude-Prozess läuft.
//! Von der PID aus geht es die Elternprozesse hoch (claude → Shell → Windows Terminal …), bis einer
//! ein sichtbares Hauptfenster hat. Den genauen Tab im Terminal kann Windows nicht auswählen.

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

#[cfg(not(windows))]
pub fn focus_process_window(_pid: u32) -> Result<(), String> {
    Err("Fenster nach vorne holen geht bisher nur unter Windows".into())
}
