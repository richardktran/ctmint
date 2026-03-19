use crate::protocol::{JsonRpcRequest, JsonRpcResponse};
use crate::tools;
use ctmint_config::{GlobalConfig, ProjectManifest};
use ctmint_storage::graph::GraphStore;
use ctmint_storage::SqliteGraphStore;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Run the MCP server over stdio (one JSON-RPC message per line).
pub async fn run_stdio() -> std::io::Result<()> {
    let config = GlobalConfig::resolve();
    std::fs::create_dir_all(&config.data_dir).ok();
    let store: Arc<SqliteGraphStore> = match SqliteGraphStore::open(config.graph_db_path()) {
        Ok(s) => Arc::new(s),
        Err(e) => {
            eprintln!("MCP: Failed to open graph store: {e}");
            std::process::exit(1);
        }
    };

    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

    loop {
        line.clear();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break; // EOF
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<JsonRpcRequest>(trimmed) {
            Ok(req) => handle_request(req, store.clone()).await,
            Err(e) => JsonRpcResponse::error(None, -32700, format!("Parse error: {e}")),
        };

        let mut out = serde_json::to_string(&response).unwrap();
        out.push('\n');
        stdout.write_all(out.as_bytes()).await?;
        stdout.flush().await?;
    }

    Ok(())
}

async fn handle_request(req: JsonRpcRequest, store: Arc<SqliteGraphStore>) -> JsonRpcResponse {
    match req.method.as_str() {
        "initialize" => handle_initialize(req.id),
        "tools/list" => handle_tools_list(req.id),
        "tools/call" => handle_tools_call(req.id, &req.params, store).await,
        _ => JsonRpcResponse::error(req.id, -32601, format!("Method not found: {}", req.method)),
    }
}

fn resolve_project_id() -> String {
    if let Ok(cwd) = std::env::current_dir() {
        if let Some(path) = ProjectManifest::discover(&cwd) {
            if let Ok(m) = ProjectManifest::load(&path) {
                return m.project;
            }
        }
    }
    "default".to_string()
}

fn handle_initialize(id: Option<serde_json::Value>) -> JsonRpcResponse {
    JsonRpcResponse::success(
        id,
        serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "ctmint",
                "version": env!("CARGO_PKG_VERSION")
            }
        }),
    )
}

fn handle_tools_list(id: Option<serde_json::Value>) -> JsonRpcResponse {
    let defs = tools::stub_tool_definitions();
    JsonRpcResponse::success(id, serde_json::json!({ "tools": defs }))
}

async fn handle_tools_call(
    id: Option<serde_json::Value>,
    params: &serde_json::Value,
    store: Arc<SqliteGraphStore>,
) -> JsonRpcResponse {
    let tool_name = params
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("<unknown>");
    let empty = serde_json::Map::new();
    let args = params.get("arguments").and_then(|v| v.as_object()).unwrap_or(&empty);

    let known = tools::stub_tool_definitions();
    let exists = known.iter().any(|t| t.name == tool_name);

    if !exists {
        return JsonRpcResponse::error(id, -32602, format!("Unknown tool: {tool_name}"));
    }

    let project_id = args
        .get("project_id")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(resolve_project_id);

    let text = match tool_name {
        "get_architecture_map" => {
            match store.get_architecture_map(&project_id).await {
                Ok(map) => serde_json::to_string_pretty(&map).unwrap_or_else(|_| "{}".to_string()),
                Err(e) => format!("Error: {e}"),
            }
        }
        "get_service_graph" => {
            let service_name = args
                .get("service_name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if service_name.is_empty() {
                "Error: service_name is required".to_string()
            } else {
                match store.get_service_graph(service_name, &project_id).await {
                    Ok(subgraph) => serde_json::to_string_pretty(&subgraph).unwrap_or_else(|_| "{}".to_string()),
                    Err(e) => format!("Error: {e}"),
                }
            }
        }
        _ => format!("Tool '{tool_name}' is not implemented yet."),
    };

    JsonRpcResponse::success(
        id,
        serde_json::json!({
            "content": [{
                "type": "text",
                "text": text
            }]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_initialize() {
        let resp = handle_initialize(Some(serde_json::json!(1)));
        let result = resp.result.unwrap();
        assert_eq!(result["serverInfo"]["name"], "ctmint");
    }

    #[test]
    fn test_tools_list() {
        let resp = handle_tools_list(Some(serde_json::json!(2)));
        let result = resp.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert!(!tools.is_empty());
        let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
        assert!(names.contains(&"get_architecture_map"));
        assert!(names.contains(&"diagnose_service"));
    }

    #[tokio::test]
    async fn test_tools_call_get_architecture_map() {
        let store = Arc::new(SqliteGraphStore::open(":memory:").unwrap());
        let params = serde_json::json!({ "name": "get_architecture_map", "arguments": {} });
        let resp = handle_tools_call(Some(serde_json::json!(3)), &params, store).await;
        let text = resp.result.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(text.contains("nodes") || text.contains("edges") || text.contains("Error"));
    }

    #[tokio::test]
    async fn test_tools_call_unknown() {
        let store = Arc::new(SqliteGraphStore::open(":memory:").unwrap());
        let params = serde_json::json!({ "name": "nonexistent_tool" });
        let resp = handle_tools_call(Some(serde_json::json!(4)), &params, store).await;
        assert!(resp.error.is_some());
    }
}
