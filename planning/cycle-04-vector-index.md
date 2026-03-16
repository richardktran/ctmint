# Cycle 4 — Vector Index (Code Chunks, Embeddings, Scoped Search)

## Goal

Add **semantic search** over code by storing **embeddings** of code chunks with **metadata** (project_id, service_id, symbol_id, file_path). Search must be **scoped**: e.g. “search only within auth-service” so that the context funnel (Cycle 5) can narrow by graph first, then retrieve relevant snippets by vector. No logs/traces in this cycle; code only.

---

## Prerequisites

- Cycle 0: VectorStore trait defined (upsert, search with filters).
- Cycle 2: GraphStore and get_service_graph available (to resolve scope).
- Cycle 3: Code indexer produces symbols and optionally chunks; we need at least function-level or file-level content to embed.
- Embedding model: either local (e.g. sentence-transformers via Python, or a Rust binding) or remote API (OpenAI, Cohere, or local Ollama embedding). Decide and document.

---

## Tasks (Step-by-Step)

### 1. Chunk producer (from indexer or standalone)

- [ ] **Input**: Either consume indexer output (list of symbols + file paths) or re-scan indexed repo using manifest.
- [ ] **Chunking strategy**:
  - Prefer **function-level** chunks: one chunk per function (content = function body or signature + first N lines). Metadata: service_id, symbol_id, file_path, line_start, line_end.
  - Fallback: **sliding window** (e.g. 50 lines with 10-line overlap) for files that are not fully parsed.
- [ ] **Output**: Stream or list of { id, content, metadata }. Id = stable (e.g. symbol_id or `{file_path}:{line_start}:{line_end}`).
- [ ] If the indexer (Cycle 3) already emits chunks, reuse that; otherwise add a **chunk** step that reads from GraphStore (list Function nodes) and reads file content from disk for each.

### 2. Embedding pipeline

- [ ] **Embedding client**: Call embedding API or local model with `content` string; get vector (list of floats). Dimension must be fixed (e.g. 384 or 768); document it.
- [ ] **Batch**: To avoid rate limits, batch requests (e.g. 10–50 chunks per request if API supports).
- [ ] **Idempotency**: Same chunk id should produce same vector; skip re-embedding if vector store already has id (optional optimization).
- [ ] **Errors**: Retry with backoff; if a chunk fails, log and skip; do not fail the whole run.

### 3. VectorStore implementation

- [ ] Implement the **VectorStore** trait (from Cycle 0) with one backend:
  - **Option A**: Embedded Qdrant (single directory). Use Qdrant’s Rust client or embed mode.
  - **Option B**: FAISS + metadata in SQLite or JSON (vector in FAISS, id + metadata in side store for filtering).
  - **Option C**: sqlite-vec or similar if available in your stack.
- [ ] **upsert(id, vector, metadata)**: Store vector and metadata. Metadata must include at least project_id, service_id, type = "code", and optionally symbol_id, file_path.
- [ ] **search(vector, filters, top_k)**:
  - Filters: by project_id, service_id (optional), type.
  - Return top_k nearest vectors with their ids and metadata (and optionally content if stored).
- [ ] **Collection**: Use one collection (or namespace) for “code”; later add “log”, “trace” in Cycle 7. Name the collection/table so it’s clear.

### 4. Build pipeline

- [ ] **ctmint vector build [--project ./ctmint.yaml]**:
  - Load manifest; get list of services and repo paths (or get Function nodes from GraphStore for those services).
  - Produce chunks (from indexer output or by reading graph + files).
  - Embed each chunk; upsert into VectorStore.
  - Print: chunks processed, failed count, duration.
- [ ] Optional: run this right after `ctmint index` in a single “ctmint index --with-vector” flow; for clarity, separate commands are fine for v1.

### 5. Search API and CLI

- [ ] **search_code(query, service_id?, project_id?, top_k)**:
  - Embed the query string (same embedding model as index).
  - Build filters: project_id from manifest or param; service_id if provided.
  - Call VectorStore.search; return list of (chunk_id, score, content_snippet, metadata).
- [ ] **ctmint vector search --query "session timeout" [--service auth-service] [--project ./ctmint.yaml]**:
  - Call search_code and print results (readable or JSON).

### 6. MCP tool

- [ ] **search_code(query, service?, top_k?)**: Input: query (string), optional service name, optional top_k (default 10). Resolve service name to service_id via GraphStore; call search_code with filters; return results as JSON (list of snippets with file path and line range).
- [ ] Wire in MCP server; ensure tool description explains that search is scoped by project (and optionally by service).

### 7. Config

- [ ] Add to global config (or manifest): **embedding** section:
  - `endpoint` (URL for API or “local”)
  - `model` or `dimension`
  - Optional: `api_key` env var reference.
- [ ] Document in README how to set up embedding (e.g. Ollama run model, or OpenAI API key).

### 8. Tests

- [ ] Unit test: insert a few vectors with metadata into VectorStore; search with same vector and filter by service_id; assert correct ids returned.
- [ ] Integration: run vector build on a small fixture repo; run search with a query; assert at least one result and metadata contains service_id and file path.

---

## Deliverables

- VectorStore implementation (embedded Qdrant or FAISS + metadata).
- Chunk producer and embedding pipeline; `ctmint vector build`.
- search_code API and CLI `ctmint vector search`.
- MCP tool `search_code(query, service?, top_k?)`.
- Config for embedding endpoint/model.
- Tests: vector upsert/search with filters; one E2E build → search.

---

## Definition of Done

- [ ] After `ctmint index` and `ctmint vector build`, semantic search for a natural language query returns relevant code chunks from the right service.
- [ ] Search can be scoped by service (and project) so that we don’t search the whole codebase when the graph has already narrowed to one service.
- [ ] Token/embedding cost is bounded (e.g. only indexed chunks are embedded; no duplicate chunks by id).

---

## Acceptance Criteria

- Context funnel (Cycle 5) can call search_code(query, service_id) and get a list of snippets to include in the prompt.
- A user can ask “where do we handle login timeout?” and get back function-level results from the auth service (when scope is set).

---

## Estimated Duration

4–7 days.
