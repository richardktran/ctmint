pub async fn run() {
    eprintln!("ContextMint MCP server starting (stdio)...");
    eprintln!("Send JSON-RPC messages on stdin, one per line.");
    eprintln!("Tools are stubs (Cycle 0). Ctrl+C to stop.");

    if let Err(e) = ctmint_mcp::server::run_stdio().await {
        eprintln!("MCP server error: {e}");
        std::process::exit(1);
    }
}
