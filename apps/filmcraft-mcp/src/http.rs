//! Loopback HTTP transport. `POST /mcp` is one JSON-RPC request; the body limit is 1 MiB.

use std::net::SocketAddr;

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::Server;
use crate::protocol::{authorize_http, handle_request};

const MAX_BODY: usize = 1024 * 1024;

pub async fn serve(server: &Server, bind: &str) -> Result<(), String> {
    let addr: SocketAddr = bind.parse().map_err(|e| format!("bad --bind `{bind}`: {e}"))?;
    if !addr.ip().is_loopback() {
        return Err(format!("refusing to bind {addr}: HTTP MCP is loopback only"));
    }
    let listener = TcpListener::bind(addr).await.map_err(|e| format!("bind {addr}: {e}"))?;
    let actual = listener.local_addr().map_err(|e| e.to_string())?;
    eprintln!("filmcraft-mcp http listening on http://{actual}/mcp");
    loop {
        let (socket, peer) = listener.accept().await.map_err(|e| e.to_string())?;
        if !peer.ip().is_loopback() {
            continue;
        }
        let server = server.clone();
        tokio::spawn(async move {
            let _ = handle_connection(server, socket).await;
        });
    }
}

async fn handle_connection(server: Server, mut socket: tokio::net::TcpStream) -> Result<(), String> {
    let token = std::env::var("FILMCRAFT_MCP_TOKEN").ok();
    loop {
        let Some(req) = read_request(&mut socket).await? else { return Ok(()) };
        let response = respond(&server, &req, token.as_deref());
        socket.write_all(response.as_bytes()).await.map_err(|e| e.to_string())?;
        if req.close {
            return Ok(());
        }
    }
}

struct Incoming {
    method: String,
    path: String,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    close: bool,
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
}

fn respond(server: &Server, req: &Incoming, token: Option<&str>) -> String {
    if req.path != "/mcp" && req.path != "/health" {
        return http(404, "application/json", &json!({"error": "not found"}).to_string(), true);
    }
    if let Err(e) = authorize_http(header(&req.headers, "host"), header(&req.headers, "origin"), header(&req.headers, "authorization"), token) {
        let status = if e == "unauthorized" { 401 } else { 403 };
        return http(status, "application/json", &json!({"error": e}).to_string(), true);
    }
    if req.path == "/health" {
        return http(200, "application/json", &json!({"ok": true, "tools": crate::tools().len()}).to_string(), req.close);
    }
    if req.method != "POST" {
        return http_raw(405, "application/json", b"{\"error\":\"POST /mcp\"}", true, Some("POST"));
    }
    let parsed: Value = match serde_json::from_slice(&req.body) {
        Ok(v) => v,
        Err(e) => {
            let err = json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":format!("parse error: {e}")}});
            return http(200, "application/json", &err.to_string(), req.close);
        }
    };
    match handle_request(server, &parsed) {
        Some(v) => http(200, "application/json", &v.to_string(), req.close),
        None => http(202, "application/json", "{}", req.close),
    }
}

fn http(status: u16, content_type: &str, body: &str, close: bool) -> String {
    http_raw(status, content_type, body.as_bytes(), close, None)
}

fn http_raw(status: u16, content_type: &str, body: &[u8], close: bool, allow: Option<&str>) -> String {
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Payload Too Large",
        _ => "Error",
    };
    let conn = if close { "close" } else { "keep-alive" };
    let mut head = format!("HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: {conn}\r\n", body.len());
    if let Some(allow) = allow {
        head.push_str(&format!("Allow: {allow}\r\n"));
    }
    head.push_str("\r\n");
    head.push_str(&String::from_utf8_lossy(body));
    head
}

async fn read_request(socket: &mut tokio::net::TcpStream) -> Result<Option<Incoming>, String> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    let header_end = loop {
        if buf.len() > MAX_BODY + 8192 {
            return Err("headers too large".into());
        }
        let n = socket.read(&mut tmp).await.map_err(|e| e.to_string())?;
        if n == 0 {
            return if buf.is_empty() { Ok(None) } else { Err("truncated request".into()) };
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(i) = find_header_end(&buf) {
            break i;
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut lines = head.split("\r\n");
    let request_line = lines.next().unwrap_or("");
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("").to_string();
    let path = parts.next().unwrap_or("").to_string();
    let mut headers = Vec::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    let len: usize = header(&headers, "content-length").and_then(|s| s.parse().ok()).unwrap_or(0);
    if len > MAX_BODY {
        let _ = socket.write_all(http(413, "application/json", &json!({"error":"body exceeds 1 MiB"}).to_string(), true).as_bytes()).await;
        return Err("body too large".into());
    }
    let mut body = buf[header_end..].to_vec();
    while body.len() < len {
        let n = socket.read(&mut tmp).await.map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("truncated body".into());
        }
        body.extend_from_slice(&tmp[..n]);
    }
    body.truncate(len);
    let close = header(&headers, "connection").is_some_and(|v| v.eq_ignore_ascii_case("close"));
    Ok(Some(Incoming { method, path, headers, body, close }))
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4)
}
