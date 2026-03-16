## MCP Core – Interface & Tooling Design

### 1. Purpose of MCP Core

**MCP Core** is the concrete implementation of the Model Context Protocol server that:
- Exposes a **unified tool surface** to external LLM agents.
- Hides internal complexity:
  - Multiple plugins.
  - Graph and vector queries.
  - Context funneling and planning.
- Implements **capability‑aware tool exposure** to control context explosion.

It is the “front door” for:
- IDEs (Cursor‑style, code assistants).
- Chat UIs.
- Other orchestrators or agents.

---

### 2. External View: MCP Tools

From a client’s perspective, MCP Core exposes a curated set of tools such as:

```text
get_service_graph
get_architecture_map
get_service_summary
get_function_summary
get_code_snippet
search_logs
query_traces
get_db_schema
get_runtime_snapshot
diagnose_endpoint
diagnose_service
```

These tools are **logical** tools that may internally:
- Call multiple plugins.
- Query SKG and vector index.
- Run context funnel and summarization.

---

### 3. Tool Taxonomy

To keep the interface clean, MCP Core tools can be grouped into categories:

1. **System Topology & Architecture**
   - `get_architecture_map(project_id?)`
   - `get_service_graph(service_name, project_id?)`
   - `get_service_dependencies(service_name)`

2. **Code Intelligence**
   - `get_service_summary(service_name)`
   - `get_module_summary(path_or_id)`
   - `get_function_summary(symbol_id)`
   - `get_code_snippet(symbol_id | file_path, range?)`

3. **Runtime & Data**
   - `search_logs(service, query, time_range, limit)`
   - `get_recent_traces(service, time_range, limit)`
   - `get_endpoint_traces(endpoint, time_range, limit)`
   - `get_db_schema(service | db_name)`
   - `get_db_usage(table_name | column_name)`

4. **High-Level Diagnosis**
   - `diagnose_endpoint(endpoint, time_range?)`
   - `diagnose_service(service_name, time_range?)`

5. **Meta / Introspection**
   - `list_capabilities()`
   - `list_services(project_id?)`
   - `list_projects()`

High‑level tools (`diagnose_*`) are essentially thin wrappers around orchestrator flows plus funnel + LLM calls.

---

### 4. Capability-Based Exposure

MCP Core does **not** always expose all tools simultaneously. Instead:
- It uses **Capability Router** (see `orchestrator.md`) to:
  - Decide which tools to include in the MCP tool list for a given interaction/session.
- For some clients, you may:
  - Expose only **low‑level tools** (for expert agents).
  - Expose **high‑level diagnosis tools** (for simpler, human‑driven flows).

Example:
- For a **debugging chat agent**:
  - Expose `diagnose_endpoint`, `diagnose_service`, and a small set of drill‑down tools (`get_service_summary`, `get_code_snippet`).
- For a **code assistant inside IDE**:
  - Emphasize `get_function_summary`, `get_code_snippet`, `get_service_graph`, etc.

---

### 5. Internal Architecture

```text
                MCP Core Server (Rust)
                          │
                          ▼
                    Tool Dispatcher
                          │
          ┌───────────────┼────────────────┐
          │               │                │
  Orchestrator API   SKG Client      Vector Client
          │               │                │
          ▼               ▼                ▼
   Planner + Funnel   Graph Storage    Vector Storage
          │
          ▼
   Capability Router ──► Plugin Bridge ──► MCP Plugins
```

Key internal modules:
- **MCP Transport Layer**:
  - Implements the protocol (JSON‑RPC or similar).
  - Handles tool schemas, calls, and streaming results if supported.
- **Tool Dispatcher**:
  - Maps tool name → internal handler.
  - Handles versioning and deprecation.
- **Plugin Bridge**:
  - Uniform interface to external MCP plugin servers or in‑process plugins.

---

### 6. Tool Schema and Types (Examples)

**Example: `diagnose_endpoint`**:

- **Input schema** (conceptual):

```json
{
  "type": "object",
  "properties": {
    "endpoint": { "type": "string" },
    "project_id": { "type": "string", "nullable": true },
    "time_range": { "type": "string", "description": "e.g. '15m', '1h', 'since 2025-01-01T00:00:00Z'" }
  },
  "required": ["endpoint"]
}
```

- **Output shape**:

```json
{
  "status": "ok",
  "endpoint": "POST /login",
  "service": "auth-service",
  "summary": "Login latency increased due to Redis timeouts.",
  "evidence": {
    "traces": [ /* summarized trace evidence */ ],
    "logs": [ /* summarized or representative log lines */ ],
    "code": [ /* references to functions/modules involved */ ],
    "db": [ /* relevant query/locking/index info */ ]
  },
  "root_cause_hypotheses": [
    {
      "description": "Redis connection pool exhausted causing timeouts.",
      "confidence": 0.82
    }
  ]
}
```

Actual MCP output will often be text, but including a structured JSON envelope (stringified) is recommended for downstream automation.

---

### 7. Multi-Project & Adapter Model

MCP Core is designed to be **project‑agnostic**:
- It operates on the **normalized System Model** (`Service`, `Endpoint`, `Function`, `LogEvent`, `Trace`, `DatabaseTable`, etc.).
- Project‑specific details are handled by **adapters** and **plugins**.

**Project manifest** (conceptual):

```yaml
project: ecommerce

services:
  - name: auth-service
    repo: git@company/auth.git
    language: python

  - name: payment-service
    repo: git@company/payment.git
    language: rust

logs:
  provider: file
  path: /var/log/ecommerce/*.log

tracing:
  provider: otel
  endpoint: http://otel-collector:4317

database:
  type: postgres
  connection: postgres://user:pass@host/db
```

Adapters use this manifest to:
- Configure ingestion pipelines.
- Map raw sources into normalized graph entities.

MCP tools accept `project_id` where necessary to operate in a multi‑tenant environment.

---

### 8. Security & Isolation

Key principles:
- **Least privilege**:
  - Plugins only get access to the data sources they need (logs, DB, traces).
- **Project isolation**:
  - When multi‑project, ensure graph and indices logically separated by `project_id`.
- **Auditing**:
  - MCP Core should log:
    - Which tools were called.
    - With what parameters (minus sensitive data).
    - What data sources were accessed.

Optional:
- Support per‑project or per‑capability access control (e.g. logs visible, DB queries restricted).

---

### 9. Performance Considerations

To keep interactive latency low:
- **Pre‑compute**:
  - Symbol graphs, architecture maps, and semantic summaries.
- **Cache**:
  - Service and function summaries.
  - Architecture map and service graph queries.
- **Parallelize**:
  - Log/trace/db searches for a given capability set.
- **Cap budgets**:
  - Max number of log lines, traces, and code snippets per call.
  - Force summarization beyond thresholds.

MCP Core should enforce global limits such as:
- Max total tokens to send to the LLM.
- Max number of tool calls per question.

---

### 10. Evolution Path

Potential future features:
- **Streaming MCP results**:
  - Stream intermediate analysis to the client while deeper queries run.
- **Tool auto‑generation**:
  - Derive new tools (e.g. `get_error_rate(service)`) automatically from SKG + metrics.
- **Adaptive capabilities**:
  - Use SKG context to refine capabilities dynamically (e.g. add `database` capability if an endpoint is tightly coupled to slow queries).

MCP Core should remain **backwards compatible** at the protocol level, adding new tools and capabilities without breaking existing clients.

