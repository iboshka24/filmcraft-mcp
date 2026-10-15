//! `media_probe`: container and stream info through FilmCraft's own demuxers.

use std::sync::Arc;

use serde_json::{Value, json};

pub fn probe_path(path: &str) -> Result<Value, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let bytes: Arc<[u8]> = bytes.into();
    let name = std::path::Path::new(path).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| path.to_string());
    let src = filmcraft_codecs::open_bytes(&name, bytes).map_err(|e| format!("{path}: {e}"))?;
    let mut v = serde_json::to_value(src.info()).map_err(|e| e.to_string())?;
    v["path"] = json!(path);
    v["bytes"] = json!(std::fs::metadata(path).map(|m| m.len()).unwrap_or(0));
    Ok(v)
}
