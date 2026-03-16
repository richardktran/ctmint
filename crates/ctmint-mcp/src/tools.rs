use serde::{Deserialize, Serialize};

/// Describes a single MCP tool (name, description, input schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

/// Returns the stub tool definitions for Cycle 0.
pub fn stub_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "get_architecture_map".into(),
            description: "Return the service-to-service dependency graph for the project.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "project_id": { "type": "string", "description": "Project identifier (optional)" }
                }
            }),
        },
        ToolDefinition {
            name: "get_service_graph".into(),
            description: "Return the subgraph of a specific service: its modules, functions, endpoints, and dependencies.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "service_name": { "type": "string", "description": "Name of the service" }
                },
                "required": ["service_name"]
            }),
        },
        ToolDefinition {
            name: "get_function_summary".into(),
            description: "Return the summary (name, signature, file, docstring) of a function by symbol id.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "symbol_id": { "type": "string", "description": "Stable symbol identifier" }
                },
                "required": ["symbol_id"]
            }),
        },
        ToolDefinition {
            name: "get_code_snippet".into(),
            description: "Return the source code snippet for a symbol or file range.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "symbol_id": { "type": "string", "description": "Symbol id or file path" },
                    "line_start": { "type": "integer" },
                    "line_end": { "type": "integer" }
                },
                "required": ["symbol_id"]
            }),
        },
        ToolDefinition {
            name: "search_code".into(),
            description: "Semantic search over code chunks, scoped by project and optionally by service.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Natural language or code query" },
                    "service": { "type": "string", "description": "Optional service name to scope search" },
                    "top_k": { "type": "integer", "description": "Number of results (default 10)" }
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "search_logs".into(),
            description: "Search log entries by service, query text, and time range.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "service": { "type": "string" },
                    "query": { "type": "string" },
                    "time_range": { "type": "string", "description": "e.g. '15m', '1h'" },
                    "limit": { "type": "integer" }
                },
                "required": ["service"]
            }),
        },
        ToolDefinition {
            name: "query_traces".into(),
            description: "Query distributed traces by service or endpoint.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "service": { "type": "string" },
                    "endpoint": { "type": "string" },
                    "time_range": { "type": "string" },
                    "limit": { "type": "integer" }
                }
            }),
        },
        ToolDefinition {
            name: "get_db_schema".into(),
            description: "Return database schema (tables, columns, indexes) for a service or database.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "service_or_db": { "type": "string", "description": "Service name or database name" }
                },
                "required": ["service_or_db"]
            }),
        },
        ToolDefinition {
            name: "diagnose_service".into(),
            description: "Run end-to-end diagnosis for a service: check logs, traces, code, and schema.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "service_name": { "type": "string" },
                    "time_range": { "type": "string" }
                },
                "required": ["service_name"]
            }),
        },
        ToolDefinition {
            name: "diagnose_endpoint".into(),
            description: "Run end-to-end diagnosis for an HTTP endpoint.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "method": { "type": "string", "description": "HTTP method (GET, POST, ...)" },
                    "path": { "type": "string", "description": "URL path (e.g. /login)" },
                    "time_range": { "type": "string" }
                },
                "required": ["method", "path"]
            }),
        },
    ]
}
