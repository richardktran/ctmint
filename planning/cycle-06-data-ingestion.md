# Cycle 6 — Data Ingestion (DB Schema, Link to SKG)

## Goal

**Extract database schema** (tables, columns, indexes, primary keys, foreign keys) from the database configured in the **project manifest** (from onboarding), and write **nodes and edges** into the System Knowledge Graph. Optionally **link code to tables** (which functions read/write which tables) using parser output or simple heuristics. After this cycle, we can answer “what columns does table X have?” and “which service touches table Y?” from the graph.

---

## Prerequisites

- Cycle 0: manifest schema with optional `database` (type, connection, schema).
- Cycle 1: onboarding may have collected database connection (or user skips; then this cycle is no-op for that project).
- Cycle 2: GraphStore; we will add node types Database, DatabaseTable, Column, Index and edges CONTAINS, HAS_COLUMN, HAS_INDEX, FOREIGN_KEY.
- Cycle 3: Code indexer may have emitted READS/WRITES edges (Function → Table); if not, we can add best-effort linking in this cycle.

---

## Tasks (Step-by-Step)

### 1. Schema extractor: one database type

- [ ] Implement **schema extraction** for **PostgreSQL** first (or MySQL if that’s your priority):
  - Connect using `manifest.database.connection` (resolve env vars like `${DATABASE_URL}`).
  - Use **read-only** user; only query catalog (information_schema / pg_catalog).
  - Extract: **tables** (schema name, table name), **columns** (name, data_type, is_nullable, default), **indexes** (name, columns, unique), **primary key** (column list), **foreign keys** (from table.column to ref_table.ref_column).
- [ ] Normalize into internal structs: Database, DatabaseTable, Column, Index, PrimaryKey, ForeignKey (see `data-ingestion.md`).
- [ ] Handle **errors**: connection failure, permission denied → log and return empty or partial result; do not crash.

### 2. Map to SKG node/edge types

- [ ] Create **Database** node: id = e.g. `db:{project_id}:default`, attrs = { name, type: postgres }.
- [ ] For each table: **DatabaseTable** node id = e.g. `table:{schema}.{name}`, attrs = { name, schema }, edge Database -[:CONTAINS]-> DatabaseTable.
- [ ] For each column: **Column** node id = e.g. `column:{schema}.{table}.{name}`, attrs = { name, data_type, nullable }, edge DatabaseTable -[:HAS_COLUMN]-> Column.
- [ ] For each index: **Index** node (or store in table attrs); edge DatabaseTable -[:HAS_INDEX]-> Index; attrs = { name, columns[], unique }.
- [ ] Foreign keys: edge DatabaseTable -[:FOREIGN_KEY]-> DatabaseTable with edge attrs { from_column, to_column }.
- [ ] All nodes/edges: set **project_id** from manifest.

### 3. Deduplication and upsert

- [ ] Use **upsert** (by node id) so re-running the extractor does not duplicate nodes. Update attrs if schema changed.
- [ ] Use **batch_commit** for consistency.

### 4. Code–table linking (best-effort)

- [ ] If Cycle 3 already writes READS/WRITES from parser (e.g. SQL string or ORM), ensure table names **match** SKG table names (e.g. `public.users`). Resolve by name.
- [ ] If not: add a simple **heuristic** in this cycle: e.g. grep function bodies for table names that exist in the extracted schema; create Function -[:READS]-> DatabaseTable or WRITES for each match. Label as “heuristic” in attrs if needed.
- [ ] Optional: defer full linking to a later cycle and only do schema in this cycle.

### 5. CLI and config

- [ ] **ctmint db ingest [--project ./ctmint.yaml]**:
  - Load manifest; if `database` is absent or connection empty, print “No database configured; skip.” and exit 0.
  - Otherwise connect, extract schema, write to GraphStore.
  - Print: tables, columns, indexes count.
- [ ] **Credentials**: never log connection string; use env var for secrets. Document in README.

### 6. MCP tools

- [ ] **get_db_schema(service_or_db)**:
  - If service: find tables linked to that service (via READS/WRITES from functions in that service). Return list of tables with columns and indexes.
  - If db name or “default”: return all tables for the project’s database.
  - Format: readable text or JSON (table name, columns, indexes, PK, FK).
- [ ] **get_db_usage(table_name)** (optional):
  - Find Function nodes that have READS or WRITES to the given table; return list of (function_id, edge_type).

### 7. Tests

- [ ] Unit test: with a **mock** catalog (e.g. in-memory SQLite with information_schema-like tables, or a fixture JSON), run extractor and assert Database, DatabaseTable, Column nodes and edges.
- [ ] Integration test: if possible, run against a real Postgres with a tiny schema (one table, two columns, one index); assert nodes in GraphStore. Use testcontainers or a dedicated test DB.
- [ ] Test “no database” path: manifest without database section → ingest exits 0 and does not write.

---

## Deliverables

- Schema extractor for Postgres (or one DB type) with normalized output.
- SKG writer for Database, DatabaseTable, Column, Index and CONTAINS, HAS_COLUMN, HAS_INDEX, FOREIGN_KEY.
- Optional: READS/WRITES from code to tables (heuristic or from parser).
- CLI: `ctmint db ingest`.
- MCP tools: get_db_schema, optionally get_db_usage.
- Tests: unit with mock catalog; optional integration with real DB.

---

## Definition of Done

- [ ] After onboarding with a DB connection and `ctmint db ingest`, the graph contains table and column nodes for that database.
- [ ] get_db_schema returns correct columns and indexes for a given table.
- [ ] No credentials in logs or docs; connection uses env var when sensitive.

---

## Acceptance Criteria

- Context funnel (Cycle 5/9) can include “tables used by this service” and “schema of table X” in the context when diagnosing DB-related issues.
- A user can ask “Does users have an index on email?” and get a correct answer from the graph.

---

## Estimated Duration

4–8 days.
