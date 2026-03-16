# Deployment and Languages – Technical Design (ContextMint)

## 1. Purpose and Scope

This document specifies **language choices**, **build and packaging**, **storage options**, and **deployment topologies** for ContextMint so that v1 is simple to install and run while leaving a clear path to scale. It aligns with the single-binary-first strategy and the hybrid Rust + Python approach for AI-heavy steps.

---

## 2. Language and Component Mapping

| Component              | Language   | Rationale |
|------------------------|-----------|-----------|
| Core binary (MCP server, orchestrator, capability router, SKG client, vector client, code indexer, code parser, runtime ingestion, data ingestion, context funnel, plugin bridge) | **Rust**   | Performance, single static binary, no runtime dependency, strong concurrency and safety; suitable for parsing, indexing, and I/O-heavy pipelines. |
| AI Setup Agent         | **Rust** (with optional LLM calls) or **Python** | If the agent uses a local LLM heavily and benefits from Python libs (Ollama, prompt templates), a small Python CLI or script is acceptable; otherwise implement the wizard in Rust with HTTP calls to an LLM API. |
| LLM calls (planning, summarization, capability classification) | **External** (Ollama, OpenAI, etc.) or **Rust HTTP client** | The core binary does not need to embed a model; it calls an LLM via HTTP. No Python required in the critical path if all reasoning is done via APIs. |
| Optional summarization jobs (e.g. Layer 2 semantic summaries) | **Rust** (HTTP to LLM) or **Python** batch job | Can be a separate small job (Python or Rust) that reads from SKG and writes summaries back; or integrated into the Rust indexer with an HTTP client. |

**Summary**: **Rust for the entire core** (parser, indexer, ingestion, graph, vector, MCP, orchestrator, funnel) with **HTTP-based LLM integration** keeps the main deployment to a single binary. Python is optional for Setup Agent or batch summarization if that simplifies prototyping.

---

## 3. Single Binary Contents (Rust)

The main binary (`ctmint` or `mcp-core`) should include:

- **CLI**: Subcommands such as `init`, `index`, `ingest`, `serve`, `status`.
- **MCP server**: Transport (stdio or HTTP) and tool dispatch.
- **Orchestrator**: Planner (with HTTP LLM calls), capability router, tool registry, plugin bridge.
- **Context Funnel**: All funnel layers; SKG and vector clients.
- **Code Parser**: Tree-sitter (or equivalent) based parsers for supported languages; normalized output schema.
- **Code Indexer**: Symbol graph builder, architecture map, SKG writer, vector chunk producer.
- **Runtime Ingestion**: Log/trace/metric adapters (file, OTLP, etc.); normalization; writers to SKG and log/trace store.
- **Data Ingestion**: Schema extractors (Postgres, MySQL, SQLite); SKG writer.
- **Knowledge Graph**: Query API and storage backend (embedded SQLite graph or client to Neo4j/Arango).
- **Vector Index**: Embedding client (HTTP to embedding API or local model) and vector store (embedded Qdrant or similar).
- **Plugins**: Built-in plugins (code, logs, traces, database) registered as capabilities; optional loading of external MCP server configs.

**Dependencies**: Tree-sitter grammars (bundled), SQLite, optional Qdrant/Neo4j clients, HTTP client, async runtime (e.g. Tokio), serde for config and MCP JSON.

---

## 4. Storage Choices

### 4.1 Embedded (v1, single node)

- **Graph**: SQLite with a **graph schema** (nodes + edges tables, indexed for traversal). No separate Neo4j/Arango required.
- **Vector**: Qdrant in **embedded mode** (single directory or file) or a minimal in-process store (e.g. file-backed FAISS) behind a trait so it can be swapped.
- **Log/Trace**: SQLite or append-only files with time-based partitioning; or “external” mode where the system only stores pointers and queries an external Loki/Tempo.

**Data directory**: Single directory (e.g. `~/.ctmint/` or `./.ctmint/`) for SQLite DBs, vector store, and config. Project-specific subdirs (e.g. `projects/<project_id>/`) for per-project state.

### 4.2 External (scale-out or existing infra)

- **Graph**: Neo4j or ArangoDB; config switch to use remote client instead of embedded SQLite.
- **Vector**: Qdrant/Weaviate/Pinecone as a service; same query API, different backend.
- **Log/Trace**: Loki, Tempo, or vendor; ingestion writes to those or only to SKG links; MCP tools query external APIs via adapters.

---

## 5. Configuration

- **Global config** (e.g. `~/.ctmint/config.toml` or env):
  - Data directory.
  - LLM endpoint (and API key if needed).
  - Embedding model endpoint.
  - Storage backend (embedded vs external URLs).
  - Plugin directories or external MCP server list.
- **Per-project config**: Project manifest (see `ai-setup-agent.md`, `mcp-core.md`) at e.g. `./ctmint.yaml` or `./.ctmint/project.yaml`, specifying repo, services, logs, tracing, database.

---

## 6. Deployment Topologies

### 6.1 Local / Single node (v1)

- **One process**: `ctmint serve` runs the MCP server and all pipelines (indexing and ingestion can be triggered by CLI or webhook).
- **User flow**: `curl | sh` install → `ctmint init` (optional AI setup) → `ctmint index` → `ctmint serve`; connect IDE or chat UI to the MCP endpoint.
- **Resource**: Single binary; one data directory; no k8s or multi-host required.

### 6.2 Single node with external stores

- Same as above but graph and/or vector (and optionally logs/traces) are external services. Useful when the team already runs Neo4j or Qdrant.
- Config points to those endpoints; binary stays the same.

### 6.3 Distributed (future)

- **Split services**:
  - `ctmint-core`: MCP server, orchestrator, context funnel, SKG/vector **clients** (no ingestion).
  - `ctmint-ingestor`: Log/trace (and optionally code re-index) ingestion; writes to shared graph and vector stores.
- **Shared storage**: Neo4j, Qdrant, and log/trace backend must be network-accessible. Project manifest and config can be shared via object store or config server.
- **Scaling**: Run multiple ingestors for throughput; keep a single (or few) core instances for MCP and reasoning.

---

## 7. Build and Release

- **Rust**: `cargo build --release`; strip binary for size. Target major platforms (linux, macOS, windows).
- **Artifacts**: Single binary; optional `.tar.gz` or installer script that places the binary and default config.
- **Versioning**: Semantic versioning; config and manifest schema versioned for compatibility (e.g. minimal supported manifest version in the binary).
- **Plugins**: Built-in plugins are compiled in; external MCP servers are separate processes and versioned independently.

---

## 8. Observability of the Platform

- **Logs**: Structured (JSON or key-value) to stderr or a log file; levels (info, warn, error) and component names.
- **Metrics**: Optional Prometheus metrics (e.g. request count, index duration, tool call latency) on an admin port or a separate endpoint.
- **Tracing**: Optional OpenTelemetry export so that ContextMint itself can be observed in the user’s observability stack.

---

## 9. Security Considerations

- **Secrets**: Never log connection strings or API keys; use env vars or a secret manager for DB and LLM keys.
- **Network**: MCP server can bind to localhost-only by default; optional TLS and auth for remote access.
- **Plugins**: External plugins run in separate processes; the core does not execute arbitrary code from manifests (only config and adapter selection).

---

## 10. Summary

- **Rust** implements the full core (MCP, orchestrator, funnel, parser, indexer, ingestion, graph, vector, plugins) for a **single, portable binary**.
- **Storage** is **embedded** (SQLite graph + embedded Qdrant or equivalent) in v1, with a clear path to **external** graph and vector stores and to **distributed** deployment (core + ingestors).
- **AI Setup Agent** can be implemented in Rust with an optional LLM over HTTP, or as a small Python wizard that only produces the manifest.
- **Deployment** starts as **single-node** with `ctmint init | index | serve`, and can evolve to multi-component and multi-node without changing the logical architecture described in `Overview.md` and the other design docs.
