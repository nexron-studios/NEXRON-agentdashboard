import json

from agentdashboard import CacheStats, cache_line, read_transcript

lines = [
    '{"truncated',
    json.dumps({"type": "assistant", "timestamp": "2026-11-02T10:00:00Z", "message": {"content": [
        {"type": "tool_use", "name": "Bash", "input": {"command": "npm test", "description": "Run tests"}}]}}),
    json.dumps({"type": "last-prompt", "lastPrompt": "fix den Login"}),
    json.dumps({"type": "ai-title", "aiTitle": "Login-Bug beheben"}),
]
info = read_transcript(lines)
assert info == {"title": "Login-Bug beheben", "prompt": "fix den Login",
                "action": "Bash: Run tests", "timestamp": "2026-11-02T10:00:00Z"}, info

def usage_line(msg_id: str, read: int, written: int, uncached: int) -> str:
    return json.dumps({"type": "assistant", "message": {"id": msg_id, "usage": {
        "cache_read_input_tokens": read, "cache_creation_input_tokens": written, "input_tokens": uncached}}})


stats = CacheStats()
stats.add([usage_line("a", 0, 900, 100), usage_line("a", 0, 900, 100),  # gleiche Antwort, zweiter Content-Block
           usage_line("b", 950, 40, 10), '{"truncated'])
assert (stats.read, stats.written, stats.uncached) == (950, 940, 110), vars(stats)
assert stats.last_hit == 0.95 and round(stats.hit_rate, 3) == 0.475, vars(stats)
assert cache_line(CacheStats()) == "noch keine API-Aufrufe"
print("ok")
