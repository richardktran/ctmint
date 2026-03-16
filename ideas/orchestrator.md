## Orchestrator Design (ContextMint)

### 1. Role of the Orchestrator

The **Orchestrator** is the brain of ContextMint. It:
- Receives **natural language questions** from an AI agent or IDE.
- Plans which **capabilities** and **tools** to use.
- Coordinates calls to:
  - System Knowledge Graph
  - Vector Retrieval Layer
  - MCP Plugins (logs, traces, DB, k8s, network, etc.)
- Runs the **Context Funnel** to compress all relevant information.
- Returns a **compact, structured context** to the LLM and merges the answer back.

It exposes a single MCP endpoint so that external agents see **one logical tool surface**, even though internally many subsystems and plugins are involved.

---

### 2. High-Level Orchestrator Architecture

```text
                External LLM / IDE / Agent
                             │
                             ▼
                      MCP Orchestrator
                  (this component, core logic)
                             │
          ┌──────────────────┼──────────────────┐
          │                  │                  │
   Capability Router   Context Funnel     Tool Registry
          │                  │                  │
          ▼                  ▼                  ▼
     MCP Plugins        SKG + Vector    Plugin Manifests
 (logs, code, db, …)     Retrieval
```

Subcomponents:
- **Request Interpreter**: normalizes incoming MCP requests + user prompts.
- **Planner**:
  - Generates a **tool plan** (which capabilities to invoke, in what order/parallel).
  - Uses a small LLM (or rules) for planning.
- **Capability Router**:
  - Maps problem into high‑level **capabilities** (e.g. `logs`, `traces`, `code`, `database`, `metrics`, `network`).
  - Selects relevant plugins and tools (reducing tool surface).
- **Context Funnel**:
  - Coordinates SKG queries, plugin calls, and vector retrieval.
  - Produces a final, compressed context bundle.
- **Tool Execution Engine**:
  - Executes tool calls (often in parallel).
  - Handles retries, timeouts, and error normalization.

---

### 3. Request Lifecycle

1. **Receive question**
   - From MCP client (IDE, chat UI, other orchestrators).
   - Example: “Why is `payment-service` returning 500?”.

2. **Interpret & classify**
   - Extract entities (service, endpoint, time window hints).
   - Classify into **capabilities** using a small local model or heuristic rules:
     - `logs`, `traces`, `code`, `database`, possibly `network`.

3. **Plan tool usage**
   - Build a plan such as:
     - `get_service_graph(payment-service)`
     - `search_traces(payment-service, timeframe=last_15m)`
     - `search_logs(payment-service, level=error)`
     - `get_db_queries(payment-service)`
   - Decide which calls can run **in parallel**.

4. **Execute plan**
   - Route tool calls to appropriate MCP plugins via **Tool Registry** and **Capability Router**.
   - Enforce deadlines and time budgets.

5. **Context funnel**
   - Combine outputs into a structured intermediate representation.
   - Query SKG and vector index as needed to:
     - Narrow entities.
     - Pull semantic summaries and minimal raw evidence.

6. **LLM call**
   - Build a compact prompt containing:
     - Problem statement.
     - Architecture/graph context.
     - Summarized evidence + small number of raw snippets.
   - Call LLM (external API or local model).

7. **Return answer**
   - Send back structured output (free text + optional structured diagnosis JSON).

---

### 4. Capability Routing

The orchestrator uses **capabilities** to avoid exposing hundreds of tools at once.

**Capability examples**:
- `code`, `logs`, `traces`, `database`, `metrics`, `network`, `kubernetes`, `deployment`, `security`, `cost`.

**Flow**:

```text
User Question
   │
   ▼
Capability Classifier (small LLM / rules)
   │
   ▼
Set of capabilities, e.g. {logs, traces, code}
   │
   ▼
Capability Router → relevant plugins
   │
   ▼
Tool Registry → tools exposed to planner/LLM
```

For example:
- Question: “Why is login endpoint slow?”
- Classifier: `logs`, `traces`, `code`.
- Router:
  - logs → `logs-plugin`
  - traces → `tracing-plugin`
  - code → `code-plugin`
- Expose only tools from these three plugins into the LLM’s tool list.

---

### 5. Tool Registry

The Tool Registry stores plugin and tool metadata.

**Plugin manifest example**:

```json
{
  "name": "logs-plugin",
  "capabilities": ["logs"],
  "tools": [
    "search_logs",
    "tail_logs",
    "aggregate_logs",
    "get_error_rate"
  ]
}
```

**Registry responsibilities**:
- Load plugin manifests (from file system or discovery endpoint).
- Index by:
  - Capability → plugin(s)
  - Tool name → plugin
- Provide filtered tool lists to:
  - Planner
  - Context Funnel

---

### 6. Planning & Execution Model

**Planning**:
- Planner receives:
  - User question
  - Candidate tools (from Capability Router + Tool Registry)
  - Optional prior state (previous results, thread context)
- Planner outputs an ordered list/graph of steps:

```text
1. get_service_graph("payment-service")
2. in parallel:
     - search_traces(service="payment-service", timeframe="15m")
     - search_logs(service="payment-service", level="error", timeframe="15m")
3. get_db_queries("payment-service")
```

**Execution engine**:
- Runs steps with:
  - Parallel batching where possible.
  - Timeouts & retries.
  - Structured error results (`{ok: false, error_type, details}`).
- Returns a structured execution trace to the Context Funnel.

---

### 7. Integration with System Knowledge Graph & Vector Index

The orchestrator does not talk to raw data stores directly; it routes through internal clients.

**Internally**:

```text
Orchestrator
   │
   ├─ SKG Client (graph queries)
   │    - find_service(service_name)
   │    - get_dependencies(service)
   │    - get_endpoints(service)
   │    - get_db_relations(service)
   │
   └─ Vector Client (semantic search)
        - search_code(query, scope)
        - search_logs(query, scope)
        - search_docs(query, scope)
```

The **Context Funnel** uses these clients to:
- Narrow the entity set based on SKG.
- Run semantic search only over the narrowed scope.

---

### 8. Context Funnel Interface

The orchestrator sees the funnel as a black box with a clear contract:

**Input**:
- User question.
- Execution results from tools/plugins (raw or minimally processed).
- Graph and vector query clients.

**Output**:
- `ContextBundle`:
  - `architecture_summary`
  - `service_summaries`
  - `function_summaries`
  - `runtime_summaries` (logs/traces/metrics)
  - `critical_snippets` (code/log/trace fragments)
  - `debug_metadata` (time window, services involved, confidence hints)

The orchestrator then passes `ContextBundle` to the LLM.

---

### 9. Error Handling & Degradation

The orchestrator must handle partial failures gracefully:
- Missing plugins (e.g. no traces):
  - Mark capability as unavailable and continue with remaining evidence.
- Graph/vector unreachable:
  - Fall back to simpler vector‑only search, or directly to plugin tools.
- Timeouts:
  - Enforce global and per‑step time budgets.
  - Return partial analysis with clear “limited evidence” markers.

The response model should allow the LLM to explain uncertainty (e.g. “No trace data available for the last 15 minutes; diagnosis based solely on logs and code.”).

---

### 10. Implementation Notes

- **Language**:
  - Core orchestrator logic implemented in **Rust** (single binary).
  - Planning and capability classification may call out to:
    - External LLM via HTTP.
    - Local LLM (e.g. via Ollama) for privacy/local scenarios.
- **Concurrency**:
  - Use async runtime (e.g. `tokio`), with bounded concurrency for tool calls.
- **Configuration**:
  - YAML/TOML config listing:
    - Plugins paths / endpoints.
    - LLM endpoints and models.
    - Timeouts and resource budgets.
- **Observability**:
  - Orchestrator should emit its own traces/logs so it can self‑diagnose.

Further details on MCP contracts and tool schemas are specified in `mcp-core.md`.

