//! Stand-in for `claude -p --input-format stream-json` in the integration
//! tests: acknowledges control requests, echoes user text as an assistant
//! reply and asks for a permission when the text says "write".

use std::io::{self, BufRead, Write};

use serde_json::{json, Value};

fn main() {
    let mut out = io::stdout().lock();
    let mut say = |value: Value| {
        writeln!(out, "{value}").and_then(|()| out.flush()).expect("stdout closed");
    };
    for line in io::stdin().lock().lines().map_while(Result::ok) {
        let message: Value = serde_json::from_str(&line).expect("agentd sent invalid JSON");
        match message["type"].as_str() {
            Some("control_request") => say(json!({ "type": "control_response",
                "response": { "subtype": "success", "request_id": message["request_id"], "response": {} } })),
            Some("control_response") => say(json!({ "type": "result", "subtype": "success",
                "result": format!("decision: {}", message["response"]["response"]["behavior"]) })),
            Some("user") => {
                let text = message["message"]["content"][0]["text"].as_str().unwrap_or_default().to_string();
                if text.contains("write") {
                    say(json!({ "type": "control_request", "request_id": "perm-1",
                        "request": { "subtype": "can_use_tool", "tool_name": "Write", "input": {} } }));
                    continue;
                }
                say(json!({ "type": "assistant", "uuid": "a1",
                    "message": { "role": "assistant", "content": [{ "type": "text", "text": format!("echo: {text}") }] } }));
                say(json!({ "type": "result", "subtype": "success", "result": format!("echo: {text}") }));
            }
            _ => {}
        }
    }
}
