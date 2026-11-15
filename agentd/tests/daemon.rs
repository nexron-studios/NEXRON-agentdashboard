//! Drives the real agentd binary with fake-claude as Claude.
//! Run with `cargo test --features fake-claude`.
#![cfg(feature = "fake-claude")]

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

const WAIT: Duration = Duration::from_secs(10);

struct Agentd {
    child: Child,
    stdin: ChildStdin,
    events: Receiver<Value>,
}

impl Agentd {
    fn spawn() -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_agentd"))
            .arg("connect")
            .env("AGENTD_CLAUDE", env!("CARGO_BIN_EXE_fake-claude"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("agentd starts");
        let stdout = child.stdout.take().expect("piped stdout");
        let (sender, events) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                sender.send(serde_json::from_str(&line).expect("agentd writes JSON lines")).ok();
            }
        });
        let stdin = child.stdin.take().expect("piped stdin");
        Self { child, stdin, events }
    }

    fn send(&mut self, request: Value) {
        writeln!(self.stdin, "{request}").unwrap();
    }

    /// Skips events until one matches, so the order of unrelated events does not matter.
    fn expect(&self, matches: impl Fn(&Value) -> bool) -> Value {
        loop {
            let event = self.events.recv_timeout(WAIT).expect("event before timeout");
            if matches(&event) {
                return event;
            }
        }
    }
}

fn claude_event<'a>(event: &'a Value, kind: &str) -> Option<&'a Value> {
    (event["type"] == "claude" && event["event"]["type"] == kind).then(|| &event["event"])
}

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("agentd-test-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn runs_a_session_end_to_end() {
    let mut agentd = Agentd::spawn();
    let cwd = temp_dir("cwd");

    agentd.send(json!({ "id": 1, "op": "hello" }));
    assert_eq!(agentd.expect(|e| e["type"] == "hello")["version"], "0.1.0");

    agentd.send(json!({ "id": 2, "op": "start", "tab": "t1", "cwd": cwd, "profile": null, "resume": null, "model": null, "permission_mode": null }));
    let started = agentd.expect(|e| e["type"] == "started");
    assert_eq!((started["id"].as_u64(), started["tab"].as_str()), (Some(2), Some("t1")));
    assert_eq!(started["session_id"].as_str().map(str::len), Some(36));
    agentd.expect(|e| claude_event(e, "control_response").is_some());

    agentd.send(json!({ "op": "send", "tab": "t1", "content": [{ "type": "text", "text": "hallo" }] }));
    let result = agentd.expect(|e| claude_event(e, "result").is_some());
    assert_eq!(result["event"]["result"], "echo: hallo");

    agentd.send(json!({ "op": "send", "tab": "t1", "content": [{ "type": "text", "text": "please write" }] }));
    let ask = agentd.expect(|e| claude_event(e, "control_request").is_some());
    assert_eq!(ask["event"]["request"]["subtype"], "can_use_tool");
    agentd.send(json!({ "op": "permission", "tab": "t1", "request_id": "perm-1", "decision": { "behavior": "allow", "updatedInput": {} } }));
    assert_eq!(agentd.expect(|e| claude_event(e, "result").is_some())["event"]["result"], "decision: \"allow\"");

    agentd.send(json!({ "id": 3, "op": "send", "tab": "nope", "content": [] }));
    let error = agentd.expect(|e| e["type"] == "error");
    assert_eq!((error["id"].as_u64(), error["message"].as_str()), (Some(3), Some("no running tab nope")));

    agentd.send(json!({ "op": "stop", "tab": "t1" }));
    assert_eq!(agentd.expect(|e| e["type"] == "exited")["tab"], "t1");
    drop(agentd.stdin);
    assert!(agentd.child.wait().unwrap().success());
}

#[test]
fn serves_history_pages_from_a_profile() {
    let mut agentd = Agentd::spawn();
    let profile = temp_dir("profile");
    let project = profile.join("projects").join("C--work");
    std::fs::create_dir_all(&project).unwrap();
    let lines: Vec<String> = (1..=3)
        .map(|i| json!({ "type": "user", "uuid": format!("m{i}"), "message": { "role": "user", "content": format!("Nachricht {i}") } }).to_string())
        .collect();
    std::fs::write(project.join("s1.jsonl"), lines.join("\n")).unwrap();

    agentd.send(json!({ "id": 7, "op": "history", "profile": profile, "session_id": "s1", "before": null, "limit": 2 }));
    let history = agentd.expect(|e| e["type"] == "history");
    assert_eq!(history["id"], 7);
    assert_eq!(history["has_more"], true);
    assert_eq!(history["messages"][0]["uuid"], "m2");
    assert_eq!(history["messages"][1]["blocks"][0], json!({ "type": "text", "text": "Nachricht 3" }));
}
