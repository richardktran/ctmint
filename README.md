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

## Quick Start (when built)

```bash
# One-time setup: AI asks where code, logs, DB, tracing are (runs fully locally, no API keys)
ctmint init

# Index codebase and build graph + vector index
ctmint index

# Optional: ingest logs/traces (if configured in manifest)
ctmint ingest

# Start MCP server for your IDE or agent
ctmint serve
```

Then ask your MCP-connected agent: *“Why is auth-service failing?”* or *“Explain the login endpoint.”*

---

## Project Structure

| Path | Contents |
|------|----------|
| **`ideas/`** | Technical design docs: architecture overview, orchestrator, MCP core, code indexer/parser, knowledge graph, vector index, runtime/data ingestion, context funnel, plugins, AI setup agent, deployment. |
| **`planning/`** | Step-by-step implementation plan (MVP, iterative cycles). Each cycle has its own doc with tasks, deliverables, and definition of done. |
| **`planning/README.md`** | Cycle index and rationale (onboarding first, then graph → index → vector → funnel → runtime → diagnose). |

**Implementation order:** Cycle 0 (skeleton + contracts) → Cycle 1 (onboarding with embedded local AI) → Cycle 2 (SKG on SQLite) → Cycle 3 (code parser + indexer) → … → Cycle 9 (diagnose MVP) → Cycle 10 (hardening).

---

## Tech Stack (target)

- **Core:** Rust (single binary, MCP server, graph store, vector store, code indexer, ingestion, context funnel).
- **Onboarding:** Embedded small model (e.g. 1B–3B, GGUF) running in-process via Rust (e.g. candle / llama-cpp-2); no user AI credentials.
- **Storage (v1):** SQLite for the graph; embedded Qdrant (or similar) for vectors; SQLite or files for log/trace store.
- **Later:** Optional Neo4j/Arango, Qdrant as a service, Loki/Tempo adapters for scale-out.

---

## Status

This repo currently holds **design and implementation planning** only. Implementation follows the cycles in `planning/`. After Cycle 3 you get code navigation and graph queries; after Cycle 7 you get code + runtime + DB in one place; after Cycle 9 you get full *“Why is X failing?”* diagnosis.

---

## Documentation

- **Architecture and design:** `ideas/Overview.md` and the other docs in `ideas/`.
- **How to implement:** `planning/README.md` and the cycle docs in `planning/`.
