# Code Parser – Technical Design (ContextMint)

## 1. Purpose and Scope

The **Code Parser** is the component that reads raw source code and produces **language-agnostic**, structured output consumed by the **Code Indexer**. It is responsible for:

- Parsing source files into **ASTs** (Abstract Syntax Trees).
- Extracting **symbols**: modules, classes, functions, methods, and—where detectable—endpoints (HTTP routes, RPC methods, message handlers).
- Extracting **relationships**: calls, imports, inheritance, and data access (e.g. SQL snippets, ORM usage, HTTP client calls).
- Supporting **multiple languages** via a common output schema so the indexer and SKG stay language-independent.

The parser does **not** build the graph, compute architecture maps, or write to SKG or vector store; it only produces a normalized stream of symbols and relations per file (or per repo).

---

## 2. Design Principles

- **Adapter / project separation**: Parser logic is split into **language-specific adapters** (e.g. Python, Rust, Go, Java, TypeScript). Each adapter outputs the **same normalized schema** (see below).
- **Incremental-friendly**: Support parsing a single file or a set of changed files; output must be mergeable and identifiable (e.g. by file path and symbol ID).
- **Robustness**: On parse failure (syntax error, unsupported construct), emit partial results where possible and tag the file as failed; do not fail the entire repo.

---

## 3. Normalized Parser Output Schema

All language adapters produce a common structure. Below is a concise, implementation-oriented schema.

**Per-file output** (conceptual):

```text
File:
  path: string
  language: string
  parse_status: ok | partial | error
  symbols: List[Symbol]
  relations: List[Relation]
  raw_refs: List[DataAccessRef]   // optional: SQL, table names, etc.
```

**Symbol** (simplified):

```text
Symbol:
  id: string (e.g. file_path::qualified_name or stable hash)
  kind: module | class | function | method | endpoint
  name: string
  qualified_name: string
  signature: string (optional)
  span: { line_start, line_end, col_start, col_end }
  metadata: map (e.g. method: "POST", path: "/login" for endpoint)
```

**Relation**:

```text
Relation:
  kind: calls | imports | extends | implements
  source_id: string (symbol id)
  target_id: string (symbol id or external ref)
  metadata: map (optional)
```

**DataAccessRef** (for DB/API usage):

```text
DataAccessRef:
  kind: sql_query | orm_read | orm_write | http_call | message_publish | message_consume
  symbol_id: string
  target: string (table name, URL pattern, topic name)
  snippet: string (optional, for SQL or query text)
```

The indexer uses these to create SKG nodes (Service, Module, Class, Function, Endpoint) and edges (CALLS, IMPORTS, READS, WRITES, IMPLEMENTS).

---

## 4. Language Adapters and Tooling

Recommended approach: use **Tree-sitter** for as many languages as possible to get a single parsing stack with consistent APIs; supplement with language-specific tools where Tree-sitter is insufficient (e.g. for endpoint or framework-specific extraction).

| Language   | Parser / tool          | Notes                                                                 |
|-----------|------------------------|-----------------------------------------------------------------------|
| Python    | Tree-sitter-python     | AST; FastAPI/Flask routes via pattern match or AST walk               |
| Rust      | Tree-sitter-rust       | AST; actix/axum routes from attributes or macro expansion (best-effort)|
| Go        | Tree-sitter-go         | AST; HTTP routes from gorilla/mux or stdlib patterns                   |
| Java      | Tree-sitter-java / javaparser | AST; Spring MVC annotations for endpoints                    |
| TypeScript/JavaScript | Tree-sitter (ts/js) | AST; Express/Fastify routes from decorators or patterns       |
| C#        | Tree-sitter (if available) or Roslyn | Controllers and routes from attributes                   |

**Endpoint detection** (examples):
- **Python (FastAPI)**: Look for `@app.post("/login")`-style decorators; extract method and path.
- **Rust (actix)**: Look for `#[post("/login")]` and associated handler function.
- **Go**: Look for `router.HandleFunc("/login", handler)` or `mux.Handle("/login", ...)`.
- **Java (Spring)**: Look for `@PostMapping("/login")` on class/method.

**Data access detection**:
- **SQL**: Regex or AST for string literals passed to `execute`, `query`; or use of SQL builders (signatures only).
- **ORM**: Detect calls like `Model.query.filter_by(...)` (Python), `repository.find_by_...` (Java), and map to table names from schema or naming conventions.
- **HTTP client**: Detect `http.get("https://auth-service/...")`, `client.Post("/login")` to infer service calls.

These are best-effort; the goal is to populate the graph well enough for the Context Funnel to narrow the search space. Gaps can be filled later by DB schema ingestion and runtime traces.

---

## 5. Parser Pipeline Stages

1. **Discovery**
   - Given a repo path (and optional include/exclude globs), list files to parse.
   - Filter by language (e.g. `.py`, `.rs`, `.go`); optionally respect `.gitignore`.

2. **Language detection**
   - Per file: extension and/or content heuristics to choose adapter.

3. **Parse**
   - Invoke the appropriate adapter; get AST (or equivalent).
   - Extract symbols and relations (and optionally raw_refs) according to the normalized schema.
   - On syntax error: set `parse_status: error` or `partial`; still emit any symbols that could be recovered.

4. **Emit**
   - Output per-file or batched output to the Code Indexer (in-process call or queue).
   - Include repo_root and project_id so the indexer can resolve paths and service boundaries.

---

## 6. Integration with Code Indexer

- **Contract**: Parser produces a stream or batch of **per-file** normalized output. Indexer consumes it and builds the symbol graph, architecture map, and vector chunks.
- **Incremental**: Indexer may request “parse only these paths”; parser returns output only for those files. Indexer then diffs against previous run to update the graph.
- **IDs**: Parser should emit **stable symbol IDs** (e.g. `repo_path::module::Class::method`) so that indexer can deduplicate and match across runs.

---

## 7. Performance Considerations

- **Scale**: Repositories with millions of LOC should be parsed in parallel (e.g. by file or by directory). Tree-sitter is fast and incremental; use it to minimize re-parsing.
- **Caching**: Cache AST or symbol output per file (keyed by path + content hash) to avoid re-parsing unchanged files.
- **Resource limits**: Cap memory per parse (e.g. very large generated files); timeout per file to avoid one bad file blocking the pipeline.

---

## 8. Configuration

- **Repo path**, **include/exclude** patterns.
- **Languages** to enable (and which adapters to load).
- **Endpoint/data-access** detection: enable/disable by language or framework.
- **Output**: in-process API vs. writing to a directory/queue for async consumption by the indexer.

---

## 9. Error Handling and Observability

- **Parse errors**: Log file path and error; set `parse_status`; continue with other files.
- **Metrics**: Files parsed, symbols extracted, relations extracted, parse errors per language.
- **Structured logs**: Emit parse duration and file path for slow or failing files.

---

## 10. Summary

The Code Parser provides **multi-language**, **normalized** symbol and relation extraction from source code. It feeds the Code Indexer, which in turn populates the System Knowledge Graph and Vector Index. Keeping the parser output schema **stable and language-agnostic** ensures that the rest of ContextMint remains independent of the mix of languages in a project.
