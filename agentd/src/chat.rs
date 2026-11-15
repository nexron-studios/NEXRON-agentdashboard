//! Turns transcript lines and live stream-json events into chat messages.
//! Both carry the same `message` shape, so one function serves history and
//! live view alike.

use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    User,
    Assistant,
    /// Markers like "conversation compacted", not spoken by anyone.
    System,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    Text { text: String },
    Image { media_type: String, data: String },
    ToolUse { id: String, name: String, input: Value },
    ToolResult { tool_use_id: String, is_error: bool, content: Vec<Block> },
    Command { name: String, args: String },
    CommandOutput { text: String },
    Compacted,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChatMessage {
    pub uuid: String,
    pub role: Role,
    pub at: Option<String>,
    pub blocks: Vec<Block>,
}

/// One transcript line or stream-json event; `None` for everything that is not
/// part of the visible conversation (attachments, meta, subagents, bookkeeping).
pub fn message_from(line: &Value) -> Option<ChatMessage> {
    let uuid = line.get("uuid")?.as_str()?.to_string();
    let at = line.get("timestamp").and_then(Value::as_str).map(str::to_string);
    let kind = line.get("type")?.as_str()?;
    if kind == "system" && line.get("subtype").and_then(Value::as_str) == Some("compact_boundary") {
        return Some(ChatMessage { uuid, role: Role::System, at, blocks: vec![Block::Compacted] });
    }
    let role = match kind {
        "user" => Role::User,
        "assistant" => Role::Assistant,
        _ => return None,
    };
    if ["isMeta", "isSidechain", "isCompactSummary"].iter().any(|flag| is_true(line, flag)) {
        return None;
    }
    let blocks = blocks_of(line.get("message")?.get("content")?);
    if blocks.is_empty() {
        return None;
    }
    Some(ChatMessage { uuid, role, at, blocks })
}

pub fn parse_transcript(text: &str) -> Vec<ChatMessage> {
    text.lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter_map(|line| message_from(&line))
        .collect()
}

/// The `limit` messages just before `before` (or the newest ones), oldest first,
/// and whether older ones remain.
pub fn page(messages: &[ChatMessage], before: Option<&str>, limit: usize) -> (Vec<ChatMessage>, bool) {
    let end = before
        .and_then(|uuid| messages.iter().position(|m| m.uuid == uuid))
        .unwrap_or(messages.len());
    let start = end.saturating_sub(limit);
    (messages[start..end].to_vec(), start > 0)
}

fn is_true(line: &Value, flag: &str) -> bool {
    line.get(flag).and_then(Value::as_bool).unwrap_or(false)
}

fn blocks_of(content: &Value) -> Vec<Block> {
    match content {
        Value::String(text) => text_block(text).into_iter().collect(),
        Value::Array(items) => items.iter().filter_map(block_of).collect(),
        _ => Vec::new(),
    }
}

fn block_of(item: &Value) -> Option<Block> {
    let field = |name: &str| item.get(name).and_then(Value::as_str).unwrap_or_default().to_string();
    match item.get("type")?.as_str()? {
        "text" => text_block(item.get("text")?.as_str()?),
        "image" => {
            let source = item.get("source")?;
            if source.get("type")?.as_str()? != "base64" {
                return None;
            }
            Some(Block::Image {
                media_type: source.get("media_type")?.as_str()?.to_string(),
                data: source.get("data")?.as_str()?.to_string(),
            })
        }
        "tool_use" => Some(Block::ToolUse {
            id: field("id"),
            name: field("name"),
            input: item.get("input").cloned().unwrap_or(Value::Null),
        }),
        "tool_result" => Some(Block::ToolResult {
            tool_use_id: field("tool_use_id"),
            is_error: item.get("is_error").and_then(Value::as_bool).unwrap_or(false),
            content: item.get("content").map(blocks_of).unwrap_or_default(),
        }),
        // Thinking blocks are stored empty (only the signature), so they show nothing.
        _ => None,
    }
}

/// Claude Code wraps slash commands and injected context in pseudo-XML tags.
/// Commands become their own block; other tagged text is context, not chat.
fn text_block(text: &str) -> Option<Block> {
    if let Some(name) = between(text, "<command-name>", "</command-name>") {
        let args = between(text, "<command-args>", "</command-args>").unwrap_or_default();
        return Some(Block::Command { name: name.trim().to_string(), args: args.trim().to_string() });
    }
    if let Some(output) = between(text, "<local-command-stdout>", "</local-command-stdout>") {
        return Some(Block::CommandOutput { text: output.trim().to_string() });
    }
    if text.trim_start().starts_with('<') || text.trim().is_empty() {
        return None;
    }
    Some(Block::Text { text: text.to_string() })
}

fn between<'a>(text: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = text.find(open)? + open.len();
    let end = start + text[start..].find(close)?;
    Some(&text[start..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn user(uuid: &str, content: Value) -> Value {
        json!({ "type": "user", "uuid": uuid, "timestamp": "2026-11-15T01:00:00Z",
                "message": { "role": "user", "content": content } })
    }

    #[test]
    fn keeps_full_text_and_images_of_a_user_message() {
        let line = user("u1", json!([
            { "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": "iVBOR" } },
            { "type": "text", "text": "Welche Farbe?\nZweite Zeile" }
        ]));
        let message = message_from(&line).unwrap();
        assert_eq!(message.role, Role::User);
        assert_eq!(message.blocks, vec![
            Block::Image { media_type: "image/png".into(), data: "iVBOR".into() },
            Block::Text { text: "Welche Farbe?\nZweite Zeile".into() },
        ]);
    }

    #[test]
    fn reads_tool_use_and_nested_tool_result() {
        let call = json!({ "type": "assistant", "uuid": "a1", "message": { "role": "assistant", "content": [
            { "type": "thinking", "thinking": "" },
            { "type": "tool_use", "id": "t1", "name": "Bash", "input": { "command": "echo hi" } }
        ]}});
        let result = user("u2", json!([{ "type": "tool_result", "tool_use_id": "t1", "is_error": false,
            "content": [{ "type": "text", "text": "hi" }] }]));
        assert_eq!(message_from(&call).unwrap().blocks, vec![
            Block::ToolUse { id: "t1".into(), name: "Bash".into(), input: json!({ "command": "echo hi" }) },
        ]);
        assert_eq!(message_from(&result).unwrap().blocks, vec![Block::ToolResult {
            tool_use_id: "t1".into(), is_error: false, content: vec![Block::Text { text: "hi".into() }],
        }]);
    }

    #[test]
    fn turns_slash_commands_into_command_blocks() {
        let command = user("u3", json!("<command-name>/compact</command-name>\n<command-message>compact</command-message>\n<command-args></command-args>"));
        let output = user("u4", json!("<local-command-stdout>Compacted </local-command-stdout>"));
        assert_eq!(message_from(&command).unwrap().blocks, vec![Block::Command { name: "/compact".into(), args: "".into() }]);
        assert_eq!(message_from(&output).unwrap().blocks, vec![Block::CommandOutput { text: "Compacted".into() }]);
    }

    #[test]
    fn skips_meta_sidechain_summaries_and_bookkeeping() {
        let mut meta = user("u5", json!("[Image: source: C:\\x.png]"));
        meta["isMeta"] = json!(true);
        let mut sidechain = user("u6", json!("subagent prompt"));
        sidechain["isSidechain"] = json!(true);
        let caveat = user("u7", json!("<local-command-caveat>The command below</local-command-caveat>"));
        let attachment = json!({ "type": "attachment", "uuid": "x1" });
        for line in [meta, sidechain, caveat, attachment] {
            assert_eq!(message_from(&line), None);
        }
    }

    #[test]
    fn marks_compaction_as_system_message() {
        let line = json!({ "type": "system", "subtype": "compact_boundary", "uuid": "s1" });
        assert_eq!(message_from(&line).unwrap(), ChatMessage {
            uuid: "s1".into(), role: Role::System, at: None, blocks: vec![Block::Compacted],
        });
    }

    #[test]
    fn pages_backwards_from_a_uuid() {
        let text = (1..=5).map(|i| user(&format!("m{i}"), json!(format!("Nachricht {i}"))).to_string())
            .collect::<Vec<_>>().join("\n");
        let messages = parse_transcript(&text);
        let uuids = |page: &[ChatMessage]| page.iter().map(|m| m.uuid.clone()).collect::<Vec<_>>();

        let (newest, has_more) = page(&messages, None, 2);
        assert_eq!((uuids(&newest), has_more), (vec!["m4".to_string(), "m5".into()], true));
        let (older, has_more) = page(&messages, Some("m4"), 5);
        assert_eq!((uuids(&older), has_more), (vec!["m1".to_string(), "m2".into(), "m3".into()], false));
    }
}
