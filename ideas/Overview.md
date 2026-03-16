## ContextMint (ctmint) – Technical Architecture Overview

### 1. Problem & Core Idea

**Problem**: Modern systems have:
- **Huge codebases** (millions of LOC)
- **Complex microservice topologies** (tens–hundreds of services)
- **Scattered runtime data** (logs, traces, metrics, DB schemas, network flows)

Existing tools (Datadog, Grafana, Sentry, Sourcegraph, etc.) typically cover **only one layer** (observability, code search, error tracking) and do **not unify code + runtime + data** into a reasoning-ready model for AI.

**Core Idea (ContextMint)**:
- Build a **System Knowledge Graph (SKG)** that unifies:
  - **Code** (services, endpoints, functions, dependencies, data access)
  - **Runtime** (logs, traces, metrics)
  - **Data** (schemas, tables, columns, indexes)
  - (Optionally) **Network** (devices, flows, endpoints)
- Add a **Context Funnel** that compresses this entire system into **problem-sized context** suitable for LLMs.
- Expose everything through a **Model Context Protocol (MCP) based orchestrator** with a **capability‑based plugin system**, ideally in a **single binary** that’s easy to install.

Result: an **AI SRE / AI System Debugger** that can answer:
- “**Why is `login` API slow today?**”
- “**Why is `payment-service` returning 500?**”
- “**Which services will be impacted if I change this table/field?**”

…by traversing the SKG + retrieving the minimum necessary evidence instead of reading the whole system.

---

### 2. Conceptual Knowledge Model

At the heart of ContextMint is a **normalized system model** shared across projects and languages.

**Core entities**:
- **Service**: logical or deployable unit (e.g. `auth-service`, `payment-service`)
- **Endpoint**: externally visible API surface (e.g. `POST /login`)
- **Module / Class / Function**: code symbols
- **DatabaseTable / Column / Index**
- **LogEvent**
- **Trace / Span**
- **Metric**
- **NetworkNode / NetworkFlow** (optional/advanced)

**Core relations**:
- `Service -[:CALLS]-> Service`
- `Service -[:IMPLEMENTS]-> Endpoint`
- `Function -[:BELONGS_TO]-> Service`
- `Function -[:READS]-> DatabaseTable`
- `Function -[:WRITES]-> DatabaseTable`
- `Service -[:PRODUCES_LOG]-> LogEvent`
- `Service -[:HAS_TRACE]-> Trace`
- `Trace -[:HAS_SPAN]-> Span`
- `Service -[:DEPENDS_ON]-> Service | Cache | ExternalAPI`

This model is stored in a **graph store** and is the primary substrate for reasoning and context selection.

---

### 3. High-Level System Architecture

At a high level, ContextMint is a **data pipeline + graph store + retrieval layer + MCP orchestrator**:

```text
                 AI Agent / IDE / Chat UI
                           │
                           ▼
                      MCP Orchestrator
                  (planner + context funnel)
                           │
         ┌─────────────────┼──────────────────┐
         │                 │                  │
   System Knowledge   Vector Retrieval   Plugin Capabilities
       Graph               Layer            (code/logs/db/…)
         │                 │                  │
         ▼                 ▼                  ▼
   Graph Database      Vector DB        MCP Plugins / Adapters
         │
         ▼
                 Data Ingestion Pipeline
         ┌───────────────┬───────────────┬───────────────┐
         │               │               │               │
     Code Parser     Log Ingestor   Trace Ingestor   DB Schema Extractor
     (multi-lang)      (stream)        (OTel)          (Postgres/…)
```

**Key properties**:
- **Graph‑first, vector‑augmented**:
  - Graph narrows the search space structurally.
  - Vector search retrieves semantically relevant snippets (code, logs, docs) **within** that narrowed set.
- **Context funnel**:
  - Multi-layer reduction: system → entities → symbols → evidence → compressed summaries.
- **Single binary UX** (v1):
  - One executable with embedded storage, but internally modular by component.

---

### 4. Major Components

Each of the following has its own design document:

- **Orchestrator / MCP Core** (`orchestrator.md`, `mcp-core.md`)
  - Owns planning, tool routing, capability routing, context building and fusion.
  - Presents a single MCP surface to agents and tools.

- **Code Intelligence Pipeline** (`code-indexer.md`, `code-parser.md`)
  - Parses repositories into ASTs and symbol graphs.
  - Builds architecture maps and semantic summaries (multi‑level code representation).

- **Runtime & Data Ingestion** (`runtime-ingestion.md`, `data-ingestion.md`)
  - Ingests logs, traces, and metrics from systems like OpenTelemetry/Prometheus.
  - Extracts database schemas and usage (tables, columns, indexes, FK).

- **System Knowledge Graph** (`knowledge-graph.md`)
  - Normalizes all inputs into the common entity/relation model.
  - Stores in a graph DB (or graph‑like schema on embedded storage).

- **Vector Retrieval Layer** (`vector-index.md`)
  - Stores embeddings for code chunks, log snippets, traces, and docs.
  - Used **after** graph has constrained the candidate set.

- **Capability-Based Plugin System** (`plugins-and-capabilities.md`)
  - Lets plugins declare **capabilities** (e.g. `logs`, `traces`, `kubernetes`), not just tools.
  - Capability router chooses which plugins and tools to expose for a given query.

- **Context Funnel Engine** (`context-funnel.md`)
  - Implements layered reduction from system‑wide data down to a compact, task‑specific prompt.

- **AI Setup / Onboarding Agent** (`ai-setup-agent.md`)
  - Guides installation and project onboarding (detects stack, generates manifest + adapters).

- **Deployment & Language Stack** (`deployment-and-languages.md`)
  - Describes language choices, binaries, storage options, and scaling path.

---

### 5. Language & Technology Choices (Summary)

**Guiding principles**:
- v1 prioritizes:
  - **Fast iteration**
  - **Easy distribution** (single binary or simple container)
  - **Leverage existing AI ecosystems**
- Long‑term:
  - Split heavy data processing and infra‑style workloads into **Rust/Go** for performance.

**Recommended initial stack**:
- **Single‑binary core**: **Rust**
  - Modules:
    - Code indexer (AST + symbol graph via Tree‑sitter)
    - Log/trace ingestion
    - Embedded graph & vector storage abstraction
    - Capability router + MCP server
  - Benefits: performance, static binary, infra‑grade reliability.

- **AI orchestration + setup agent**: **Python**, external but tightly integrated
  - Runs LLM calls (remote or local, e.g. via Ollama).
  - Implements AI Setup Agent and some summarization pipelines if not done directly from Rust.

- **Storage**:
  - **Graph**:
    - v1: graph schema over **SQLite** (or lightweight embedded graph engine).
    - Later: optional Neo4j/Arango integration for large deployments.
  - **Vector**:
    - v1: embedded Qdrant or similar; fallback to file‑backed FAISS for local single‑node usage.

This gives a practical balance between **distributable single binary** and **AI‑heavy workflows** that benefit from Python tooling.

---

### 6. Deployment Model

**Local / Single-node (v1)**:
- Distributed as a **single Rust binary** (`ctmint` or `mcp-core`) running:
  - Embedded HTTP MCP server
  - Embedded graph + vector stores
  - Internal modules for code/runtime ingestion and context funnel
- Optional sidecar Python process started on demand for:
  - AI Setup Agent
  - Summarization jobs, if not handled by an external LLM API directly from Rust

Developer UX:

```text
curl -sSL <install-url> | sh
ctmint init           # runs AI setup agent and detects project
ctmint index          # build or refresh SKG & indices
ctmint serve          # start MCP server for IDE/agents
```

**Scalable / Distributed (later versions)**:
- Split into:
  - `ctmint-core` (planner + MCP + context funnel)
  - `ctmint-ingestor` (high‑throughput logs/traces ingestion)
  - Optional external Neo4j/Qdrant instances
- Retain the same logical architecture; only change the deployment topology.

---

### 7. End-to-End Debugging Flow (Conceptual)

Example question:

> “Why is `POST /login` slow today?”

High-level reasoning pipeline:

```text
User Question
   │
   ▼
MCP Orchestrator
   │  (LLM planner + capability router)
   ▼
Context Funnel
   │
   ├─ Graph narrowing:
   │     endpoint → service → dependencies
   │
   ├─ Symbol selection:
   │     relevant functions, modules, DB tables
   │
   ├─ Runtime evidence:
   │     recent traces, error logs, latency metrics
   │
   └─ Compression:
         structured summaries + minimal raw snippets
   ▼
LLM reasoning
   ▼
Natural language diagnosis + optional suggested fix
```

---

### 8. Document Map

The rest of the design is split into focused documents:
- `orchestrator.md` – overall orchestrator responsibilities and flows
- `mcp-core.md` – MCP interface, tool contracts, and planning
- `code-indexer.md` – indexing pipeline, symbol graph, architecture map
- `code-parser.md` – multi‑language parsing strategy (Tree‑sitter, analyzers)
- `runtime-ingestion.md` – logs/traces/metrics ingestion and normalization
- `data-ingestion.md` – database/schema extraction and mapping
- `knowledge-graph.md` – SKG schema, storage, and query patterns
- `vector-index.md` – embeddings, retrieval strategy, and hybrid graph+vector
- `plugins-and-capabilities.md` – plugin lifecycle, capability routing, tool funnel
- `context-funnel.md` – multi‑layer context reduction and prompt construction
- `ai-setup-agent.md` – onboarding flows, manifest generation, adapter scaffolding
- `deployment-and-languages.md` – deeper dive into stack, performance, and evolution

Each file will expand this overview into implementation‑ready technical detail.

