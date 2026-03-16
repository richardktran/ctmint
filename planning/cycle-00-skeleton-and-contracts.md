# Cycle 0 — Project Skeleton and Contracts

## Goal

Establish the project structure, core data model, storage interfaces, and **manifest schema** so that onboarding (Cycle 1) can generate a valid config and all later cycles can implement against stable contracts.

---

## Prerequisites

- Rust toolchain (or chosen language) and repo initialized.
- No prior ContextMint code required.

---

## Tasks (Step-by-Step)

### 1. Repo layout

- [ ] Create directory layout, e.g.:
  - `src/` (or `crates/` if multi-crate)
  - `src/core/` — data model, types
  - `src/storage/` — graph and vector traits
  - `src/config/` — manifest and global config
  - `src/cli/` — CLI entry and subcommands
  - `src/mcp/` — MCP server and tool stubs
- [ ] Add dependency manifest (Cargo.toml or equivalent) with:
  - CLI framework (e.g. clap)
  - serde (or equivalent) for JSON/YAML
  - async runtime (e.g. tokio) if needed later
  - (Optional) SQLite driver for Cycle 2

### 2. Manifest schema (project config)

- [ ] Define **project manifest** schema that onboarding will produce. At minimum:
  - `project`: string (project id/name)
  - `services`: list of:
    - `name`: string
    - `repo_path`: string (path to repo or service dir)
    - `language`: string (e.g. python, rust, go)
  - `logs`: optional
    - `provider`: string (file | loki | otel | none)
    - `path` or `endpoint` or `url`: string (as needed)
    - `format`: optional (json | jsonl | text)
  - `database`: optional
    - `type`: string (postgres | mysql | sqlite | none)
    - `connection`: string (URL or `${ENV_VAR}`)
    - `schema`: optional string
  - `tracing`: optional
    - `provider`: string (otel | jaeger | zipkin | none)
    - `endpoint`: string
- [ ] Implement **parsing and validation** for this schema (e.g. load from `ctmint.yaml` or `./.ctmint/project.yaml`).
- [ ] Document the schema (in code or in `docs/manifest-schema.md`).

### 3. Global config schema

- [ ] Define **global config** (e.g. `~/.ctmint/config.toml` or env):
  - `data_dir`: where to store SQLite, vector store, caches
  - (Later) `llm_endpoint`, `embedding_endpoint`, etc.
- [ ] Implement load from file + env overrides.

### 4. Core data model (types only)

- [ ] Define **node types** as an enum or sealed type: e.g. `Service`, `Module`, `Function`, `Endpoint`, `Database`, `DatabaseTable`, `Column`, `LogEvent`, `Trace`, `Span`.
- [ ] Define **edge types**: e.g. `CONTAINS`, `CALLS`, `IMPLEMENTS`, `READS`, `WRITES`, `IMPORTS`, `DEPENDS_ON`, `PRODUCES_LOG`, `HAS_TRACE`.
- [ ] Define **Node** and **Edge** structs with:
  - `id`, `type`, `project_id`, attributes (map or typed struct)
  - For Edge: `source_id`, `target_id`, `type`
- [ ] Define **stable ID** conventions (e.g. `service:{name}`, `func:{service}::{name}`) in a short doc or comment.

### 5. Storage interfaces (traits)

- [ ] Define **GraphStore** trait (or interface) with:
  - `upsert_node(node: Node) -> Result<()>`
  - `upsert_edge(edge: Edge) -> Result<()>`
  - `get_node(id: &str) -> Result<Option<Node>>`
  - `get_neighbors(node_id: &str, edge_type: Option<EdgeType>, direction: Incoming|Outgoing) -> Result<Vec<Node>>`
  - `batch_commit(nodes: Vec<Node>, edges: Vec<Edge>) -> Result<()>`
- [ ] Define **VectorStore** trait with:
  - `upsert(id: &str, vector: &[f32], metadata: &Metadata) -> Result<()>`
  - `search(vector: &[f32], filters: &Filters, top_k: usize) -> Result<Vec<SearchResult>>`
- [ ] Add a **Metadata** type that includes at least `project_id`, `service_id`, `type` (code|log|trace).

### 6. CLI skeleton

- [ ] Implement main entry: `ctmint [SUBCOMMAND]`.
- [ ] Subcommands (stubs that print or return):
  - `ctmint init` — “Run onboarding (not implemented yet).”
  - `ctmint index` — “Index codebase (not implemented yet).”
  - `ctmint graph ...` — placeholder for graph commands.
  - `ctmint serve` — “Start MCP server (not implemented yet).”
- [ ] `ctmint --help` and `ctmint init --help` show clear descriptions.
- [ ] If a project manifest path is needed, support `--project ./ctmint.yaml` or default to `./ctmint.yaml` / `./.ctmint/project.yaml`.

### 7. MCP server skeleton

- [ ] Start an MCP server (stdio or HTTP) that:
  - Lists tools (stub list): e.g. `get_architecture_map`, `get_service_graph`, `get_code_snippet`, `search_code`.
  - On tool call, returns a fixed response: “Not implemented.”
- [ ] Tool schemas (name, description, input_schema) match the planned design in `mcp-core.md`.
- [ ] Document how to connect a client (e.g. IDE) to the server for later testing.

### 8. Tests and docs

- [ ] Unit test: load a sample manifest YAML and assert parsed fields.
- [ ] Unit test: create in-memory or no-op implementations of GraphStore and VectorStore that satisfy the traits (for compile-time and contract tests).
- [ ] README section or `docs/architecture.md` that points to manifest schema and storage contracts.

---

## Deliverables

- Runnable binary: `ctmint --help`, `ctmint init`, `ctmint index`, `ctmint serve` (stubs).
- Manifest schema defined and loadable; at least one sample `ctmint.yaml` in repo.
- GraphStore and VectorStore traits defined; one no-op or in-memory implementation each.
- MCP server that lists tools and returns “Not implemented” on call.

---

## Definition of Done

- [ ] You can run `ctmint --help` and see all subcommands.
- [ ] You can load a valid `ctmint.yaml` and read `project`, `services`, `logs`, `database`, `tracing`.
- [ ] New graph or vector implementation can be added by implementing the existing traits without changing callers.
- [ ] MCP client can list tools and call one; response is explicit “Not implemented.”

---

## Acceptance Criteria

- Code compiles and passes the unit tests above.
- A new developer can read the manifest schema and storage traits and understand what Cycle 1 (onboarding) must produce and what Cycle 2 (graph) must implement.

---

## Estimated Duration

1–2 days (depending on familiarity with the stack).
