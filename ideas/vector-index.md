# Vector Index – Technical Design (ContextMint)

## 1. Purpose and Scope

The **Vector Index** stores **embeddings** of code chunks, log snippets, trace summaries, and optional documentation so that **semantic search** can retrieve relevant content. It is used **after** the System Knowledge Graph (SKG) has narrowed the search space: e.g. “search only within auth-service’s code and logs,” not over the entire system. This **hybrid (graph + vector)** approach keeps context size small and relevance high.

Goals:
- Store embeddings and metadata (service_id, symbol_id, file_path, timestamp, level, etc.).
- Support **scoped search**: filter by project_id, service_id, type (code/log/trace/doc), time range.
- Support **similarity search**: top-k by embedding distance (e.g. cosine or inner product).
- Optional: **reranking** or filtering by keyword in addition to vector.

---

## 2. High-Level Architecture

```text
                    Vector Index (this component)
                                │
        ┌───────────────────────┼───────────────────────┐
        │                       │                       │
   Code Chunks              Log Chunks              Trace / Doc
   (from Indexer)           (from Runtime           Chunks
                             Ingestion)
        │                       │                       │
        └───────────────────────┼───────────────────────┘
                                │
                        Embedding Model
                        (one or more: code, log, general)
                                │
                        Vector Store
                (embedded Qdrant / FAISS / external)
                                │
                        Query API
                (search with filters + top_k)
```

**Writers**: Code Indexer (code chunks), Runtime Ingestion (log/trace chunks). Optional: doc ingestor.  
**Readers**: Context Funnel, MCP tools (e.g. semantic code search).

---

## 3. Chunk Types and Metadata

### 3.1 Code chunks

- **Content**: Snippet of source (e.g. function body, or fixed-size window with overlap).
- **Metadata**: `project_id`, `service_id`, `symbol_id`, `file_path`, `line_start`, `line_end`, `layer` (0=raw, 1=symbol ref, 2=summary).
- **Use**: Semantic search for “where do we validate email?” scoped to a service or repo.

### 3.2 Log chunks

- **Content**: Log message (and optionally key attributes concatenated).
- **Metadata**: `project_id`, `service_id`, `timestamp`, `level`, `trace_id`, `span_id`.
- **Use**: “Find logs similar to ‘timeout’ or ‘connection refused’” within a service and time range.

### 3.3 Trace chunks

- **Content**: Summary text (e.g. “gateway → auth-service → payment-service” or span names + attributes).
- **Metadata**: `project_id`, `trace_id`, `service_id`, `start_time`, `duration`, `status`.
- **Use**: “Find traces similar to this error pattern” within a service.

### 3.4 Doc chunks (optional)

- **Content**: Paragraph or section from internal docs, README, runbooks.
- **Metadata**: `project_id`, `source`, `path`.
- **Use**: Retrieve relevant docs for “how do we deploy X?”.

---

## 4. Embedding Strategy

- **Models**: Prefer one model per domain (code, log, general text) if quality justifies it; otherwise a single general-purpose embedding model (e.g. sentence-transformers, or vendor API) for all.
- **Code**: Use a code-aware model (e.g. CodeBERT, StarCoder embeddings, or vendor code embedding API) for code chunks; fallback to general text model if needed.
- **Logs/traces**: General text or log-specific fine-tuned model; keep embedding dimension consistent for a single store or use separate collections per type.
- **Dimension**: Fix dimension (e.g. 384 or 768); all vectors in one collection must share dimension. Normalize if using cosine similarity.

---

## 5. Storage Options

### 5.1 Embedded (v1)

- **Qdrant**: Embedded mode (single file or directory); Rust client; supports filters and payload.
- **FAISS**: File-backed index; good for read-heavy, less flexible on metadata filtering; can wrap in a small service that applies filters in application layer.
- **sqlite-vec** (or similar): Store vectors in SQLite with a vector extension; simple for small scale.

Choose one for v1 (e.g. Qdrant embedded) and abstract behind a **VectorStore** trait so switching is possible.

### 5.2 External

- **Qdrant / Weaviate / Pinecone**: When scale or multi-node deployment requires a dedicated service. Same query API (embed query → search with filters → top_k).

---

## 6. Indexing Pipeline

1. **Chunk producer** (Indexer or Runtime Ingestion) emits (content, metadata).
2. **Embedding**: Call embedding model (local or API) to get vector.
3. **Store**: Upsert (id, vector, metadata) into vector store. Id can be content hash or deterministic (e.g. `file_path:line_start:line_end`) for deduplication.
4. **Scoping**: Metadata must include at least `project_id` and preferably `service_id` and `type` (code/log/trace/doc) so queries can filter.

**Incremental**: When code or logs change, update or delete old chunks and add new ones (e.g. by symbol_id or log id) to avoid stale results.

---

## 7. Query API

**Input**:
- `query_text`: natural language or code snippet (will be embedded).
- `filters`: e.g. `project_id`, `service_id`, `type`, `time_range` (for logs/traces).
- `top_k`: number of results (e.g. 10–50).
- Optional: `min_score` (distance threshold).

**Output**:
- List of (chunk_id, score, content_snippet, metadata) for the funnel or MCP to use.

**Flow**:
1. Embed `query_text`.
2. Query vector store with filters and top_k.
3. Optionally rerank or filter by keyword.
4. Return results.

---

## 8. Hybrid Retrieval (Graph + Vector)

The Context Funnel uses the SKG first, then the vector index:

1. **Graph**: User asks “Why is login slow?” → Funnel finds endpoint POST /login → service auth-service → dependencies (redis, postgres, user-service).
2. **Scope**: Funnel sets filter: `service_id IN (auth-service, redis, postgres)` and `type IN (code, log, trace)`.
3. **Vector**: Query “login slow timeout error” with that scope → get top_k code chunks and log chunks.
4. **Context**: Funnel builds a compact context from graph summary + vector results and sends to LLM.

So the vector index **never** searches the whole system in one go when answering a focused question; it always runs in a **scoped** way derived from the graph.

---

## 9. Configuration and Deployment

- **Embedding model**: Model name or API endpoint; API key if cloud.
- **Vector store**: Backend (embedded vs external), path or URL, collection names (e.g. code, logs, traces).
- **Chunking**: Size and overlap for code; whether to embed full log line or truncated.
- **Deployment**: Same binary (Rust) embeds the store or connects to external; Code Indexer and Runtime Ingestion run inside the same process and write to the same store.

---

## 10. Error Handling and Observability

- **Embedding failures**: Retry with backoff; skip chunk and log if persistent.
- **Storage full**: Configurable max size; eviction by oldest or by project.
- **Metrics**: Indexed chunk count by type and project; query latency and result count; embedding API latency.
- **Structured logs**: For debugging retrieval (e.g. “vector search returned 0 for service X”).

---

## 11. Summary

The Vector Index provides **semantic search** over code, logs, traces, and optional docs, with **metadata filtering** (project, service, type, time) so that retrieval is always **scoped** by the graph. It is a key part of the **graph-first, vector-augmented** design: the SKG narrows the space, and the vector index returns the most relevant snippets within that space for the Context Funnel and the LLM.
