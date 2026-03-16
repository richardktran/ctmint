# ContextMint Implementation Plan — Detailed Cycles

## Rationale: Onboarding First

We **run onboarding first** so that every later cycle has a single source of truth for:

- **Where is the source code?** (repo path, service boundaries)
- **Where are logs?** (file path, Loki URL, OTLP endpoint)
- **Database credentials** (connection string or env reference, read-only)
- **Tracing** (OpenTelemetry/Jaeger endpoint, or “none”)

The AI Setup Agent produces a **project manifest** (`ctmint.yaml`). All downstream work (Knowledge Graph, code indexer, parser, runtime ingestion, data ingestion) **reads this manifest** and does not hardcode paths or credentials. So:

1. **Cycle 0**: Skeleton + manifest schema (contracts).
2. **Cycle 1**: Onboarding — AI collects paths and credentials, writes manifest.
3. **Cycle 2**: System Knowledge Graph (embedded).
4. **Cycle 3**: Code parser + indexer (using manifest).
5. **Cycle 4**: Vector index.
6. **Cycle 5**: Context funnel.
7. **Cycle 6**: Data ingestion (DB from manifest).
8. **Cycle 7**: Runtime ingestion (logs/traces from manifest).
9. **Cycle 8**: Plugins and capabilities.
10. **Cycle 9**: Diagnose MVP.
11. **Cycle 10**: Hardening.

Each cycle has its own detailed doc in this folder.

---

## Cycle Index

| Cycle | Doc | Focus |
|-------|-----|--------|
| 0 | [cycle-00-skeleton-and-contracts.md](cycle-00-skeleton-and-contracts.md) | Project skeleton, data model, manifest schema, storage interfaces |
| 1 | [cycle-01-onboarding.md](cycle-01-onboarding.md) | AI onboarding: source code, logs, DB, tracing → manifest |
| 2 | [cycle-02-knowledge-graph.md](cycle-02-knowledge-graph.md) | Embedded SKG (SQLite), traversal API |
| 3 | [cycle-03-code-parser-and-indexer.md](cycle-03-code-parser-and-indexer.md) | Parse repo, build symbol graph, write to SKG |
| 4 | [cycle-04-vector-index.md](cycle-04-vector-index.md) | Code chunks, embeddings, scoped semantic search |
| 5 | [cycle-05-context-funnel.md](cycle-05-context-funnel.md) | Multi-layer context reduction, token budget |
| 6 | [cycle-06-data-ingestion.md](cycle-06-data-ingestion.md) | DB schema extraction, link to SKG |
| 7 | [cycle-07-runtime-ingestion.md](cycle-07-runtime-ingestion.md) | Logs + traces ingestion, normalize, link to SKG |
| 8 | [cycle-08-plugins-and-capabilities.md](cycle-08-plugins-and-capabilities.md) | Capability router, tool registry, plugin manifest |
| 9 | [cycle-09-diagnose-mvp.md](cycle-09-diagnose-mvp.md) | End-to-end diagnose_service / diagnose_endpoint |
| 10 | [cycle-10-hardening.md](cycle-10-hardening.md) | Incremental index, security, observability, scale |

---

## How to Use

- Start with **Cycle 0** and complete its “Definition of done” before moving on.
- Each cycle doc has: **Goal**, **Prerequisites**, **Tasks** (step-by-step), **Deliverables**, **Definition of done**, **Acceptance criteria**.
- After **Cycle 1**, you have a valid `ctmint.yaml` for at least one project.
- After **Cycle 3**, you have a populated SKG and code navigation.
- After **Cycle 7**, you have code + runtime + data in one place for diagnosis.
- After **Cycle 9**, you have a full “Why is X failing?” demo.
