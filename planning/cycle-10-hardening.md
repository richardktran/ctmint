# Cycle 10 — Hardening, Quality, and Scale

## Goal

Improve **reliability**, **security**, **observability**, and **scalability** of ContextMint so it is production-friendly. This cycle is **ongoing** and can be split into multiple sprints; the tasks below are a checklist to prioritize.

---

## Prerequisites

- Cycles 0–9 done: full flow from onboarding → graph → index → vector → funnel → runtime → diagnose works end-to-end.

---

## Tasks (Step-by-Step)

### 1. Incremental indexing

- [ ] **Detect changed files**: Use git (e.g. `git diff --name-only HEAD~1` or file watcher) or manifest of “last indexed commit” to get list of changed paths.
- [ ] **Re-parse only changed files** (and optionally files that depend on them by import). Run parser on that subset.
- [ ] **Diff symbol output**: Compare with previous run (e.g. previous symbol list per file stored in a side table or file). Compute: new symbols, updated symbols, deleted symbols.
- [ ] **Update graph**: Upsert new/updated nodes and edges; **delete** nodes that no longer exist (e.g. function removed). Remove edges pointing to deleted nodes.
- [ ] **Update vector index**: Delete chunks for removed symbols; add/update chunks for new or changed symbols. Re-embed only changed chunks.
- [ ] **CLI**: `ctmint index --incremental` or make incremental the default when a previous index state exists.
- [ ] **Tests**: Change one file in fixture repo; run incremental index; assert graph and vector store reflect only the change.

### 2. Graph consistency and versioning

- [ ] **Snapshot**: Optionally tag the graph state after each full or incremental index (e.g. “index_version” or “commit_sha”). Readers (funnel, MCP) can pin to a version for consistent reads during a request.
- [ ] **Transactions**: Ensure batch_commit is atomic (single transaction); if the process crashes mid-write, next run can repair or re-index from scratch.
- [ ] **Consistency check**: Optional CLI `ctmint graph verify` that checks for orphan edges (target node missing) and reports inconsistencies.

### 3. Caching

- [ ] **Architecture map**: Cache get_architecture_map result per project (in-memory or small cache file); invalidate on next index.
- [ ] **Service graph**: Cache get_service_graph(service_name) per service; invalidate when that service is re-indexed.
- [ ] **Summaries**: If you add LLM-generated function summaries (Cycle 5 or later), cache them by symbol_id; invalidate when the function’s code changes (e.g. content hash).
- [ ] **Config**: TTL or invalidation rules in config; document cache behavior in README.

### 4. Security

- [ ] **Secrets**: Never log connection strings, API keys, or passwords. Use env vars (e.g. `${DATABASE_URL}`) in manifest; resolve only at runtime and do not echo.
- [ ] **Redaction**: Optional redaction of log content (e.g. mask credit card patterns, tokens) before storing or sending to LLM. Config: enable/disable, list of patterns.
- [ ] **MCP server**: Bind to localhost by default; optional TLS and auth (e.g. API key or mTLS) for remote access. Document security considerations in README.
- [ ] **Least privilege**: DB user used for schema extraction is read-only; log ingestion only reads log files or receives OTLP (no write to user’s app).

### 5. Observability of ContextMint

- [ ] **Structured logging**: All components log with level, component name, and structured fields (e.g. project_id, service_id, duration). Use a single logging facade.
- [ ] **Metrics** (optional): Expose Prometheus metrics on an admin port or file: e.g. index_duration_seconds, graph_queries_total, vector_search_latency_seconds, tool_calls_total, llm_requests_total. Document endpoint.
- [ ] **Tracing**: Optional OpenTelemetry tracing for requests (e.g. one span per MCP request, child spans for graph query, vector search, LLM call). Helps debug “why is diagnose slow?”
- [ ] **Health check**: Optional `GET /health` or `ctmint health` that checks: config loaded, graph DB reachable, vector store reachable, optional LLM ping. Return 200 or 503.

### 6. External backends (scale-out)

- [ ] **Graph**: Support switching from embedded SQLite to **Neo4j** or **ArangoDB** via config (e.g. `graph.backend: neo4j`, `graph.url: ...`). Same GraphStore trait; different implementation. Migrate or re-index to populate external DB.
- [ ] **Vector**: Support **Qdrant** or **Weaviate** as a remote service; same VectorStore trait. Optional: run vector build to push existing chunks to remote.
- [ ] **Log/trace**: Support **Loki** and **Tempo** (or Jaeger) as query backends instead of local SQLite; ingestion writes to those, or we only query them via adapter. Document config and limitations.
- [ ] **Document** deployment topology: single-node (embedded) vs multi-node (external graph + vector + optional ingestors).

### 7. Better call graph and endpoint detection

- [ ] **Call graph**: Improve parser to resolve cross-file and cross-module calls (e.g. type-based or import-based resolution) so CALLS edges are more complete.
- [ ] **Endpoint detection**: Add more frameworks (e.g. Spring, Express, Actix) so Endpoint nodes and IMPLEMENTS edges cover more repos. Document supported frameworks per language.
- [ ] **Service boundaries**: Support monorepo with multiple services (e.g. one service per directory under `services/`); ensure architecture map reflects real service-to-service calls from code or config.

### 8. Documentation and ops

- [ ] **README**: Quick start (init → index → ingest → serve), config reference, manifest schema, and troubleshooting (e.g. “no logs” → check manifest and ingest).
- [ ] **Runbook**: How to re-index from scratch, how to add a new project, how to upgrade schema (graph/vector) if you change node types.
- [ ] **Changelog**: Maintain a CHANGELOG for releases; note breaking changes (e.g. manifest schema version).

### 9. Performance and limits

- [ ] **Timeouts**: All external calls (LLM, embedding API, DB, OTLP) have timeouts and retries with backoff. Document limits.
- [ ] **Rate limits**: Optional rate limit on MCP tool calls or diagnose requests per project to avoid abuse.
- [ ] **Index scale**: Test index on a repo with 100k+ LOC; tune batch size and parallelism so index completes in reasonable time (e.g. under 10 minutes). Document recommended limits (e.g. “tested up to 500k LOC”).

### 10. Tests and CI

- [ ] **CI pipeline**: On every push, run unit tests and integration tests (fixture repos, optional testcontainers for Postgres). No flaky tests.
- [ ] **E2E test**: Optional: script that runs init (with fixture manifest) → index → vector build → ingest (with fixture logs) → serve → call diagnose_service → assert response contains expected evidence. Run in CI or nightly.
- [ ] **Regression**: When adding a new feature, add at least one test that would have caught a bug you fixed in the past.

---

## Deliverables

- Incremental index (optional but recommended).
- Graph consistency and optional versioning.
- Caching for architecture map and service graph (and summaries if applicable).
- Security: no secrets in logs; optional redaction and MCP auth.
- Observability: structured logs, optional metrics and tracing, health check.
- Optional: external graph and vector backends; Loki/Tempo adapters.
- Improved call graph and endpoint detection (document supported stacks).
- README, runbook, changelog; CI with tests and optional E2E.

---

## Definition of Done

- [ ] A new developer can clone, read README, run init → index → serve, and get a working MCP server without guessing.
- [ ] Re-indexing a large repo does not require full rescan every time (incremental path works).
- [ ] No credentials in logs; MCP can be exposed with documented auth option.
- [ ] If something is slow or broken, logs and optional metrics give enough signal to debug.

---

## Acceptance Criteria

- All acceptance criteria from previous cycles still pass after hardening changes.
- At least one E2E test (or manual demo) runs from init through diagnose with fixture data and passes consistently.

---

## Estimated Duration

Ongoing; 2–4 weeks for the first hardening pass (incremental index, security, observability, docs). External backends and advanced call graph can be follow-up sprints.
