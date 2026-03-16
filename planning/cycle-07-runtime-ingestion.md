# Cycle 7 — Runtime Ingestion (Logs and Traces)

## Goal

**Ingest logs and traces** from the locations and endpoints specified in the **project manifest** (from onboarding). Normalize them into a common event model (LogEvent, Trace, Span) and **link to the graph** (Service, Endpoint) so the context funnel and diagnose tools can retrieve “recent logs for service X” and “recent traces for endpoint Y.” Store minimal data in the graph (or in a dedicated log/trace store) and expose **search** and **query** via MCP tools.

---

## Prerequisites

- Cycle 1: Manifest has optional `logs` (provider, path/endpoint, format) and `tracing` (provider, endpoint).
- Cycle 2: GraphStore; we will link Service to log/trace store or store minimal LogEvent/Trace nodes.
- Cycle 5: Context funnel will be extended in Cycle 9 to include runtime evidence; this cycle only provides the data and tools.

---

## Tasks (Step-by-Step)

### 1. Normalized event model

- [ ] Define **LogEvent**: timestamp, service_name (or service_id), level (info/warn/error), message, attributes (map), optional trace_id/span_id, optional raw.
- [ ] Define **Trace**: trace_id, root_service, start_time, end_time, status (ok/error).
- [ ] Define **Span**: span_id, trace_id, parent_span_id, service_name, name (operation), kind, start_time, duration_ms, status, attributes.
- [ ] Document how to **resolve** service_name to graph Service node (by name and project_id).

### 2. Log ingestion: one provider

- [ ] Implement **file log** ingestion first (most universal):
  - Read from `manifest.logs.path` (support glob like `/var/log/app/*.log`).
  - **Tail** (follow) or **batch read** (last N bytes/lines). v1: batch read for last 24h or configurable window.
  - **Parse** by format: json (one JSON object per line), jsonl, or plain text (regex for timestamp, level, message).
  - Extract: timestamp, level, message; try to get service name from path (e.g. auth-service.log) or from a field (e.g. `service` in JSON).
  - Produce list of LogEvent; optionally **link** to Service node (create edge Service -[:PRODUCES_LOG]-> log_ref or store service_id in event).
- [ ] **Storage**: Option A — append to a **log store** (e.g. SQLite table with timestamp, service_id, level, message, trace_id; index by service_id, time). Option B — write to external Loki and only store “query handle” in graph. v1: Option A (SQLite or a simple log table) so we can search without external deps.
- [ ] **Retention**: Configurable (e.g. keep last 7 days); drop or archive older.

### 3. Trace ingestion: one provider

- [ ] Implement **OpenTelemetry** trace ingestion (OTLP gRPC or HTTP):
  - Connect to `manifest.tracing.endpoint`; receive trace/span export.
  - Or **pull** from Jaeger/Zipkin API if endpoint is a query URL.
  - Normalize to Trace and Span structs; set service_name from resource or span attributes.
  - **Link** to graph: match service_name to Service node; optionally link Span to Endpoint if operation name or attribute matches (e.g. http.route).
  - **Storage**: Option A — SQLite tables (traces, spans) with indexes by service_id, trace_id, time. Option B — external Tempo/Jaeger; store only pointers. v1: Option A for self-contained demo.
- [ ] If tracing is not configured (no endpoint), skip trace ingestion and return empty from trace queries.

### 4. Log store API

- [ ] **search_logs(service_id?, project_id?, query?, time_range?, limit)**:
  - Query the log store (SQL or full-text). If query is a string, simple LIKE or full-text search on message.
  - Filter by service_id and time_range. Return list of LogEvent (or summary lines).
  - Cap at `limit` (e.g. 100 lines).
- [ ] **get_recent_logs(service_id, minutes, level?)**: Convenience for “last N minutes”; optional filter by level (error/warn).

### 5. Trace store API

- [ ] **query_traces(service_id?, endpoint?, time_range?, limit)**:
  - Query spans/traces by service and time; optionally filter by operation/endpoint name.
  - Return list of Trace summaries (trace_id, root_service, duration, status, span_count) and optionally top N span paths (e.g. gateway → auth → redis).
- [ ] **get_trace_detail(trace_id)**: Return full span tree for one trace.

### 6. CLI and background ingestion

- [ ] **ctmint ingest [--project ./ctmint.yaml] [--once]**:
  - **Logs**: If manifest has logs, run file ingestion (batch or tail) and write to log store. If `--once`, run once and exit; else run in a loop with interval (e.g. 60s).
  - **Traces**: If manifest has tracing, connect to OTLP or pull from API; ingest and write to trace store. Same once/daemon behavior.
- [ ] Optional: run `ctmint ingest` as a **background** process when `ctmint serve` starts, so that logs/traces are always fresh. Document in README.

### 7. MCP tools

- [ ] **search_logs(service, query?, time_range?, limit?)**: Input: service name (resolve to service_id), optional query string, optional time range (e.g. last 15m), optional limit. Return matching log lines (or summary).
- [ ] **query_traces(service?, endpoint?, time_range?, limit?)**: Return trace summaries for the scope. Optional: include span path (e.g. “auth-service → redis 2.1s”).
- [ ] Wire in MCP server; document that these require logs/tracing configured in manifest and at least one ingest run.

### 8. Graph linkage

- [ ] Ensure **Service** nodes can be found by name so that “recent logs for auth-service” uses the same service_id as the code graph. Optionally add edges Service -[:PRODUCES_LOG] or store service_id on each log row.
- [ ] Optional: add **LogEvent** or **Trace** as lightweight nodes in the graph with external_id pointing to the log/trace store row, so the funnel can “get all entities in scope” including recent logs/traces. v1 can skip this and query log/trace store by service_id directly.

### 9. Tests

- [ ] Unit test: parse a sample log file (JSON and plain text) into LogEvent list; assert fields.
- [ ] Unit test: with a mock trace (list of spans), normalize to Trace/Span and assert linkage to service.
- [ ] Integration: write a few log lines to a temp file; run ingest --once with manifest pointing to that file; run search_logs and assert results.
- [ ] Test “no logs/tracing configured”: ingest exits 0 and does not fail.

---

## Deliverables

- Log ingestion (file provider) and log store (e.g. SQLite); normalized LogEvent.
- Trace ingestion (OTLP or Jaeger API) and trace store; normalized Trace/Span.
- search_logs and query_traces APIs; CLI `ctmint ingest`.
- MCP tools: search_logs(service, query?, time_range?, limit?), query_traces(service?, endpoint?, time_range?, limit?).
- Graph linkage: logs and traces scoped by service_id (and project_id).
- Tests: parsing, ingest → search, “no config” path.

---

## Definition of Done

- [ ] After onboarding with log path and tracing endpoint, and after `ctmint ingest` (once or daemon), search_logs and query_traces return data for the configured services.
- [ ] Context funnel (Cycle 9) can request “recent logs and traces for service X” and receive them for the diagnose flow.
- [ ] No crash when logs or tracing are not configured; tools return empty or “not configured.”

---

## Acceptance Criteria

- User can ask “What errors did auth-service log in the last 10 minutes?” and get real log lines.
- User can ask “Show me recent traces for POST /login” and get trace summaries with span paths.

---

## Estimated Duration

7–14 days (logs 3–5, traces 4–6, store design and MCP 2–3).
