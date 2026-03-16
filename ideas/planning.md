# ContextMint Implementation Plan

The full implementation plan lives in the **`planning/`** folder. It is organized so that **onboarding comes first** (AI collects source code paths, log locations, database credentials, tracing), then we build the Knowledge Graph, index and parse the codebase, and add vector search, context funnel, runtime ingestion, and diagnosis.

## Entry point

- **[planning/README.md](planning/README.md)** — Overview, cycle order, and index of all cycle docs.

## Cycle order (onboarding first)

| Order | Cycle | Doc |
|-------|--------|-----|
| 0 | Skeleton and contracts | [cycle-00-skeleton-and-contracts.md](planning/cycle-00-skeleton-and-contracts.md) |
| 1 | **Onboarding** (AI: source code, logs, DB, tracing → manifest) | [cycle-01-onboarding.md](planning/cycle-01-onboarding.md) |
| 2 | System Knowledge Graph (embedded) | [cycle-02-knowledge-graph.md](planning/cycle-02-knowledge-graph.md) |
| 3 | Code parser and indexer | [cycle-03-code-parser-and-indexer.md](planning/cycle-03-code-parser-and-indexer.md) |
| 4 | Vector index | [cycle-04-vector-index.md](planning/cycle-04-vector-index.md) |
| 5 | Context funnel | [cycle-05-context-funnel.md](planning/cycle-05-context-funnel.md) |
| 6 | Data ingestion (DB schema) | [cycle-06-data-ingestion.md](planning/cycle-06-data-ingestion.md) |
| 7 | Runtime ingestion (logs and traces) | [cycle-07-runtime-ingestion.md](planning/cycle-07-runtime-ingestion.md) |
| 8 | Plugins and capabilities | [cycle-08-plugins-and-capabilities.md](planning/cycle-08-plugins-and-capabilities.md) |
| 9 | Diagnose MVP | [cycle-09-diagnose-mvp.md](planning/cycle-09-diagnose-mvp.md) |
| 10 | Hardening | [cycle-10-hardening.md](planning/cycle-10-hardening.md) |

Each cycle doc has: **Goal**, **Prerequisites**, **Tasks** (step-by-step with checkboxes), **Deliverables**, **Definition of done**, **Acceptance criteria**, and **Estimated duration**.

Start with **Cycle 0**, then **Cycle 1 (onboarding)** so that every later cycle reads paths and credentials from the generated `ctmint.yaml` manifest.
