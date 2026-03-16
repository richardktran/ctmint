# Cycle 9 — Diagnose MVP (End-to-End “Why Is X Failing?”)

## Goal

Deliver an end-to-end **diagnosis** flow: user asks “Why is payment-service returning 500?” or “Why is login API slow?” and the system returns a **structured answer** with hypotheses, evidence (logs, traces, code, schema), and optional suggested next steps. This cycle ties together: **onboarding** (manifest), **graph** (scope), **code** (snippets), **runtime** (logs, traces), **data** (schema), **context funnel** (compression), and **capability-based tools** (right tools for the question). Use a **deterministic planner** first (fixed sequence of tool calls) for reliability; optional LLM planner later.

---

## Prerequisites

- Cycle 1: Manifest with services, logs, DB, tracing (optional).
- Cycle 2: GraphStore (get_service_graph, get_architecture_map).
- Cycle 3: get_code_snippet, get_function_summary; code index.
- Cycle 4: search_code (scoped).
- Cycle 5: Context funnel (build_context, build_prompt).
- Cycle 6: get_db_schema, get_db_usage.
- Cycle 7: search_logs, query_traces.
- Cycle 8: Capability router, tool registry, filtered tool list.
- LLM endpoint configured for final answer generation.

---

## Tasks (Step-by-Step)

### 1. Diagnose input/output contract

- [ ] **diagnose_service(service_name, time_range?)**: Input: service name (string), optional time range (e.g. “15m”, “1h”). Output: structured diagnosis (see below).
- [ ] **diagnose_endpoint(method, path, time_range?)**: Input: HTTP method and path (e.g. “POST”, “/login”), optional time range. Output: same structure. Internally resolve endpoint to service(s) via graph, then run same pipeline.
- [ ] **Diagnosis output** (structured):
  - `summary`: short natural language answer (1–3 sentences).
  - `hypotheses`: list of { description, confidence (0–1), evidence_ids }.
  - `evidence`: list of { id, type (log|trace|code|schema), snippet, source (service/file/trace_id) }.
  - `suggested_actions`: optional list of next steps (e.g. “Check Redis connection pool,” “Add index on users.email”).
  - `metadata`: time_range, services_in_scope, tools_used.

### 2. Deterministic planner (template-based)

- [ ] For **diagnose_service(service_name, time_range)**:
  - **Plan** (fixed sequence): 
    1. get_service_graph(service_name) → scope (dependencies, endpoints, functions).
    2. search_logs(service_name, query = “error OR fail OR 500”, time_range, limit = 50).
    3. query_traces(service_name, time_range, limit = 20).
    4. get_architecture_map() (or reuse from step 1) for context.
    5. search_code(question = “error handling, timeout, retry”, service = service_name, top_k = 5).
    6. get_db_schema(service_name) if the service has READS/WRITES in graph (optional).
  - Execute steps in order; collect results. If a step fails (e.g. no logs configured), continue with others and note “logs: not available” in evidence.
- [ ] For **diagnose_endpoint(method, path)**:
  - Resolve (method, path) to service and optionally function via graph (Endpoint node → Service).
  - Then run the same plan with that service_name; optionally narrow traces to that endpoint (query_traces with endpoint filter).
- [ ] **Time range**: Default to “last 15 minutes” if not provided; parse “15m”, “1h”, “24h” into start/end time.

### 3. Evidence collection and deduplication

- [ ] **Collect** raw results from each tool call into a list of **evidence items**. Assign each an **evidence_id** (e.g. “log_1”, “trace_2”, “code_3”).
- [ ] **Deduplicate**: e.g. same log line or same trace_id not repeated. **Truncate**: cap at N log lines, M traces, K code snippets (e.g. 20, 10, 5).
- [ ] **Format** for LLM: short snippets (log: 2–3 lines per entry; trace: span path + duration; code: function name + 10 lines). Store in **ContextBundle** or a **DiagnosisContext** struct that the funnel can compress.

### 4. Context funnel integration

- [ ] **Build context for diagnosis**:
  - Use funnel’s **build_context** with question = user question and scope = service_name (and dependencies from graph).
  - **Add** runtime evidence: inject collected logs, traces, and (optionally) schema snippet into the bundle’s runtime_summary and raw_evidence.
  - **Token budget**: Apply same per-section and total caps; truncate evidence if over budget.
- [ ] **Build prompt**: Use funnel’s **build_prompt** or a diagnosis-specific template: “You are diagnosing a production issue. Context: … Evidence: … Question: … Respond with: summary, hypotheses with confidence, and suggested actions. Cite evidence by id.”

### 5. LLM call and response parsing

- [ ] **Send** the constructed prompt to the LLM (endpoint from config). **Parse** the response:
  - Extract summary (first paragraph or marked section).
  - Extract hypotheses (numbered list or “Hypothesis 1: …”).
  - Extract suggested actions.
  - If the LLM returns JSON, use it directly; else use regex or a second small LLM call to structure the answer.
- [ ] **Fallback**: If parsing fails, return raw LLM response plus raw evidence so the user still gets value.
- [ ] **Structured output**: Return the Diagnosis output struct (summary, hypotheses, evidence, suggested_actions, metadata) as the MCP tool result (JSON or equivalent).

### 6. MCP tools

- [ ] **diagnose_service(service_name, time_range?)**: Implement as above; return structured diagnosis.
- [ ] **diagnose_endpoint(method, path, time_range?)**: Resolve endpoint → service; run same pipeline; return same structure.
- [ ] Wire in MCP server; document that LLM endpoint must be configured and that logs/traces/DB are optional (diagnosis degrades gracefully if missing).

### 7. Graceful degradation

- [ ] If **logs** not configured: skip search_logs; add to metadata “logs: not configured.” Same for traces and DB.
- [ ] If **no results** (e.g. no errors in time range): still return a summary like “No errors found in the last 15m for service X. Code and schema context attached.” with code/schema evidence only.
- [ ] If **LLM fails**: return evidence only and a message “Could not generate summary; see evidence below.”

### 8. Tests

- [ ] **Unit test**: With mock tool responses (fixed log lines, one trace, one code snippet), run planner, build context, and assert prompt contains all evidence and is under token budget.
- [ ] **Integration test**: Run diagnose_service on a real project with real logs/traces (or fixture data); assert structured output has summary, at least one hypothesis, and evidence list. Optionally assert that the summary mentions the service name.
- [ ] **Degradation test**: Run with manifest that has no logs/tracing; assert diagnosis still returns (with code/schema only and “logs/traces not configured” in metadata).

---

## Deliverables

- diagnose_service(service_name, time_range?) and diagnose_endpoint(method, path, time_range?) with deterministic planner.
- Evidence collection, deduplication, truncation; integration with context funnel.
- LLM prompt template and response parsing; structured Diagnosis output.
- MCP tools wired; graceful degradation when logs/traces/DB are missing.
- Tests: unit (mock evidence), integration (real or fixture), degradation.

---

## Definition of Done

- [ ] User can ask “Why is payment-service returning 500?” and receive a structured answer with summary, hypotheses, evidence (logs/traces/code), and suggested actions.
- [ ] Evidence is cited (by id or inline) so the answer is traceable.
- [ ] When runtime data is missing, diagnosis still returns code and schema context and does not crash.
- [ ] End-to-end flow uses only manifest-driven config (no hardcoded paths or credentials).

---

## Acceptance Criteria

- Demo: run init → index → ingest (logs/traces) → serve; ask “Why is auth-service failing?” (with some errors in logs); get a coherent diagnosis that references real log lines and/or traces.
- diagnose_endpoint("POST", "/login") returns a diagnosis scoped to the login endpoint and its service.

---

## Estimated Duration

7–14 days.
