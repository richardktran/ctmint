# Cycle 2 — System Knowledge Graph (Embedded)

## Goal

Implement the **System Knowledge Graph** using **embedded storage** (SQLite) and a small **traversal API**. After this cycle, we can store and query nodes (Service, Module, Function, Endpoint, etc.) and edges (CALLS, IMPLEMENTS, CONTAINS, etc.) so that the code indexer (Cycle 3) and MCP tools can read topology and dependencies.

---

## Prerequisites

- Cycle 0 done: GraphStore trait and Node/Edge types defined.
- Cycle 1 done: onboarding produces a manifest; we will use `project_id` and (later) service names from it.
- No dependency on code parser yet; we can populate the graph with synthetic data for testing.

---

## Tasks (Step-by-Step)

### 1. SQLite schema

- [ ] Create **nodes** table:
  - `id` (primary key), `type` (string), `project_id` (string), `attrs` (JSON text), `created_at`, `updated_at`.
  - Index on `(type, project_id)` and on `id`.
- [ ] Create **edges** table:
  - `source_id`, `target_id`, `type` (string), `project_id`, `attrs` (JSON text), `created_at`, `updated_at`.
  - Index on `(source_id, type)`, `(target_id, type)`, and `(project_id)`.
- [ ] Migration or init script that creates these tables if they do not exist (e.g. on first open of the DB file).
- [ ] DB file path: from global config `data_dir` (e.g. `{data_dir}/graph.db`).

### 2. GraphStore implementation

- [ ] Implement the **GraphStore** trait (from Cycle 0) with a struct that holds a SQLite connection (or connection pool).
- [ ] **upsert_node**: INSERT or REPLACE into `nodes` by `id`; set `updated_at`.
- [ ] **upsert_edge**: INSERT or REPLACE into `edges` by `(source_id, target_id, type)`; set `updated_at`.
- [ ] **get_node(id)**: SELECT by id; deserialize `attrs` JSON into a map or typed struct.
- [ ] **get_neighbors(node_id, edge_type, direction)**: 
  - If Outgoing: SELECT from edges where source_id = ? and (type = ? or type is null); join nodes on target_id.
  - If Incoming: same with target_id and source_id.
  - Return list of nodes.
- [ ] **batch_commit(nodes, edges)**: run upserts in a transaction for consistency.
- [ ] **Optional**: `delete_node(id)` and cascade or delete edges where source or target = id (for re-indexing).

### 3. Query helpers (used by funnel and MCP)

- [ ] **get_architecture_map(project_id)**: Return all Service nodes and Service→Service edges (edge type CALLS or DEPENDS_ON). Format as list of (from_name, to_name) or a small graph struct.
- [ ] **get_service_graph(service_name, project_id)**: 
  - Find Service node by name (and project_id).
  - One-hop: get all outgoing and incoming edges (CALLS, CONTAINS, IMPLEMENTS, etc.) and related nodes.
  - Return a subgraph (nodes + edges) for that service.
- [ ] **get_node_by_name(type, name, project_id)**: SELECT from nodes where type = ? and attrs->name = ? and project_id = ? (syntax depends on SQLite JSON support).
- [ ] Add **bounded traversal** (e.g. 2–3 hops) if needed for “all dependencies of service X”: implement as repeated get_neighbors with a visited set and max depth.

### 4. CLI commands

- [ ] **ctmint graph load-sample**: 
  - Insert a small synthetic graph: 2–3 services, a few functions, CONTAINS and CALLS edges.
  - Use a fixed project_id (e.g. `sample`).
- [ ] **ctmint graph query --service <name> [--project <id>]**:
  - Call get_service_graph and print nodes and edges (readable text or JSON).
- [ ] **ctmint graph list-services [--project <id>]**:
  - List all nodes of type Service for the project.
- [ ] Project id: from `--project` or from default manifest at `./ctmint.yaml`.

### 5. MCP tools (read-only)

- [ ] **get_architecture_map**: No args or project_id. Call get_architecture_map, return JSON (nodes + edges).
- [ ] **get_service_graph(service_name)**: Call get_service_graph, return JSON. Tool schema: input `service_name` (string).
- [ ] Wire these tools in the MCP server so a client can invoke them; they must read from the real GraphStore implementation (using data_dir from config).

### 6. Integration with config

- [ ] On startup, open or create SQLite DB at `config.data_dir/graph.db`.
- [ ] Ensure project_id is always set when writing from manifest (e.g. from `project` field in ctmint.yaml).

### 7. Tests

- [ ] Unit tests: create in-memory SQLite DB, insert nodes/edges, run get_node, get_neighbors, get_architecture_map; assert results.
- [ ] Test load-sample then query: after load-sample, get_service_graph("auth-service") returns expected nodes and edges.
- [ ] Test concurrent read while write: one thread does batch_commit, another does get_neighbors; no panic and read sees consistent state (eventually).

---

## Deliverables

- SQLite-backed GraphStore satisfying the Cycle 0 trait.
- CLI: `ctmint graph load-sample`, `ctmint graph query --service X`, `ctmint graph list-services`.
- MCP tools: `get_architecture_map`, `get_service_graph(service_name)` returning real data from the DB.
- Tests: graph operations and at least one end-to-end (load-sample → query).

---

## Definition of Done

- [ ] You can run `ctmint graph load-sample` and then `ctmint graph query --service auth-service` and see a consistent subgraph.
- [ ] MCP client can call `get_architecture_map` and `get_service_graph` and receive valid JSON.
- [ ] Graph queries complete in under ~100 ms for graphs with hundreds of nodes (local SQLite).
- [ ] No runtime or log/trace nodes required yet; focus on code-domain nodes (Service, Module, Function, Endpoint) and edges.

---

## Acceptance Criteria

- Code indexer (Cycle 3) can use this GraphStore to write nodes/edges and the same API to verify what was written.
- Orchestrator and context funnel (later) can call get_architecture_map and get_service_graph without knowing the storage backend.

---

## Estimated Duration

3–5 days.
