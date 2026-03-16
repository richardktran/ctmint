# Cycle 3 — Code Parser and Code Indexer

## Goal

- **Parser**: Read source files from the paths specified in the **project manifest** (from onboarding) and produce a **normalized symbol/relation output** (functions, modules, imports, optional endpoints).
- **Indexer**: Consume parser output and write **nodes and edges** into the System Knowledge Graph (Cycle 2), and optionally produce **chunks** for the vector index (Cycle 4). After this cycle, we can answer “where is X defined?” and “what does service Y contain?” using the graph.

All paths and service boundaries come from **manifest only** (no hardcoded paths).

---

## Prerequisites

- Cycle 0: manifest schema, GraphStore trait, Node/Edge types.
- Cycle 1: valid `ctmint.yaml` with `project`, `services` (each with `repo_path`, `language`).
- Cycle 2: GraphStore implemented (SQLite); get_service_graph, get_architecture_map work.
- Parser and indexer do not require vector store yet; vector chunks can be a follow-up task in this cycle or in Cycle 4.

---

## Tasks (Step-by-Step)

### 1. Parser: normalized output schema

- [ ] Define the **parser output** struct (per file):
  - `path`, `language`, `parse_status` (ok | partial | error)
  - `symbols`: list of { id, kind (module|class|function|method|endpoint), name, qualified_name, signature?, span { line_start, line_end } }
  - `relations`: list of { kind (calls|imports|extends), source_id, target_id }
  - Optional: `data_access`: list of { symbol_id, kind (sql_query|orm_read|http_call), target (table/URL) }
- [ ] Document stable **symbol ID** format (e.g. `{repo_path}::{qualified_name}` or hash) so indexer can deduplicate and link.

### 2. Parser: one language first

- [ ] Pick **one language** (e.g. Python or Rust) and implement a **parser adapter**:
  - Use Tree-sitter (or language-specific parser) to get AST.
  - Walk AST to extract: functions/methods (name, span, signature if easy), modules/packages (from path or AST).
  - Extract **imports**: map module → module (or file → file).
  - **Calls**: best-effort (e.g. same-file call targets); cross-file can be “unknown” target id in v1.
  - **Endpoints**: if framework is detectable (e.g. FastAPI decorators, Flask route), extract method + path and attach to handler function.
- [ ] Output must conform to the normalized schema above.
- [ ] Handle **parse errors**: return parse_status = error and empty or partial symbols; do not crash the pipeline.
- [ ] Add a **file discovery** step: given `repo_path`, list files by extension (e.g. `.py`, `.rs`) and optionally respect `.gitignore` or an include/exclude list.

### 3. Parser: second language (optional in same cycle)

- [ ] Add another language (e.g. TypeScript or Go) with the same output schema. Reuse file discovery; switch parser by extension or manifest `language`.
- [ ] If only one language is in the manifest for a service, run only that parser for that service’s path.

### 4. Indexer: service and module nodes

- [ ] For each entry in `manifest.services`:
  - Create or update a **Service** node: id = e.g. `service:{name}`, attrs = { name, repo_path, language, project_id }.
  - Run parser on `repo_path` (or monorepo subpath).
  - Create **Module** nodes for top-level packages/namespaces (e.g. one per directory or per AST module). Edge Service -[:CONTAINS]-> Module.
- [ ] Use **project_id** from manifest for all nodes/edges.

### 5. Indexer: function and endpoint nodes

- [ ] For each parsed **function/method** (and optional endpoint):
  - Create **Function** node: id = stable id from parser, attrs = { name, file_path, line_start, line_end, signature?, service_id }.
  - Edge Module -[:CONTAINS]-> Function (or Class -[:CONTAINS]-> Function if you have classes).
  - If endpoint detected: create **Endpoint** node (method, path), edge Service -[:IMPLEMENTS]-> Endpoint and Function -[:IMPLEMENTS]-> Endpoint.
- [ ] **Relations** from parser: create edges IMPORTS (module→module), CALLS (function→function). If target is in another file, resolve by qualified name or leave as external ref (e.g. target_id = “external:…”).
- [ ] **Batch write**: collect all nodes/edges for the repo and call GraphStore.batch_commit so the graph updates in one go.

### 6. Indexer: architecture map (service → service)

- [ ] From CALLS or HTTP client usage in parser output, infer **service → service** edges:
  - If a function in service A calls a URL like `http://service-b/...` or a client named “service_b”, add edge Service A -[:CALLS]-> Service B (create Service B node if not exists, or link by name).
  - If no HTTP detection, skip or use a simple heuristic (e.g. shared imports from a “common” package).
- [ ] Write these edges so **get_architecture_map** returns a meaningful service topology for multi-service repos.

### 7. CLI: index command

- [ ] **ctmint index [--project ./ctmint.yaml]**:
  - Load manifest; for each service, run parser on repo_path → run indexer → batch_commit to GraphStore.
  - Print summary: files parsed, nodes/edges written, any parse errors per file.
- [ ] Optional: `--incremental` (later): only re-parse changed files; for v1 full re-index is fine.

### 8. MCP tools

- [ ] **get_code_snippet(symbol_id)**: Look up Function node by id; read file_path and line range from attrs; read file from disk (or from a cached blob) and return the snippet. If file missing, return error or “file not found.”
- [ ] **get_function_summary(symbol_id)**: Return name, signature, file path, and optional docstring/summary (e.g. first comment block or “summary” field if you add it in a later cycle). No LLM required for v1.
- [ ] **get_service_graph(service_name)** already exists (Cycle 2); ensure it now returns real data after index.

### 9. Optional: chunks for vector index

- [ ] For each Function (or each file), produce a **chunk** (content = function body or file slice, metadata = service_id, symbol_id, file_path, line range). Store in a list or send to VectorStore. If VectorStore is not yet implemented, write chunks to a simple JSON/NDJSON file so Cycle 4 can consume them. Alternatively defer chunk production to Cycle 4 and only pass symbol list + file paths.

### 10. Tests

- [ ] Parser test: run parser on a fixture file (e.g. a small Python file with one function and one import); assert symbols and relations match expected.
- [ ] Indexer test: run indexer on a fixture repo (with a minimal manifest); assert Service, Module, Function nodes and CONTAINS/IMPORTS edges in GraphStore.
- [ ] E2E: `ctmint index --project tests/fixtures/sample-manifest.yaml` then `ctmint graph query --service <name>` returns the indexed nodes.

---

## Deliverables

- Parser for at least one language (two if time) with normalized output schema.
- Indexer that reads manifest, runs parser, writes to GraphStore (Service, Module, Function, Endpoint, CONTAINS, IMPLEMENTS, CALLS, IMPORTS).
- CLI: `ctmint index --project ./ctmint.yaml`.
- MCP tools: `get_code_snippet(symbol_id)`, `get_function_summary(symbol_id)`.
- Optional: chunk output for vector index (or clearly deferred to Cycle 4).
- Tests: parser unit test, indexer integration test, one E2E index → graph query.

---

## Definition of Done

- [ ] After `ctmint init` (Cycle 1) and `ctmint index` (this cycle), `get_service_graph(service_name)` returns real functions and modules.
- [ ] `get_code_snippet(symbol_id)` returns the correct source snippet for an indexed function.
- [ ] No hardcoded repo paths; all paths come from manifest.
- [ ] Parse errors in individual files do not abort the whole index; they are reported and the rest of the repo is indexed.

---

## Acceptance Criteria

- A new service added to the manifest (new repo_path) can be indexed by re-running `ctmint index` and appears in the graph.
- Architecture map (service → service) is non-empty when the codebase contains detectable cross-service calls.

---

## Estimated Duration

5–10 days (parser 2–3, indexer 2–3, integration and tests 1–2, optional second language + chunks 2).
