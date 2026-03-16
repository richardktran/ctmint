# ContextMint (ctmint)

**AI-native system debugger** — unify code, runtime (logs/traces), and data (DB schema) into a **System Knowledge Graph**, then let an AI agent answer questions like *“Why is the login API slow?”* or *“Why is payment-service returning 500?”* by traversing the graph and retrieving only the relevant evidence.

---

## The Problem

Modern systems have huge codebases, complex microservice topologies, and scattered runtime data. Tools like Datadog, Grafana, Sentry, and Sourcegraph each cover **one layer** (observability, code search, errors) but don’t unify **code + runtime + data** into something an AI can reason over. ContextMint builds that unified layer and exposes it via **Model Context Protocol (MCP)** so any AI agent or IDE can use it.

---

## What ContextMint Does

- **System Knowledge Graph (SKG)** — One graph of services, endpoints, functions, database tables, logs, and traces, with relations like `CALLS`, `IMPLEMENTS`, `READS`, `WRITES`, `PRODUCES_LOG`.
- **Graph-first, vector-augmented retrieval** — The graph narrows *where* to look; semantic (vector) search finds *what* is relevant inside that scope. So the AI gets problem-sized context, not the whole repo.
- **Context Funnel** — Multi-step reduction: raw system → graph scope → symbol summaries → runtime evidence → compressed prompt. Fits millions of LOC and GB of logs into a few thousand tokens.
- **MCP orchestrator** — Single MCP server with tools such as `get_service_graph`, `search_logs`, `query_traces`, `get_db_schema`, `diagnose_service`, `diagnose_endpoint`. Capability-based routing exposes only the tools needed for each question.
- **Onboarding first** — An **embedded small AI model** (runs locally, no API keys) guides setup: where is source code, where are logs, DB connection, tracing. Output is a **project manifest** (`ctmint.yaml`) that every other component uses. No hardcoded paths or credentials.
- **Single binary (v1)** — One executable with embedded graph (SQLite) and vector store; optional external Neo4j/Qdrant later for scale.

---

## High-Level Architecture

```
     AI Agent / IDE / Chat UI
                │
                ▼
           MCP Orchestrator (planner + context funnel)
                │
    ┌───────────┼───────────┐
    │           │           │
 SKG (graph)  Vector DB   Plugins (code, logs, traces, db)
    │           │           │
    └───────────┼───────────┘
                │
    Data pipeline: Code parser → Indexer | Log/Trace ingest | DB schema
```

All of this is driven by the **manifest** produced at onboarding: repo paths, log locations, DB and tracing config.

---

## Quick Start

**Prerequisites:** Rust toolchain (`rustup`).

```bash
# Build the workspace
cargo build

# Run the CLI
cargo run -- --help
```

**What works today (Cycle 0):**

```bash
# Stub: prints that AI onboarding is not implemented yet
ctmint init [--path .] [--output ctmint.yaml]

# Loads ctmint.yaml and lists project + services (indexing not implemented yet)
ctmint index [--project ctmint.yaml]

# Lists services from manifest; graph-based listing not implemented yet
ctmint graph list-services [--project ctmint.yaml]
ctmint graph query --service auth-service [--project ctmint.yaml]

# Start MCP server on stdio — lists 10 tools, tool calls return "Not implemented"
ctmint serve
```

A sample manifest is at **`ctmint.yaml`**. Use it to try `ctmint index` and `ctmint graph list-services`. To talk to the MCP server, send one JSON-RPC message per line on stdin (e.g. `initialize`, `tools/list`, `tools/call` with `name` and `arguments`).

---

## Current Implementation (Cycle 0)

| Component | Status |
|-----------|--------|
| **Multi-crate workspace** | ✅ 5 crates: `ctmint-core`, `ctmint-config`, `ctmint-storage`, `ctmint-mcp`, `ctmint-cli` |
| **Core data model** | ✅ `Node` / `Edge` with `NodeType`, `EdgeType`; stable ID helpers; `VectorMetadata`, `SearchResult`, `SearchFilters` |
| **Manifest** | ✅ `ProjectManifest` schema (project, services, logs, database, tracing); load/validate from YAML; `ctmint.yaml` sample |
| **Global config** | ✅ `GlobalConfig` (data_dir, optional LLM/embedding endpoints); file + env overrides |
| **Storage traits** | ✅ `GraphStore` and `VectorStore` async traits; **in-memory** implementations for tests and stubs |
| **CLI** | ✅ `init`, `index`, `graph list-services`, `graph query`, `serve` (stubs where logic is not yet built) |
| **MCP server** | ✅ Stdio JSON-RPC server; `initialize`, `tools/list`, `tools/call`; 10 stub tools; tool calls return “Not implemented” |
| **Tests** | ✅ 19 unit tests (manifest parse/validate, graph CRUD, vector search, MCP handlers) |

**Not yet implemented:** AI onboarding (Cycle 1), SQLite graph store (Cycle 2), code parser/indexer (Cycle 3), vector index, context funnel, real tool implementations.

---

## Project Structure

| Path | Contents |
|------|----------|
| **`crates/`** | Rust workspace (multi-crate). |
| **`crates/ctmint-core`** | Data model: graph types, IDs, vector metadata, errors. |
| **`crates/ctmint-config`** | Manifest and global config schemas, YAML/TOML loading, validation. |
| **`crates/ctmint-storage`** | `GraphStore` and `VectorStore` traits; in-memory implementations. |
| **`crates/ctmint-mcp`** | MCP server (stdio), JSON-RPC, stub tool definitions and handlers. |
| **`crates/ctmint-cli`** | `ctmint` binary; subcommands for init, index, graph, serve. |
| **`ctmint.yaml`** | Sample project manifest (used by `index` and `graph`). |
| **`ideas/`** | Technical design: architecture, orchestrator, MCP core, code indexer/parser, knowledge graph, vector index, runtime/data ingestion, context funnel, plugins, AI setup agent, deployment. |
| **`planning/`** | Step-by-step implementation plan (MVP cycles). Each cycle has its own doc. |

**Implementation order:** Cycle 0 ✅ → Cycle 1 (onboarding + embedded local AI) → Cycle 2 (SKG on SQLite) → Cycle 3 (code parser + indexer) → … → Cycle 9 (diagnose MVP) → Cycle 10 (hardening).

---

## Tech Stack

- **Core:** Rust; single `ctmint` binary from workspace; MCP server, config, and storage contracts in separate crates.
- **Storage (current):** In-memory graph and vector stores (Cycle 0). **Target v1:** SQLite for graph; embedded vector store (e.g. Qdrant/FAISS); optional Loki/Tempo for logs/traces.
- **Onboarding (planned):** Embedded small model (1B–3B, GGUF) in-process; no user AI credentials.
- **Later:** Optional Neo4j/Arango, Qdrant as a service for scale-out.

---

## Status

**Cycle 0 is done.** The repo has a runnable CLI and MCP server stub, loadable manifest and config, and storage traits with in-memory implementations. Design and planning live in `ideas/` and `planning/`. Next: Cycle 1 (AI-guided onboarding producing `ctmint.yaml`).

---

## Documentation

- **Architecture and design:** `ideas/Overview.md` and the other docs in `ideas/`.
- **How to implement:** `planning/README.md` and the cycle docs in `planning/`.
