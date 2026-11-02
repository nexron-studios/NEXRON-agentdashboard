# Agent Dashboard

Live-Übersicht aller Claude-Code-Sessions auf diesem Rechner, als Desktop-App (Tauri 2 + Vue 3) und als Terminal-Variante.

- **Übersicht:** alle laufenden Sessions aus allen Claude-Profilen in drei Spalten, in der Reihenfolge der herdr-Tabs (`claude-1`, `claude-2`, …), mit Status „Braucht Freigabe“, „Arbeitet“, „Aufgabe abgeschlossen“ oder „Bereit“. Jede Karte zeigt den herdr-Tab (z. B. `me › claude-3`) mit Knopf zum Hinspringen, den Account, die erste und die letzte Anfrage, den letzten Schritt und die Kontextauslastung. Darunter die zuletzt benutzten Sessions, nach Tagen gruppiert und durchsuchbar.
- **Detail:** alle eigenen Anfragen in voller Länge, Kontextfenster, Tool-Nutzung, Tokens je Kategorie und Modell, Kosten (sobald Claude Code sie ins Transkript schreibt), Subagents, Fehler und ein Zeitverlauf.

Die App liest nur. Profile sind alle Ordner `~/.claude` und `~/.claude-<name>` mit einem `projects/`-Unterordner, dazu `CLAUDE_CONFIG_DIR`. Laufende Sessions kommen aus `sessions/<pid>.json` jedes Profils (geprüft, ob der Prozess noch lebt), die Inhalte aus den Transkripten `projects/*/<session>.jsonl`. Laufende Transkripte werden einmal ganz und danach nur noch um neue Zeilen gelesen.

**herdr:** Ist [herdr](https://herdr.dev) installiert und sein Server läuft, ordnet die App jede Session ihrem Tab zu: `herdr pane process-info` nennt die PID des `claude.exe` im Pane, dieselbe PID steht in `sessions/<pid>.json`. Ohne herdr fehlt nur dieser Teil.

## Starten

```powershell
npm install
npm run app:dev      # Desktop-App im Dev-Modus (Vite auf Port 1450, HMR 1451)
npm run app:build    # Installer unter src-tauri/target/release/bundle
python tui/agentdashboard.py   # Terminal-Variante, Strg+C beendet
```

Voraussetzungen: Node 22, Rust (stable), WebView2 (bei Windows 11 dabei). herdr ist optional.

## Tests

```powershell
npm test             # Rust-Tests + Test der Terminal-Variante
npm run typecheck
# Parser gegen ein echtes Transkript messen:
$env:AGENTDASHBOARD_SAMPLE = "<pfad>.jsonl"; cargo test --release --manifest-path src-tauri/Cargo.toml -- --ignored --nocapture
```

## Aufbau

| Pfad | Inhalt |
| --- | --- |
| `src-tauri/src/sessions.rs` | Profile, laufende Sessions, Transkripte finden |
| `src-tauri/src/herdr.rs` | Zuordnung Session → herdr-Workspace und -Tab |
| `src-tauri/src/transcript.rs` | inkrementeller Parser für Transkripte |
| `src-tauri/src/lib.rs` | Tauri-Befehle `overview` und `session_detail` |
| `src/components/` | Übersicht, Session-Karte, Detailansicht, Zeitverlauf, Statusabzeichen, Clawd-Logo |
| `tui/` | Terminal-Variante (Python, ohne Abhängigkeiten) |
