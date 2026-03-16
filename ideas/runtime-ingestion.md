# Runtime Ingestion – Technical Design (ContextMint)

## 1. Purpose and Scope

The **Runtime Ingestion** pipeline collects **logs**, **traces**, and **metrics** from the target system and normalizes them into the System Knowledge Graph (SKG) and into searchable/aggregatable stores used by the Context Funnel. It enables the AI to reason over **runtime evidence** (e.g. “login is slow because Redis timeouts show up in traces”) in addition to code and schema.

Goals:
- Ingest from multiple sources (files, OpenTelemetry, Prometheus, Grafana, vendor APIs).
- Normalize to a **common event model** (LogEvent, Trace, Span, Metric).
- Link runtime data to **graph entities** (Service, Endpoint, Function) so the funnel can scope retrieval by service/endpoint.
- Support **real-time and batch** ingestion for both interactive debugging and historical analysis.

---

## 2. High-Level Architecture

```text
                    Runtime Ingestion (this component)
                                │
        ┌───────────────────────┼───────────────────────┐
        │                       │                       │
   Log Ingestor           Trace Ingestor          Metrics Ingestor
        │                       │                       │
   (files, syslog,          (OTel, Jaeger,           (Prometheus,
    Loki, vendor)            Zipkin, vendor)          vendor APIs)
        │                       │                       │
        └───────────────────────┼───────────────────────┘
                                │
                        Normalization Layer
                    (LogEvent, Trace, Span, Metric)
                                │
        ┌───────────────────────┼───────────────────────┐
        │                       │                       │
   SKG / Graph Writer      Log/Trace Store         Vector Index
   (link to Service,       (time-series or          (embeddings for
    Endpoint, etc.)         searchable store)        semantic search)
```

---

## 3. Normalized Event Model

All ingested runtime data is mapped to a small set of entity types.

### 3.1 LogEvent

- **Fields**: `timestamp`, `service_name`, `level` (info/warn/error/debug), `message`, `attributes` (key-value), `source` (file/collector), `raw` (optional original line).
- **Link to graph**: `service_name` → Service; optional `span_id`/`trace_id` → Trace/Span.
- **Use**: Search by service, time range, level, or semantic query (vector search on `message` + attributes).

### 3.2 Trace and Span

- **Trace**: `trace_id`, `root_service`, `start_time`, `end_time`, `status` (ok/error), optional `attributes`.
- **Span**: `span_id`, `trace_id`, `parent_span_id`, `service_name`, `name` (operation), `kind` (server/client/internal), `start_time`, `duration`, `status`, `attributes`, optional `resource`.
- **Link to graph**: `service_name` → Service; operation name or attributes (e.g. `http.route`) → Endpoint.
- **Use**: List traces for a service/endpoint; get latency distribution; find error traces; feed span path into context (e.g. gateway → auth-service → payment-service).

### 3.3 Metric

- **Fields**: `name`, `type` (counter/gauge/histogram), `value` (or histogram buckets), `timestamp`, `labels` (e.g. service, endpoint, status_code).
- **Link to graph**: `service` / `endpoint` labels → Service, Endpoint.
- **Use**: Latency percentiles, error rates, throughput; optional aggregation before feeding to LLM.

---

## 4. Data Sources and Adapters

### 4.1 Logs

| Source        | Adapter behavior                                                                 |
|---------------|-----------------------------------------------------------------------------------|
| File         | Tail or read; parse JSON/JSONL, syslog, or regex; extract timestamp, level, message, service (from path or field). |
| Syslog       | Parse RFC format; map to LogEvent; service from hostname or tag.                 |
| Loki         | Query API or stream; map log lines to LogEvent; preserve labels as attributes.     |
| OpenTelemetry| OTLP log ingestion; map ResourceLogs to LogEvent; link trace_id/span_id.          |
| Vendor (e.g. Datadog) | Pull via API; normalize to LogEvent; map service/tags.                    |

**Project config** (e.g. from manifest):
- `logs.provider`: file | syslog | loki | otel | datadog
- `logs.path` or `logs.endpoint` or `logs.url`
- `logs.format`: json | jsonl | text | syslog
- Optional: field mapping (e.g. `service` ← `service.name`)

### 4.2 Traces

| Source         | Adapter behavior                                                                 |
|----------------|-----------------------------------------------------------------------------------|
| OpenTelemetry  | OTLP trace ingestion; map to Trace/Span; extract service, operation, duration, status. |
| Jaeger         | Query API or Kafka; map to same model.                                            |
| Zipkin         | JSON or API; map spans to Span; reconstruct trace.                               |
| Vendor         | Pull via API; normalize to Trace/Span.                                            |

**Project config**:
- `tracing.provider`: otel | jaeger | zipkin | vendor
- `tracing.endpoint` (OTLP gRPC/HTTP, or Jaeger/Zipkin URL)
- Optional: sampling, time range for historical pull

### 4.3 Metrics

| Source     | Adapter behavior                                                       |
|-----------|------------------------------------------------------------------------|
| Prometheus| Scrape or remote read; map to Metric; use labels for service/endpoint. |
| OpenTelemetry | OTLP metrics; map to Metric.                                      |
| Vendor    | Pull via API; normalize.                                               |

**Project config**:
- `metrics.provider`: prometheus | otel | vendor
- `metrics.endpoint` or `metrics.scrape_config`
- Optional: which metrics to retain (e.g. latency, error_rate, throughput)

---

## 5. Normalization Layer

- **Input**: Raw log line, OTLP span, Prometheus sample, etc.
- **Output**: One or more normalized records (LogEvent, Span, Metric) with:
  - Canonical field names and types.
  - Resolved `service_name` (and optionally `endpoint_id` or `function_id`) for linking to SKG.
- **Service resolution**: Use resource attributes, labels, or tags to set `service_name`; optionally resolve to SKG `Service` node and attach `service_id` for strict linking.
- **Deduplication**: Optional dedup key (e.g. trace_id+span_id, or log line hash) to avoid double ingestion.

---

## 6. Storage and Indexing

- **Log/Trace store**: Can be:
  - **Embedded**: SQLite or similar with time-series–friendly layout (partition by time, index by service_id, trace_id).
  - **External**: Existing log/trace backend (e.g. Loki, Tempo); ingestion only writes to SKG and optionally to vector index; queries go to that backend via adapter.
- **Graph (SKG)**:
  - Write **links** only: e.g. `(Service)-[:PRODUCES_LOG]->(LogEvent)` or store LogEvent as node with `service_id`; similarly `(Service)-[:HAS_TRACE]->(Trace)`, `(Span)-[:PART_OF]->(Trace)`.
  - Optionally materialize aggregates: e.g. “last 24h error count per service” as node or cached value.
- **Vector index**:
  - Embed log messages (and optionally trace operation names/attributes) for semantic search; attach `service_id`, `timestamp`, `level` as metadata for filtering.
  - Context Funnel uses graph to restrict scope (e.g. auth-service only), then vector search within that scope.

---

## 7. Real-Time vs Batch

- **Real-time**: Stream from OTLP, tail files, or poll vendor APIs; normalize and write continuously; low latency for “last N minutes” queries.
- **Batch**: Historical backfill (e.g. last 7 days) from object storage, Loki, or vendor APIs; same normalization and storage path.
- **Retention**: Configurable (e.g. keep last 7 days in hot store; archive or drop older). Retention policy can be per project.

---

## 8. Integration with Context Funnel and Orchestrator

- **Funnel** requests “logs and traces for service X in time range T”. Runtime ingestion does not implement the funnel; it only ensures data is **available** and **linkable** to Service/Endpoint.
- **Orchestrator** calls plugins (e.g. logs plugin, telemetry plugin) that **query** the log/trace store (or external backends). Those plugins may be implemented inside the same binary and read from the same stores that ingestion writes to.
- **Capability routing**: “logs” and “traces” capabilities are backed by this data; plugin tools (e.g. `search_logs`, `query_traces`) use the normalized model and scoping by `service_id`/`endpoint_id`.

---

## 9. Configuration and Deployment

- **Per-project manifest** (see `Overview.md`, `mcp-core.md`): `logs`, `tracing`, `metrics` sections with provider and connection details.
- **Global**: Retention, buffer sizes, parallelism; optional auth for vendor APIs.
- **Deployment**: Runs inside the single binary (Rust); can be a background task (streaming) or triggered by CLI/cron for batch. For scale-out, a separate `ctmint-ingestor` process can run the same ingestion logic and write to shared storage.

---

## 10. Error Handling and Observability

- **Backpressure**: If write is slow, buffer in memory (with limit) or drop oldest; emit metric for dropped events.
- **Failures**: Log adapter errors (e.g. OTLP connection failed); retry with backoff; do not crash the process.
- **Metrics**: Ingested events per second by source and type; latency from receive to stored; errors and retries.
- **Structured logs**: Use for debugging pipeline (e.g. “normalization skipped: unknown format”).

---

## 11. Summary

Runtime Ingestion turns **logs**, **traces**, and **metrics** from heterogeneous sources into a **normalized event model** linked to the SKG (Service, Endpoint). It feeds log/trace stores and optionally the vector index so that the Context Funnel and MCP tools can retrieve runtime evidence scoped by service and time, enabling the AI to correlate code, schema, and runtime behavior in one place.
