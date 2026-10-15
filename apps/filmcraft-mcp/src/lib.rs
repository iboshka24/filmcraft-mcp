//! Named-tool MCP server for the FilmCraft engine.
//!
//! Every edit tool calls [`filmcraft_engine::Session::execute`] (or the engine's renderer /
//! media probe). Stdio and loopback HTTP are transports over the same dispatcher.

#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable))]

use std::sync::{Arc, Mutex, PoisonError};

use filmcraft_engine::Session;
use serde_json::json;

pub mod http;
pub mod probe;
pub mod protocol;
pub mod stdio;
pub mod tools;

pub use protocol::{authorize_http, handle_request};
pub use tools::{Tool, tools, tools_markdown};

/// In-process editing session shared by every transport.
#[derive(Clone)]
pub struct Server {
    session: Arc<Mutex<Session>>,
}

impl Server {
    pub fn new(session: Session) -> Self {
        Self { session: Arc::new(Mutex::new(session)) }
    }

    pub fn empty() -> Self {
        Self::new(Session::default())
    }

    /// Open a project file, or the built-in demo project.
    pub fn open(project: Option<&str>, demo: bool) -> Result<Self, String> {
        if project.is_some() && demo {
            return Err("pass either --project or --demo".into());
        }
        let mut session = Session::default();
        if let Some(path) = project {
            session.execute("file.open", json!({"path": path})).map_err(|e| e.to_string())?;
        } else if demo {
            session.execute("file.openDemoProject", json!({})).map_err(|e| e.to_string())?;
        }
        Ok(Self::new(session))
    }

    pub fn with_session<T>(&self, f: impl FnOnce(&mut Session) -> T) -> T {
        let mut guard = self.session.lock().unwrap_or_else(PoisonError::into_inner);
        f(&mut guard)
    }
}

/// Process arguments. `stdio` is the default. `http` listens on loopback. `tools` prints the catalog.
pub fn cli() -> Result<(), String> {
    let mut project: Option<String> = None;
    let mut demo = false;
    let mut bind = "127.0.0.1:8765".to_string();
    let mut positionals: Vec<String> = Vec::new();
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                print!("{HELP}");
                return Ok(());
            }
            "--version" | "-V" => {
                println!("filmcraft-mcp {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--demo" => demo = true,
            "--project" => {
                project = Some(it.next().ok_or("--project needs a path")?);
            }
            "--bind" => {
                bind = it.next().ok_or("--bind needs host:port")?;
            }
            "--" => {
                positionals.extend(it.by_ref());
                break;
            }
            other if other.starts_with("--project=") => {
                project = Some(other.trim_start_matches("--project=").to_string());
            }
            other if other.starts_with("--bind=") => {
                bind = other.trim_start_matches("--bind=").to_string();
            }
            other if other.starts_with('-') => return Err(format!("unknown option `{other}`")),
            other => positionals.push(other.to_string()),
        }
    }
    let cmd = positionals.first().map(String::as_str).unwrap_or("stdio");
    match cmd {
        "tools" => {
            print!("{}", tools_markdown());
            Ok(())
        }
        "stdio" => {
            if positionals.len() > 1 {
                return Err("stdio takes no extra arguments".into());
            }
            let server = Server::open(project.as_deref(), demo)?;
            stdio::serve(&server)
        }
        "http" => {
            if positionals.len() > 1 {
                return Err("http takes no extra arguments".into());
            }
            let server = Server::open(project.as_deref(), demo)?;
            let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().map_err(|e| e.to_string())?;
            rt.block_on(http::serve(&server, &bind))
        }
        other => Err(format!("unknown command `{other}` (stdio, http, tools)")),
    }
}

const HELP: &str = "\
filmcraft-mcp — MCP server for the FilmCraft engine

USAGE
  filmcraft-mcp [options] [stdio]
  filmcraft-mcp [options] http [--bind 127.0.0.1:8765]
  filmcraft-mcp tools

OPTIONS
  --project PATH    Open this .fcproj before serving
  --demo            Open the built-in demo project
  --bind ADDR       Loopback address for `http` (default 127.0.0.1:8765)
  --version         Print the version
  --help            This text

TRANSPORTS
  stdio   JSON-RPC, one JSON object per line (Content-Length framing is also accepted)
  http    POST http://127.0.0.1:8765/mcp   loopback only, optional FILMCRAFT_MCP_TOKEN

The tool list is `filmcraft-mcp tools` and docs/MCP.md.
";
