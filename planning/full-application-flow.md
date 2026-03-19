# Full Application Flow: Onboarding -> Index -> MCP Server

This document describes the end-to-end pipeline for ContextMint so an external client can:

1. Run onboarding to generate a project manifest (`ctmint.yaml`)
2. Index code to populate the System Knowledge Graph (SKG) (and optionally the vector index)
3. Start the MCP server (stdio JSON-RPC)
4. Call MCP tools to query code/runtime/data through graph traversal (and vector retrieval where applicable)

Notes about current repo state:
- In the CLI right now, `ctmint index` and the MCP tool implementations are still stubs (Cycle 2/3 not implemented yet). This doc includes the intended “final” flow once those cycles are completed, and it also calls out what happens today so you are not surprised.

---

## 0. Components (who does what)

### Onboarding (Cycle 1 writer)
- Command: `ctmint init`
- Purpose: scan your repo / ask questions and write a single source of truth manifest file: `ctmint.yaml`.

### Code Parser (Cycle 3 helper)
- Reads: source files from paths in the manifest
- Outputs: normalized `symbols` + `relations` (e.g., modules/functions/endpoints, imports/calls/data access hints)
- Does NOT write to SKG.

### Code Indexer (Cycle 3 writer)
- Reads: parser output
- Writes: graph nodes + edges into the SKG through the `GraphStore` interface
- Also produces vector “chunks” (optional until Cycle 4).

### SKG / Graph Store (Cycle 2 storage + traversal API)
- Stores: `Node` + `Edge` records
- Provides: traversal queries like “architecture map” and “service subgraph”

### Vector Index (Cycle 4)
- Stores embeddings for code chunks
- Enables semantic search after the graph narrows the scope.

### Runtime + Data Ingestion (Cycles 6-7)
- DB schema ingestion: writes `Database`, `DatabaseTable`, `Column`, etc. into the SKG and links code reads/writes
- Logs/traces ingestion: links runtime evidence to services/spans/endpoints.

### Context Funnel + Orchestrator (Cycle 5 and later)
- Read-only: uses SKG traversal to choose the right entities
- Fetches code snippets/summaries and runtime evidence
- Builds a compressed “prompt context” for the AI agent.

### MCP Server (final interface)
- Command: `ctmint serve`
- Protocol: MCP over stdio using JSON-RPC 2.0 messages (one per line)
- In the final system, tool handlers will call:
  - SKG traversal APIs for graph questions
  - Vector search for semantic retrieval
  - Runtime/data query APIs for evidence

---

## 1. “Normal run” (what you will do)

### Step 1: Build the binary
Run from the repo root:

```bash
cargo build
```

### Step 2: Onboard (generate `ctmint.yaml`)
Example:

```bash
ctmint init
```

If you want non-interactive paths:
```bash
ctmint init --path /path/to/your/repo --output my-project.yaml
```

What you should get:
- A manifest file (typically `ctmint.yaml`) describing:
  - `project`
  - `services[]` with `name`, `language`, and `repo_path`
  - locations/config for logs/DB/tracing (depending on your onboarding answers)

### Step 3: Index code (populate the SKG)
Example:

```bash
ctmint index --project ctmint.yaml
```

What this does in the final flow (Cycles 2-3):
- Load `ctmint.yaml`
- For each service in `manifest.services`:
  - Run the **Code Parser** on `service.repo_path` (language adapter from `service.language`)
  - Run the **Code Indexer**:
    - create/update `Service`, `Module`, `Function`, `Endpoint` nodes
    - create edges like `CONTAINS`, `IMPLEMENTS`, `CALLS`, `IMPORTS`, and `READS/WRITES` (as detectable)
    - batch-write nodes/edges into SKG using `GraphStore.batch_commit`
- Optionally create vector chunks (Cycle 4)

What happens today in this repo:
- The CLI prints: `[Cycle 3] Code parser and indexer are not implemented yet.`

### Step 4: Start the MCP server (so a client can call it)
Example:

```bash
ctmint serve
```

What happens today in this repo:
- The server starts and accepts JSON-RPC messages on stdin/stdout.
- Tool calls return “Cycle 0 stub” text (tools are not wired to real SKG/Vector yet).

In the final flow:
- Tool calls will return real graph answers and evidence.

---

## 2. Architecture: end-to-end data flow

### Runbook view

```text
ctmint init
    |
    v
ctmint.yaml (manifest)
    |
    v
ctmint index
    |
    +--> Code Parser (Cycle 3) reads source -> symbols/relations (normalized)
    |
    +--> Code Indexer (Cycle 3) writes Node/Edge into SKG (Cycle 2)
    |
    +--> (optional) Vector chunking (Cycle 4)
    |
    v
SKG (SQLite) + Vector index
    |
    v
ctmint serve
    |
    +--> MCP tool handlers:
          - graph traversal (SKG)
          - semantic retrieval (Vector)
          - evidence retrieval (logs/traces/db after Cycles 6-7)
```

### Where “parse and index” live
- The **parse** step happens inside the Code Indexer pipeline, which is triggered by `ctmint index`.
- The **index** step happens immediately after parser output is produced, and it performs SKG writes via `GraphStore`.

In other words:
- `ctmint index` is the entrypoint that orchestrates parser + indexer (Cycles 3 writers)
- SKG writes are performed during the indexer stage using Node/Edge upserts + `batch_commit`.

---

## 3. Client interaction: how the server is called

The MCP server runs over stdio and expects one JSON-RPC message per line.

### Message format
Typical MCP pattern:
- `initialize`
- `tools/list`
- `tools/call` with `{ "name": "...", "arguments": { ... } }`

### Example: tool discovery
Client sends:

```json
{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}
```

Server responds with a JSON result containing tool definitions (names, input schema).

### Example: query the architecture map (service -> service dependencies)
Client sends (tool name depends on your final wiring, but your repo already defines this stub tool):

```json
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"get_architecture_map","arguments":{}}}
```

Final flow behavior:
- Handler calls SKG traversal helper (Cycle 2 API) like `get_architecture_map(project_id)`
- Returns JSON (graph nodes + edges or adjacency list) describing service-to-service calls/dependencies.

What happens today:
- Server returns: `Tool 'get_architecture_map' is not implemented yet. This is a Cycle 0 stub.`

### Example: query a service subgraph
Client sends:

```json
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_service_graph","arguments":{"service_name":"auth-service"}}}
```

Final flow behavior:
- Handler finds the `Service` node for `auth-service`
- Traverses one-hop and related edges/nodes (modules/functions/endpoints + dependencies)
- Returns a JSON subgraph.

---

## 4. What to store in SKG (so tools can answer)

SKG content is a graph of:
- **Nodes**: typed entities (Service, Module, Function, Endpoint, DatabaseTable, Column, LogEvent, Trace, Span, etc.)
- **Edges**: typed relations (CONTAINS, CALLS, IMPORTS, READS, WRITES, IMPLEMENTS, FOREIGN_KEY, etc.)

Implementation details (Cycle 2 storage contract):
- A `Node` record contains:
  - `id` (stable)
  - `node_type` (Service/Function/Endpoint/...)
  - `project_id`
  - `attrs` (JSON key-value data)
- An `Edge` record contains:
  - `source_id`, `target_id`
  - `edge_type` (CALLS/CONTAINS/READS/...)
  - `project_id`
  - `attrs` (JSON metadata for extra info, if needed)

In the final system, Cycle 3 will populate the “code-domain” nodes/edges; later cycles will populate data/runtime nodes/edges.

---

## 5. When code changes: what happens next

The key requirement is: when code changes, the SKG must become consistent again, so the server’s answers match reality.

### Final strategy: incremental indexing (Cycle 10)

Trigger:
- Developer commits changes (or a file watcher sees changes)

Index update:
1. Detect changed files
   - e.g. via `git diff --name-only <last-indexed>...HEAD` or an on-disk “last indexed commit”
2. Re-run Code Parser only for the changed subset (and optionally dependent files)
3. Diff parser output:
   - new/updated/deleted symbols
4. Update SKG:
   - upsert updated nodes/edges
   - delete nodes/edges for removed symbols
   - keep traversal queries consistent (transaction + optional snapshot/versioning)
5. Update vector index (if enabled):
   - delete/refresh chunks only for changed symbols

Server behavior while indexing:
- Readers (MCP requests) should see either:
  - “old graph” while indexing happens, or
  - “new graph” after the transaction commits.
- The simplest guarantee is to batch writes in one transaction (`batch_commit`) so the update is atomic.

What happens today in this repo:
- Incremental indexing is a planned Cycle 10 item; the current `ctmint index` command is still stubbed.

---

## 6. Full example: end-to-end scenario

### Setup (manifest)
Assume `ctmint.yaml` describes:
- `auth-service` (language: Rust or Python)
- `user-service`

### Index
You run:

```bash
ctmint index --project ctmint.yaml
```

Final behavior (Cycle 3) for a simple “auth calls user” case:
- Parser finds:
  - a handler function in `auth-service`
  - a call from that handler to a function in `user-service` (resolved via call graph + imports, or via HTTP client naming heuristics)
- Indexer writes into SKG:
  - `Service` nodes for both services
  - `Function` nodes for the discovered functions
  - `CONTAINS` edges (Service->Module->Function)
  - `CALLS` edge (auth function -> user function)
  - optionally `Service->Service` `CALLS` edge for architecture map aggregation

### Serve and query
Run:
```bash
ctmint serve
```

Then your client calls `get_service_graph` for `auth-service`:
```json
{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"get_service_graph","arguments":{"service_name":"auth-service"}}}
```

Final response:
- JSON subgraph including:
  - `auth-service` service node
  - its modules/functions/endpoints
  - relevant outgoing edges (`CALLS`, `READS`, etc.) and their target nodes

---

## 7. “Finish the server” checklist (what needs to be wired)

To turn the currently-stubbed MCP tools into a usable system for real clients, wire the pipeline in this order:

1. Cycle 2: implement SQLite `GraphStore` + traversal helpers used by tools (`get_service_graph`, `get_architecture_map`)
2. Cycle 3: implement Code Parser + Code Indexer so `ctmint index` populates the SKG
3. Cycle 4: implement Vector chunking + semantic search tool (`search_code`)
4. Cycle 6: implement DB schema ingestion and (optionally) linking code -> tables (`READS/WRITES`)
5. Cycle 7: implement logs/traces ingestion and linking to SKG
6. Cycle 5 + Cycle 8 + Cycle 9:
   - Context funnel to assemble evidence
   - MCP capability routing for correct tool use
   - diagnose tools to produce an end-to-end answer

Even if only steps 1-3 are done, graph-based tools should already become meaningful (architecture + code navigation).

---

## 8. Quick “developer workflow” summary

1. Run `ctmint init` to generate/update `ctmint.yaml`
2. Run `ctmint index --project ctmint.yaml` after major changes (or `--incremental` later)
3. Run `ctmint serve` and have your client call MCP tools over stdio
4. When code changes:
   - rerun indexing
   - rely on incremental update logic to keep SKG consistent

