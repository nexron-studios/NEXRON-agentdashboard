"""Live-Übersicht aller laufenden Claude-Code-Sessions im Terminal.

Start: python tui/agentdashboard.py   (Strg+C beendet)
"""
import glob
import json
import os
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone

REFRESH_SECONDS = 2
TAIL_BYTES = 300_000  # Transkripte werden mehrere MB groß; das Ende reicht für Titel und letzten Schritt
CONFIG_DIR = os.environ.get("CLAUDE_CONFIG_DIR") or os.path.expanduser("~/.claude")
STATUS_ORDER = {"blocked": 0, "busy": 1, "idle": 2}
STATUS_LABEL = {"blocked": "\x1b[31m● wartet auf dich\x1b[0m", "busy": "\x1b[33m● arbeitet\x1b[0m",
                "idle": "\x1b[90m○ fertig/idle\x1b[0m"}
INPUT_KEYS = ("description", "command", "file_path", "pattern", "url", "query", "prompt")


def list_sessions() -> list[dict]:
    out = subprocess.run("claude agents --json", shell=True, capture_output=True, text=True, encoding="utf-8")
    return json.loads(out.stdout or "[]")


def describe_tool(block: dict) -> str:
    params = block.get("input") or {}
    detail = next((str(params[k]) for k in INPUT_KEYS if params.get(k)), "")
    name = block["name"].split("__")[-1]
    return f"{name}: {detail}".replace("\n", " ") if detail else name


def read_transcript(lines: list[str]) -> dict[str, str]:
    """Titel, letzte Anfrage, letzter Schritt und Zeitpunkt aus den letzten Transkript-Zeilen."""
    info = {"title": "", "prompt": "", "action": "", "timestamp": ""}
    for line in reversed(lines):
        try:
            entry = json.loads(line)
        except json.JSONDecodeError:
            continue  # erste Zeile ist durch das Abschneiden meist unvollständig
        kind = entry.get("type")
        if kind == "ai-title" and not info["title"]:
            info["title"] = entry["aiTitle"]
        elif kind == "last-prompt" and not info["prompt"]:
            info["prompt"] = entry["lastPrompt"]
        elif kind == "assistant" and not info["action"]:
            content = entry["message"].get("content") or []
            tools = [b for b in content if b.get("type") == "tool_use"]
            texts = [b["text"] for b in content if b.get("type") == "text"]
            info["action"] = describe_tool(tools[-1]) if tools else ("💬 " + texts[-1] if texts else "")
        if entry.get("timestamp") and not info["timestamp"]:
            info["timestamp"] = entry["timestamp"]
    return info


def find_transcript(session_id: str) -> str | None:
    paths = glob.glob(os.path.join(CONFIG_DIR, "projects", "*", f"{session_id}.jsonl"))
    return paths[0] if paths else None


def load_transcript(path: str | None) -> dict[str, str]:
    if not path:
        return read_transcript([])
    with open(path, "rb") as f:
        f.seek(max(0, os.path.getsize(path) - TAIL_BYTES))
        return read_transcript(f.read().decode("utf-8", "ignore").splitlines())


class CacheStats:
    """Prompt-Cache-Nutzung einer Session über das ganze Transkript.

    Liest pro Refresh nur die seit dem letzten Mal angehängten Bytes. Jede API-Antwort steht
    mehrfach im Transkript (eine Zeile pro Content-Block, gleiche message.id), daher wird
    über die id dedupliziert.
    """

    def __init__(self) -> None:
        self.offset = 0
        self.seen: set[str] = set()
        self.read = self.written = self.uncached = 0
        self.last_hit: float | None = None

    def add(self, lines: list[str]) -> None:
        for line in lines:
            try:
                entry = json.loads(line)
            except json.JSONDecodeError:
                continue
            if entry.get("type") != "assistant":
                continue
            message = entry.get("message") or {}
            usage = message.get("usage")
            if not usage or message.get("id") in self.seen:
                continue
            self.seen.add(message.get("id"))
            read = usage.get("cache_read_input_tokens") or 0
            written = usage.get("cache_creation_input_tokens") or 0
            uncached = usage.get("input_tokens") or 0
            if read + written + uncached == 0:
                continue  # synthetische Einträge ohne echten API-Aufruf
            self.read += read
            self.written += written
            self.uncached += uncached
            self.last_hit = read / (read + written + uncached)

    def update(self, path: str) -> None:
        if os.path.getsize(path) < self.offset:
            self.__init__()  # Datei wurde neu geschrieben
        with open(path, "rb") as f:
            f.seek(self.offset)
            data = f.read()
        complete = data.rfind(b"\n") + 1  # unvollständige letzte Zeile beim nächsten Mal lesen
        self.offset += complete
        self.add(data[:complete].decode("utf-8", "ignore").splitlines())

    @property
    def hit_rate(self) -> float | None:
        total = self.read + self.written + self.uncached
        return self.read / total if total else None


CACHE_STATS: dict[str, CacheStats] = {}


def tokens(n: int) -> str:
    if n >= 1_000_000:
        return f"{n / 1_000_000:.1f}M"
    return f"{n / 1000:.0f}k" if n >= 1000 else str(n)


def cache_line(stats: CacheStats) -> str:
    if stats.hit_rate is None:
        return "noch keine API-Aufrufe"
    return (f"Cache {stats.hit_rate:.0%} Treffer · {tokens(stats.read)} gelesen · "
            f"{tokens(stats.written)} geschrieben · {tokens(stats.uncached)} ungecacht · "
            f"letzte Anfrage {stats.last_hit:.0%}")


def age(timestamp: str) -> str:
    if not timestamp:
        return ""
    seconds = int((datetime.now(timezone.utc) - datetime.fromisoformat(timestamp.replace("Z", "+00:00"))).total_seconds())
    if seconds < 60:
        return f"vor {seconds}s"
    return f"vor {seconds // 60}min" if seconds < 3600 else f"vor {seconds // 3600}h"


def cut(text: str, width: int) -> str:
    text = " ".join(text.split())
    return text if len(text) <= width else text[: width - 1] + "…"


def render() -> None:
    width = shutil.get_terminal_size().columns
    sessions = list_sessions()
    for s in sessions:
        s["status"] = s.get("status") or s.get("state") or "idle"
    sessions.sort(key=lambda s: (STATUS_ORDER.get(s["status"], 3), -s["startedAt"]))

    rows = [f"\x1b[1mAgent Dashboard\x1b[0m  {len(sessions)} aktiv · {time.strftime('%H:%M:%S')}", ""]
    for s in sessions:
        path = find_transcript(s["sessionId"])
        t = load_transcript(path)
        stats = CACHE_STATS.setdefault(s["sessionId"], CacheStats())
        if path:
            stats.update(path)
        title = t["title"] or s.get("name") or s["sessionId"][:8]
        rows.append(f"{STATUS_LABEL.get(s['status'], s['status'])}  \x1b[1m{cut(title, width - 40)}\x1b[0m"
                    f"  \x1b[90m{s.get('name', '')} · {age(t['timestamp'])}\x1b[0m")
        rows.append(f"   📁 {cut(s['cwd'], width - 6)}")
        rows.append(f"   ❯ {cut(t['prompt'], width - 6)}")
        rows.append(f"   ⚙ {cut(t['action'], width - 6)}")
        rows.append(f"   ⚡ {cut(cache_line(stats), width - 6)}")
        rows.append("")
    sys.stdout.write("\x1b[H\x1b[2J" + "\n".join(rows))
    sys.stdout.flush()


if __name__ == "__main__":
    os.system("")  # schaltet ANSI-Farben in der Windows-Konsole ein
    sys.stdout.reconfigure(encoding="utf-8")
    try:
        while True:
            render()
            time.sleep(REFRESH_SECONDS)
    except KeyboardInterrupt:
        pass
