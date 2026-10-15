//! Stdio transport: one JSON-RPC message per line, or LSP-style `Content-Length` frames.

use std::io::{BufRead, Write};

use serde_json::Value;

use crate::Server;
use crate::protocol::handle_request;

pub fn serve(server: &Server) -> Result<(), String> {
    let stdin = std::io::stdin();
    let mut reader = stdin.lock();
    let mut stdout = std::io::stdout();
    let mut header = String::new();
    loop {
        header.clear();
        let n = reader.read_line(&mut header).map_err(|e| e.to_string())?;
        if n == 0 {
            return Ok(());
        }
        let trimmed = header.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (body, framed) = if trimmed.to_ascii_lowercase().starts_with("content-length:") {
            let len: usize = trimmed.split(':').nth(1).and_then(|s| s.trim().parse().ok()).ok_or_else(|| "bad Content-Length".to_string())?;
            if len > 1024 * 1024 {
                return Err("stdio message exceeds 1 MiB".into());
            }
            loop {
                let mut extra = String::new();
                reader.read_line(&mut extra).map_err(|e| e.to_string())?;
                if extra.trim().is_empty() {
                    break;
                }
            }
            let mut buf = vec![0u8; len];
            std::io::Read::read_exact(&mut reader, &mut buf).map_err(|e| e.to_string())?;
            (String::from_utf8(buf).map_err(|e| e.to_string())?, true)
        } else {
            (trimmed.to_string(), false)
        };
        let req: Value = match serde_json::from_str(&body) {
            Ok(v) => v,
            Err(e) => {
                let err = serde_json::json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":format!("parse error: {e}")}});
                write_message(&mut stdout, &err, framed)?;
                continue;
            }
        };
        if let Some(resp) = handle_request(server, &req) {
            write_message(&mut stdout, &resp, framed)?;
        }
    }
}

fn write_message(out: &mut impl Write, value: &Value, framed: bool) -> Result<(), String> {
    let body = serde_json::to_string(value).map_err(|e| e.to_string())?;
    if framed {
        write!(out, "Content-Length: {}\r\n\r\n{}", body.len(), body).map_err(|e| e.to_string())?;
    } else {
        writeln!(out, "{body}").map_err(|e| e.to_string())?;
    }
    out.flush().map_err(|e| e.to_string())
}
