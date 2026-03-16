use crate::protocol::{JsonRpcRequest, JsonRpcResponse};
use crate::tools;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Run the MCP server over stdio (one JSON-RPC message per line).
///
/// Cycle 0: all tool calls return "Not implemented."
/// Later cycles will wire real handlers via GraphStore / VectorStore.
pub async fn run_stdio() -> std::io::Result<()> {
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
            Ok(req) => handle_request(req),
            Err(e) => JsonRpcResponse::error(None, -32700, format!("Parse error: {e}")),
        };

        let mut out = serde_json::to_string(&response).unwrap();
        out.push('\n');
        stdout.write_all(out.as_bytes()).await?;
        stdout.flush().await?;
    }

    Ok(())
}

fn handle_request(req: JsonRpcRequest) -> JsonRpcResponse {
    match req.method.as_str() {
        "initialize" => handle_initialize(req.id),
        "tools/list" => handle_tools_list(req.id),
        "tools/call" => handle_tools_call(req.id, &req.params),
        _ => JsonRpcResponse::error(req.id, -32601, format!("Method not found: {}", req.method)),
    }
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

fn handle_tools_call(
    id: Option<serde_json::Value>,
    params: &serde_json::Value,
) -> JsonRpcResponse {
    let tool_name = params
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("<unknown>");

    let known = tools::stub_tool_definitions();
    let exists = known.iter().any(|t| t.name == tool_name);

    if !exists {
        return JsonRpcResponse::error(id, -32602, format!("Unknown tool: {tool_name}"));
    }

    JsonRpcResponse::success(
        id,
        serde_json::json!({
            "content": [{
                "type": "text",
                "text": format!("Tool '{tool_name}' is not implemented yet. This is a Cycle 0 stub.")
            }]
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_tools_call_stub() {
        let params = serde_json::json!({ "name": "get_architecture_map", "arguments": {} });
        let resp = handle_tools_call(Some(serde_json::json!(3)), &params);
        let text = resp.result.unwrap()["content"][0]["text"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(text.contains("not implemented"));
    }

    #[test]
    fn test_tools_call_unknown() {
        let params = serde_json::json!({ "name": "nonexistent_tool" });
        let resp = handle_tools_call(Some(serde_json::json!(4)), &params);
        assert!(resp.error.is_some());
    }
}
