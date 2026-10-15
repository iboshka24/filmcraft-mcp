//! JSON-RPC for MCP `initialize`, `tools/list`, `tools/call`, `ping`, and two resources.

use serde_json::{Value, json};

use crate::Server;
use crate::tools::{self, tools};

const PROTOCOL: &str = "2025-06-18";
const SERVER_NAME: &str = "filmcraft-mcp";

pub fn handle_request(server: &Server, req: &Value) -> Option<Value> {
    let id = req.get("id").cloned();
    if id.is_none() {
        return None;
    }
    let method = req.get("method").and_then(Value::as_str).unwrap_or("");
    let params = req.get("params").cloned().unwrap_or_else(|| json!({}));
    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL,
            "capabilities": {"tools": {"listChanged": false}, "resources": {}},
            "serverInfo": {"name": SERVER_NAME, "version": env!("CARGO_PKG_VERSION")},
            "instructions": instructions(),
        })),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({"tools": tools().iter().map(|t| t.schema()).collect::<Vec<_>>()})),
        "tools/call" => tools::call(server, &params),
        "resources/list" => Ok(json!({"resources": [
            {"uri": "filmcraft://document", "name": "document", "mimeType": "application/json", "description": "Project tree and active sequence."},
            {"uri": "filmcraft://tools", "name": "tools", "mimeType": "text/markdown", "description": "Catalog of named MCP tools."}
        ]})),
        "resources/read" => read_resource(server, &params),
        _ => Err((-32601, format!("method not found: {method}"))),
    };
    Some(match result {
        Ok(v) => json!({"jsonrpc": "2.0", "id": id, "result": v}),
        Err((code, message)) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}),
    })
}

fn read_resource(server: &Server, params: &Value) -> Result<Value, (i64, String)> {
    let uri = params.get("uri").and_then(Value::as_str).ok_or((-32602, "resources/read needs uri".to_string()))?;
    let (text, mime) = match uri {
        "filmcraft://document" => {
            let v = server.with_session(|s| tools::state_query(s, &json!({}))).map_err(|e| (-32000, e))?;
            (serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".into()), "application/json")
        }
        "filmcraft://tools" => (tools::tools_markdown(), "text/markdown"),
        other => return Err((-32002, format!("unknown resource {other}"))),
    };
    Ok(json!({"contents": [{"uri": uri, "mimeType": mime, "text": text}]}))
}

fn instructions() -> String {
    format!(
        "FilmCraft MCP fork: {} named tools wired to the engine (projects, media, sequences, tracks, insert/overwrite/ripple/roll/slip/slide, split, trim, move, delete, transitions, titles, effects, color, audio, markers, undo/redo, frame capture, export). Time accepts seconds, frame, time (ticks; 254016000000 = 1s) or timecode. command_run reaches any engine command id.",
        tools().len()
    )
}

/// Loopback HTTP checks: Host must be local, browser Origin is refused, optional bearer token.
pub fn authorize_http(host: Option<&str>, origin: Option<&str>, authorization: Option<&str>, token: Option<&str>) -> Result<(), String> {
    let host = host.unwrap_or("").trim();
    if !host_is_loopback(host) {
        return Err(format!("refusing Host `{host}` (loopback only)"));
    }
    if let Some(origin) = origin.map(str::trim).filter(|s| !s.is_empty() && *s != "null") {
        let ok = origin.starts_with("http://127.0.0.1") || origin.starts_with("http://localhost") || origin.starts_with("http://[::1]");
        if !ok {
            return Err(format!("refusing Origin `{origin}`"));
        }
    }
    if let Some(expect) = token.filter(|t| !t.is_empty()) {
        let got = authorization.unwrap_or("").strip_prefix("Bearer ").unwrap_or("");
        if got != expect {
            return Err("unauthorized".into());
        }
    }
    Ok(())
}

fn host_is_loopback(host: &str) -> bool {
    let host = host.trim();
    if host.is_empty() {
        return false;
    }
    let name = if let Some(rest) = host.strip_prefix('[') { rest.split(']').next().unwrap_or("") } else { host.split(':').next().unwrap_or("") };
    matches!(name, "127.0.0.1" | "localhost" | "::1")
}

pub fn tool_result_text(value: &Value, is_error: bool) -> Value {
    let mut payload = value.clone();
    let image = payload.as_object_mut().and_then(|o| o.remove("pngBase64")).and_then(|v| v.as_str().map(str::to_string));
    let text = serde_json::to_string_pretty(&payload).unwrap_or_else(|_| payload.to_string());
    let mut content = vec![json!({"type": "text", "text": text})];
    if let Some(data) = image {
        content.push(json!({"type": "image", "mimeType": "image/png", "data": data}));
    }
    json!({"content": content, "isError": is_error})
}
