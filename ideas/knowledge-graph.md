# System Knowledge Graph (SKG) – Technical Design (ContextMint)

## 1. Purpose and Scope

The **System Knowledge Graph (SKG)** is the central store of **normalized system structure** for ContextMint. It holds:

- **Code structure**: Services, modules, classes, functions, endpoints, and their relationships (CALLS, IMPLEMENTS, IMPORTS, READS, WRITES).
- **Data structure**: Databases, tables, columns, indexes, and relationships (CONTAINS, HAS_COLUMN, HAS_INDEX, FOREIGN_KEY).
- **Runtime linkage**: References to logs, traces, and metrics (e.g. Service PRODUCES_LOG, HAS_TRACE) so the Context Funnel can scope retrieval.

The SKG is **graph-first**: reasoning and context selection are driven by **traversal** (e.g. endpoint → service → dependencies → tables), not by full-text or vector search alone. Vector search is used **after** the graph narrows the candidate set (see `vector-index.md`).

**Design principle (index-time over query-time complexity)**  
Complexity is shifted to **index time** (AST parsing, symbol extraction, dependency and call-graph building), not to query time. At query time we use **deterministic graph traversal** (and optionally Cypher or a fluent API), not embedding-based retrieval for structure. Benefits: deterministic answers, full structural context in one traversal, no chunking of the graph, and lower token usage for “what calls this?” / “what does this depend on?” style questions. This aligns with a **Graph RAG** approach: query → graph traversal → results, rather than query → embedding → vector search.

---

## 2. High-Level Architecture

```text
                    System Knowledge Graph
                                │
        ┌───────────────────────┼───────────────────────┐
        │                       │                       │
   Node Types              Edge Types               Query API
   (Service, Function,     (CALLS, IMPLEMENTS,      (traverse, filter,
    Table, LogEvent, ...)   READS, WRITES, ...)      scope by project)
        │                       │                       │
        └───────────────────────┼───────────────────────┘
                                │
                        Storage Layer
                (SQLite graph schema / Neo4j / ArangoDB)
```

**Writers**: Code Indexer, Runtime Ingestion, Data Ingestion.  
**Readers**: Orchestrator, Context Funnel, MCP tools (via SKG client).

---

## 3. Canonical Node Types

All nodes share a minimal base:
- `id`: globally unique (e.g. UUID or namespaced string like `service:auth-service`).
- `type`: node type (e.g. `Service`, `Function`).
- `project_id`: optional; for multi-tenant isolation.
- `created_at`, `updated_at`: optional timestamps.

### 3.1 Code Domain

| Node Type  | Key attributes                                      | Example id / name   |
|------------|------------------------------------------------------|---------------------|
| Service    | name, repo_path, language                            | auth-service        |
| Module     | name, path, service_id                               | auth.login          |
| Class      | name, module_id, service_id                           | LoginHandler        |
| Function   | name, signature, file_path, line_start, line_end, summary (optional), service_id | login_user          |
| Endpoint   | method, path, service_id                              | POST /login         |

**Optional (structure-first pass)**  
For repos where a full file-tree view is useful, **File** and **Folder** can be first-class nodes: **Folder** (path, parent_id), **File** (path, folder_id or module_id). Edges: Folder -[:CONTAINS]-> Folder | File; Module -[:CONTAINS]-> File when mapping “this module is implemented by these files.” Building the graph in a **structure pass** first (repo → Folder/File tree → CONTAINS) then layering AST-derived nodes (Function, Class, etc.) on top keeps the pipeline clear and supports “which files are in this module?” without scanning the filesystem at query time.

### 3.2 Data Domain

| Node Type     | Key attributes                          | Example           |
|---------------|----------------------------------------|-------------------|
| Database      | name, type (postgres/mysql/...), connection_id (opaque) | main_db           |
| DatabaseTable | name, schema, database_id               | public.users      |
| Column        | name, data_type, nullable, table_id    | users.email       |
| Index         | name, columns[], unique, table_id       | idx_users_email   |

### 3.3 Runtime Domain (lightweight in graph)

| Node Type | Key attributes                    | Role |
|-----------|-----------------------------------|------|
| LogEvent  | timestamp, service_id, level, message (truncated), external_id (pointer to log store) | Link to log store; filter by service |
| Trace     | trace_id, root_service_id, start_time, end_time, status, external_id | Link to trace store |
| Span      | span_id, trace_id, service_id, name, duration, external_id | Link to trace store |

Runtime nodes may be **minimal**: only enough to link and filter; full payload lives in the log/trace store. Alternatively, only **edges** from Service to external store (e.g. “log_store_id” + query params) are stored.

### 3.4 Optional: Network Domain

| Node Type    | Key attributes     | Role |
|--------------|---------------------|------|
| NetworkNode  | name, type (device/service) | For future network-layer reasoning |
| NetworkFlow  | source_id, target_id, protocol, metadata | |

---

## 4. Canonical Edge Types

Edges are directed and typed. Convention: `(Source)-[:TYPE]->(Target)`.

### 4.1 Code

| Edge Type   | Source    | Target    | Meaning |
|-------------|-----------|-----------|---------|
| CONTAINS    | Service   | Module    | Service owns module |
| CONTAINS    | Module    | Class, Function | Module contains symbol |
| CONTAINS    | Class     | Function  | Method of class |
| IMPLEMENTS  | Service   | Endpoint  | Service exposes endpoint |
| IMPLEMENTS  | Function  | Endpoint  | Handler for endpoint |
| CALLS       | Function  | Function  | Call relationship |
| CALLS       | Service   | Service   | Service-to-service call (derived or from code) |
| IMPORTS     | Module    | Module    | Import dependency |
| DEPENDS_ON  | Service   | Service, ExternalAPI, Cache | Dependency (code or config) |
| READS       | Function  | DatabaseTable | Function reads table |
| WRITES      | Function  | DatabaseTable | Function writes table |

**Call-graph construction (CALLS edges)**  
CALLS edges can be built in stages for better coverage: (1) **Exact match** using resolved imports and symbol IDs; (2) **Fuzzy match** (e.g. name similarity or Levenshtein) when the target is a string or dynamic call; (3) **Heuristics** for chaining, callbacks, or framework-specific patterns. This multi-stage approach improves CALLS coverage without requiring perfect static analysis. See Code Indexer and Code Parser design for how parser output feeds these stages.

### 4.2 Data

| Edge Type     | Source       | Target       |
|---------------|--------------|--------------|
| CONTAINS      | Database     | DatabaseTable|
| HAS_COLUMN   | DatabaseTable| Column       |
| HAS_INDEX     | DatabaseTable| Index        |
| HAS_PRIMARY_KEY | DatabaseTable | (internal) |
| FOREIGN_KEY   | DatabaseTable| DatabaseTable (with edge attrs: from_col, to_col) |

### 4.3 Runtime

| Edge Type    | Source  | Target   |
|--------------|---------|----------|
| PRODUCES_LOG| Service | LogEvent (or “log store” ref) |
| HAS_TRACE    | Service | Trace    |
| PART_OF      | Span    | Trace    |
| BELONGS_TO   | Span    | Service  |

---

## 5. Identity and Naming Conventions

- **Stable IDs**: Prefer deterministic IDs for code entities so that re-indexing does not create duplicates (e.g. `service:auth-service`, `func:auth-service::login_user`).
- **Composite keys**: For tables/columns, use `db:main_db::table:public.users`, `column:public.users.email`.
- **Project scope**: All nodes can carry `project_id`; queries default to one project when the context is project-scoped.

---

## 6. Query Patterns (Used by Context Funnel and MCP)

Typical read patterns:

1. **Service topology**
   - `MATCH (s:Service {name: $name})-[:CALLS|DEPENDS_ON]->(t) RETURN t`
   - Used to get dependencies of a service for “why is X failing?”.

2. **Endpoint → implementation**
   - `MATCH (e:Endpoint {method: $m, path: $p})<-[:IMPLEMENTS]-(f:Function) RETURN f`
   - Used to find handler code for an endpoint.

3. **Service → tables**
   - `MATCH (s:Service)-[:CONTAINS]->()-[:CONTAINS]->(f:Function)-[:READS|WRITES]->(t:DatabaseTable) WHERE s.name = $name RETURN DISTINCT t`
   - Used to show “tables this service uses”.

4. **Table → columns and indexes**
   - `MATCH (t:DatabaseTable {name: $name})-[:HAS_COLUMN]->(c:Column) RETURN c`
   - `MATCH (t:DatabaseTable {name: $name})-[:HAS_INDEX]->(i:Index) RETURN i`
   - Used for schema context when explaining slow queries or schema changes.

5. **Architecture map (Layer 3)**
   - Precomputed or on-demand: `MATCH (s:Service)-[:CALLS]->(t:Service) RETURN s, t` to get service-to-service graph for “architecture summary”.

6. **Callers and impact (Graph RAG style)**
   - **get_callers(symbol_id)**: Traverse incoming CALLS edges from the given Function (or symbol). “What calls this function?”
   - **get_dependencies(file_id | symbol_id)**: Traverse IMPORTS and optionally CALLS from the given node. “What does this file/symbol depend on?”
   - **blast_radius(function_id | service_id, max_hops?)**: BFS (or bounded traversal) over CALLS and IMPORTS from the given node to approximate “impact of changing this.” Useful for change-impact and risk scope.

These patterns are good candidates for **MCP tools**: expose them so the AI can ask “what calls X?” or “what’s the blast radius of changing Y?” via graph traversal instead of vector search.

---

## 7. Storage Options

### 7.1 Embedded (v1): SQLite as graph store

- **Schema**: Tables for nodes (e.g. `nodes`: id, type, project_id, attributes JSON) and edges (e.g. `edges`: source_id, target_id, type, attributes JSON).
- **Indexes**: (type, project_id), (source_id, type), (target_id, type) for fast traversal.
- **Queries**: Implement a small query layer (e.g. Cypher-like or fluent API) that translates to SQL (recursive CTEs or multiple joins for short paths).

### 7.2 External: Neo4j or ArangoDB

- **Use when**: Scale or query complexity justifies a dedicated graph DB.
- **Schema**: Same node/edge model; use native labels and relationship types.
- **Client**: Rust client (e.g. neo4j crate, arango driver) from the single binary; config to switch between embedded and external.

### 7.3 Hybrid

- **Embedded** for single-node/small deployments; **Neo4j/Arango** for large or multi-node deployments. Same API surface behind a storage abstraction.

---

## 8. Consistency and Updates

- **Writers**: Code Indexer, Runtime Ingestion, Data Ingestion write in batches or transactions. Prefer “upsert by id” so re-runs are idempotent.
- **Deletes**: When a file or service is removed, either soft-delete (e.g. `deleted_at`) or cascade delete related nodes/edges so the graph does not retain stale references.
- **Versioning**: Optional “graph version” or “index version” per project so readers can pin to a consistent snapshot during a request.

---

## 9. Build Pipeline and Performance (code graph)

A logical order for building the **code** part of the graph (aligned with graph-first code indexers) is:

1. **Structure pass** — Traverse repo; create Folder/File (or at least Module) nodes and CONTAINS edges (file tree skeleton).
2. **AST pass** — Parse files (e.g. Tree-sitter); extract Function, Class, Symbol nodes and attach to File/Module; optional **AST cache** (LRU, memory-bounded) to avoid re-parsing unchanged files (see `code-parser.md`).
3. **Import resolution** — Resolve module paths; add IMPORTS edges between modules/files.
4. **Call-graph pass** — Add CALLS edges (exact, fuzzy, heuristic as in §4.1).

Writers (e.g. Code Indexer) can build the graph **in memory** during the run, then **bulk load** into the persistent store (single transaction or batched writes) instead of writing row-by-row during parsing. This reduces DB round-trips and keeps index-time performance predictable for large repos.

---

## 10. Integration Summary

- **Code Indexer**: Writes Service, Module, Class, Function, Endpoint and code-domain edges.
- **Data Ingestion**: Writes Database, DatabaseTable, Column, Index and data-domain edges; Code Indexer adds READS/WRITES from Function to DatabaseTable.
- **Runtime Ingestion**: Writes minimal runtime nodes or only edges (Service → log/trace store refs) so funnel and plugins can scope queries.
- **Orchestrator / Funnel**: Read-only; traverse graph to narrow entities, then pull details from vector index and runtime stores.
- **MCP tools**: Expose `get_service_graph`, `get_db_schema`, `get_architecture_map`, and graph-RAG style tools such as `get_callers(symbol_id)`, `get_dependencies(file_id|symbol_id)`, `blast_radius(function_id|service_id)` by querying the SKG via the same query API.

---

## 11. Summary

The System Knowledge Graph is the **single source of truth** for system structure (code, data, and lightweight runtime linkage). It uses a **normalized node/edge model** and supports **traversal-based reasoning** so the Context Funnel and AI can answer “what services call this one?”, “what tables does this endpoint touch?”, and “what indexes exist on this table?” without scanning the whole codebase or database.
