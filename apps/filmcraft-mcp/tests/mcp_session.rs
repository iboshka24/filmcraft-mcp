//! MCP dispatcher tests against a real FilmCraft session.

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use filmcraft_mcp::{Server, handle_request, tools};

fn rpc(server: &Server, id: i64, method: &str, params: Value) -> Value {
    handle_request(server, &json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})).expect("response")
}

fn call(server: &Server, id: i64, name: &str, args: Value) -> Value {
    rpc(server, id, "tools/call", json!({"name": name, "arguments": args}))
}

fn text(response: &Value) -> Value {
    assert_eq!(response["error"], Value::Null, "{response}");
    assert_eq!(response["result"]["isError"], json!(false), "{response}");
    let raw = response["result"]["content"][0]["text"].as_str().expect("text");
    serde_json::from_str(raw).unwrap_or(Value::String(raw.to_string()))
}

fn must(server: &Server, id: i64, name: &str, args: Value) -> Value {
    let response = call(server, id, name, args);
    text(&response)
}

#[test]
fn catalog_is_large_and_documented() {
    let tools = tools();
    assert!(tools.len() >= 60, "tool count {}", tools.len());
    let names: Vec<&str> = tools.iter().map(|t| t.name).collect();
    for want in [
        "project_new",
        "media_import",
        "media_probe",
        "sequence_new",
        "tracks_add",
        "edit_insert",
        "edit_overwrite",
        "edit_ripple_delete",
        "edit_roll",
        "edit_slip",
        "edit_slide",
        "edit_split",
        "edit_trim",
        "edit_move",
        "edit_delete",
        "transition_video",
        "title_add",
        "media_import_demo",
        "keyframe_set",
        "effect_apply",
        "color_grade",
        "audio_gain",
        "audio_fade",
        "marker_add",
        "undo",
        "redo",
        "capture_frame",
        "export_media",
        "state_query",
    ] {
        assert!(names.contains(&want), "missing {want}");
    }
    for tool in tools {
        assert!(!tool.description.is_empty(), "{}", tool.name);
        assert!(!tool.title.is_empty(), "{}", tool.name);
        let schema = tool.schema();
        for hint in ["readOnlyHint", "destructiveHint", "idempotentHint", "openWorldHint"] {
            assert!(schema["annotations"][hint].is_boolean(), "{} {hint}", tool.name);
        }
    }
    let doc = filmcraft_mcp::tools_markdown();
    for tool in tools {
        assert!(doc.contains(&format!("`{}`", tool.name)), "{}", tool.name);
    }
}

#[test]
fn strict_arguments_and_initialize() {
    let server = Server::empty();
    let init = rpc(&server, 1, "initialize", json!({}));
    assert_eq!(init["result"]["serverInfo"]["name"], "filmcraft-mcp");
    assert!(init["result"]["instructions"].as_str().unwrap_or("").contains("named tools"));
    let listed = rpc(&server, 2, "tools/list", json!({}));
    assert_eq!(listed["result"]["tools"].as_array().map(Vec::len), Some(tools().len()));
    let bad = call(&server, 3, "edit_split", json!({"nope": 1}));
    assert_eq!(bad["error"]["code"], -32602, "{bad}");
    assert!(bad["error"]["message"].as_str().unwrap_or("").contains("nope"));
    let missing = call(&server, 4, "not_a_tool", json!({}));
    assert_eq!(missing["error"]["code"], -32602, "{missing}");
    let ping = rpc(&server, 5, "ping", json!({}));
    assert!(ping["result"].is_object(), "{ping}");
}

#[test]
fn edit_tools_drive_the_engine() {
    let server = Server::empty();
    must(&server, 1, "sequence_new", json!({"name": "Demo", "width": 320, "height": 180, "fps": 24, "video": 2, "audio": 2}));
    let red = must(&server, 2, "generate_color_matte", json!({"color": "#c0392b", "seconds": 2, "name": "Red"}));
    let green = must(&server, 3, "generate_color_matte", json!({"color": "#27ae60", "seconds": 2, "name": "Green"}));
    let blue = must(&server, 4, "generate_color_matte", json!({"color": "#2980b9", "seconds": 2, "name": "Blue"}));
    let bars = must(&server, 5, "generate_bars_and_tone", json!({"seconds": 4, "name": "Bars", "width": 320, "height": 180}));
    let red_id = red["item"].as_u64().expect("red");
    let green_id = green["item"].as_u64().expect("green");
    let blue_id = blue["item"].as_u64().expect("blue");
    let bars_id = bars["item"].as_u64().expect("bars");

    let a = must(&server, 6, "edit_overwrite", json!({"item": red_id, "track": "V1", "seconds": 0, "durationSeconds": 1, "video": true, "audio": false}));
    let b = must(&server, 7, "edit_overwrite", json!({"item": green_id, "track": "V1", "seconds": 1, "durationSeconds": 1, "video": true, "audio": false}));
    let c = must(&server, 8, "edit_overwrite", json!({"item": blue_id, "track": "V1", "seconds": 2, "durationSeconds": 1, "video": true, "audio": false}));
    let tone = must(&server, 9, "edit_overwrite", json!({"item": bars_id, "track": "V2", "audioTrack": "A1", "seconds": 0, "durationSeconds": 2}));
    let left = a["clips"][0].as_u64().expect("left clip");
    let mid = b["clips"][0].as_u64().expect("mid clip");
    let right = c["clips"][0].as_u64().expect("right clip");
    let tone_clips = tone["clips"].as_array().cloned().unwrap_or_default();
    assert!(!tone_clips.is_empty(), "{tone}");

    must(&server, 10, "edit_roll", json!({"left": left, "right": mid, "deltaFrames": 2}));
    must(&server, 11, "edit_slide", json!({"clip": mid, "deltaFrames": 1}));
    must(&server, 12, "edit_split", json!({"seconds": 0.4, "clip": left}));
    let after_split = must(&server, 13, "sequence_inspect", json!({}));
    let v1 = after_split["video"][0]["items"].as_array().map(Vec::len).unwrap_or(0);
    assert!(v1 >= 4, "razor should add a clip on V1: {after_split}");
    let slipped = tone_clips.iter().find_map(Value::as_u64).expect("tone clip");
    must(&server, 14, "edit_slip", json!({"clip": slipped, "deltaFrames": 2}));
    must(&server, 15, "edit_trim", json!({"clip": right, "edge": "out", "mode": "regular", "deltaFrames": -2}));

    must(&server, 16, "edit_move", json!({"moves": [{"clip": right, "track": "V2", "seconds": 2.2}], "linked": false}));

    must(&server, 17, "edit_insert", json!({"item": blue_id, "track": "V1", "seconds": 0.0, "durationSeconds": 0.5, "video": true, "audio": false}));
    must(&server, 18, "transition_video", json!({"clip": mid, "effect": "cross_dissolve", "frames": 4, "edge": "in"}));
    must(&server, 19, "title_add", json!({"text": "FilmCraft MCP", "seconds": 2, "position": [40, 40], "size": 28}));
    must(&server, 20, "effect_apply", json!({"effect": "gaussian_blur", "clips": [mid]}));
    must(&server, 21, "effect_set_param", json!({"clip": mid, "effect": "gaussian_blur", "param": "blurriness", "value": 8}));
    must(&server, 22, "color_grade", json!({"clips": [left], "exposure": 0.4, "contrast": 12, "saturation": 130, "temperature": 8}));
    must(&server, 23, "marker_add", json!({"seconds": 1.0, "name": "Cut", "comment": "demo", "color": "Rose"}));
    must(&server, 24, "mark_in", json!({"seconds": 0.25}));
    must(&server, 25, "mark_out", json!({"seconds": 1.25}));

    let inspected = must(&server, 26, "sequence_inspect", json!({}));
    let audio_clip = inspected["audio"]
        .as_array()
        .and_then(|tracks| tracks.iter().find_map(|t| t["items"].as_array().and_then(|items| items.first()).and_then(|i| i["clip"].as_u64())))
        .expect("an audio clip");
    must(&server, 27, "audio_gain", json!({"clips": [audio_clip], "mode": "set", "db": -6}));
    must(&server, 28, "audio_fade", json!({"clip": audio_clip, "edge": "in", "frames": 6, "effect": "exponential_fade"}));
    must(&server, 29, "audio_peak", json!({"clips": [audio_clip]}));

    let undone = must(&server, 30, "undo", json!({}));
    assert!(undone.get("undone").is_some() || undone.as_object().is_some(), "{undone}");
    must(&server, 31, "redo", json!({}));

    let effects = must(&server, 32, "effect_list", json!({"query": "lumetri"}));
    assert!(effects["count"].as_u64().unwrap_or(0) >= 1, "{effects}");

    let dir = std::env::temp_dir().join(format!("filmcraft-mcp-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let png = dir.join("frame.png");
    let shot = must(&server, 33, "capture_frame", json!({"seconds": 0.5, "path": png.to_string_lossy(), "maxSide": 160}));
    assert!(png.is_file(), "frame written");
    let bytes = std::fs::read(&png).unwrap();
    assert!(bytes.starts_with(b"\x89PNG"), "png magic");
    assert!(shot["pngBase64"].is_null(), "base64 is an image content block, not the text payload");
    let response = call(&server, 32, "capture_frame", json!({"seconds": 0.8, "maxSide": 64}));
    assert_eq!(response["result"]["content"][1]["type"], "image", "{response}");

    let wav = dir.join("tone.wav");
    must(
        &server,
        34,
        "export_media",
        json!({"path": wav.to_string_lossy(), "format": "wav", "range": "custom", "startSeconds": 0, "endSeconds": 0.4, "wait": true}),
    );
    assert!(wav.is_file() && std::fs::metadata(&wav).unwrap().len() > 44, "wav");
    let probed = must(&server, 35, "media_probe", json!({"path": wav.to_string_lossy()}));
    assert!(probed.is_object(), "{probed}");

    let state = must(&server, 36, "state_query", json!({}));
    assert!(state["project"].is_object() && state["sequence"].is_object(), "{state}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn keyframe_set_moves_a_title() {
    let server = Server::empty();
    must(&server, 1, "sequence_new", json!({"name": "Move", "width": 320, "height": 180, "fps": 24, "video": 1, "audio": 0}));
    let title = must(&server, 2, "title_add", json!({"text": "MOVE", "seconds": 2, "position": [20, 40], "size": 36}));
    let clip = title["clip"].clone();
    let first = must(
        &server,
        3,
        "keyframe_set",
        json!({"clip": clip, "effect": "graphic_text", "param": "position", "value": [16, 40], "seconds": 0.0, "ease": "easeOut"}),
    );
    assert_eq!(first["keyframes"], 1, "{first}");
    let second = must(
        &server,
        4,
        "keyframe_set",
        json!({"clip": clip, "effect": "graphic_text", "param": "position", "value": [240, 120], "seconds": 1.5, "ease": "easeIn"}),
    );
    assert_eq!(second["keyframes"], 2, "{second}");
    let end =
        must(&server, 7, "keyframe_set", json!({"clip": clip, "effect": "graphic_text", "param": "scale", "value": 140, "seconds": 2.0, "ease": "easeIn"}));
    assert_eq!(end["keyframes"], 1, "{end}");
    let early = call(&server, 5, "capture_frame", json!({"seconds": 0.05, "maxSide": 160}));
    let late = call(&server, 6, "capture_frame", json!({"seconds": 1.4, "maxSide": 160}));
    assert_eq!(early["result"]["isError"], json!(false), "{early}");
    assert_eq!(late["result"]["isError"], json!(false), "{late}");
    let a = early["result"]["content"][1]["data"].as_str().unwrap();
    let b = late["result"]["content"][1]["data"].as_str().unwrap();
    assert_ne!(a, b, "the title should be in a different place");
}

#[test]
fn stdio_roundtrip() {
    let bin = env!("CARGO_BIN_EXE_filmcraft-mcp");
    let mut child = Command::new(bin).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().expect("spawn");
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut lines = BufReader::new(stdout).lines();
    writeln!(stdin, "{}", json!({"jsonrpc":"2.0","id":1,"method":"tools/list"})).unwrap();
    let line = lines.next().unwrap().unwrap();
    let v: Value = serde_json::from_str(&line).unwrap();
    assert!(v["result"]["tools"].as_array().map(Vec::len).unwrap_or(0) >= 60, "{v}");
    writeln!(stdin, "{}", json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"sequence_new","arguments":{"name":"Stdio","width":160,"height":90,"fps":24,"video":1,"audio":1}}})).unwrap();
    let line = lines.next().unwrap().unwrap();
    let v: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(v["result"]["isError"], json!(false), "{v}");
    drop(stdin);
    let _ = child.wait();
}

#[tokio::test]
async fn http_loopback_edits_and_rejects_bad_hosts() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    // The library binds itself; this test uses the public authorize helper and a one-shot client
    // against `filmcraft-mcp http` only when a port is free. Drive authorize plus an in-process
    // request, then confirm the HTTP parser by spawning the binary when it can bind.
    drop(listener);
    assert!(filmcraft_mcp::authorize_http(Some("127.0.0.1:8765"), None, None, None).is_ok());
    assert!(filmcraft_mcp::authorize_http(Some("evil.example"), None, None, None).is_err());
    assert!(filmcraft_mcp::authorize_http(Some("127.0.0.1"), Some("https://evil.example"), None, None).is_err());
    assert!(filmcraft_mcp::authorize_http(Some("localhost:9"), None, Some("Bearer no"), Some("secret")).is_err());
    assert!(filmcraft_mcp::authorize_http(Some("localhost:9"), None, Some("Bearer secret"), Some("secret")).is_ok());

    let bin = env!("CARGO_BIN_EXE_filmcraft-mcp");
    let mut child = Command::new(bin).args(["http", "--bind", &addr.to_string()]).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap();
    let ready = wait_port(addr).await;
    assert!(ready, "server did not listen");
    let body = json!({"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"generate_black_video","arguments":{"seconds":1,"name":"Black"}}});
    let client = req(addr, "POST", "/mcp", Some(&body.to_string()), None).await;
    assert!(client.contains("\"isError\":false"), "{client}");
    let health = req(addr, "GET", "/health", None, None).await;
    assert!(health.contains("\"ok\":true"), "{health}");
    let blocked = req(addr, "POST", "/mcp", Some("{}"), Some("evil.example")).await;
    assert!(blocked.contains("403") || blocked.contains("refusing Host"), "{blocked}");
    let _ = child.kill();
    let _ = child.wait();
}

async fn wait_port(addr: std::net::SocketAddr) -> bool {
    for _ in 0..50 {
        if tokio::net::TcpStream::connect(addr).await.is_ok() {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    false
}

async fn req(addr: std::net::SocketAddr, method: &str, path: &str, body: Option<&str>, host: Option<&str>) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    let fallback = addr.ip().to_string();
    let host = host.unwrap_or(fallback.as_str());
    let host_header = format!("{host}:{}", addr.port());
    let payload = body.unwrap_or("");
    let req = format!("{method} {path} HTTP/1.1\r\nHost: {host_header}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}", payload.len());
    stream.write_all(req.as_bytes()).await.unwrap();
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.unwrap();
    String::from_utf8_lossy(&buf).to_string()
}
