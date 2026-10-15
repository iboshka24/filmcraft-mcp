fn main() {
    if let Err(e) = filmcraft_mcp::cli() {
        eprintln!("filmcraft-mcp: {e}");
        std::process::exit(1);
    }
}
