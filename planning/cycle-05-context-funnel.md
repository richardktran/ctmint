# Cycle 5 — Context Funnel (Multi-Layer Reduction, Token Budget)

## Goal

Implement the **Context Funnel** so that for a given user question we:

1. **Narrow** the search space using the graph (which services/endpoints/functions are relevant).
2. **Extract** symbol summaries and a small number of code snippets (from graph + vector search).
3. **Compress** into a fixed **token budget** and produce a **ContextBundle** (or prompt-ready string) for the LLM.

This cycle is **code-only** (no runtime logs/traces yet); runtime evidence is added in Cycle 7 and funnel extended in Cycle 9.

---

## Prerequisites

- Cycle 2: GraphStore with get_architecture_map, get_service_graph.
- Cycle 3: get_code_snippet, get_function_summary.
- Cycle 4: search_code with service scope.
- LLM endpoint configured (for the final “explain” or “diagnose” step; funnel itself can be tested with mock LLM).

---

## Tasks (Step-by-Step)

### 1. Funnel input/output types

- [ ] Define **FunnelInput**: user_question (string), optional project_id, optional service_name or endpoint (to force scope).
- [ ] Define **ContextBundle** (output):
  - question, architecture_summary (string), entities (list of { id, type, name }), code_summaries (list of { symbol_id, name, summary, snippet? }), schema_snippet (optional string), runtime_summary (optional, leave empty for this cycle), raw_evidence (optional list), metadata (time_range, token_estimate).
- [ ] Define **token budget** config: max total tokens for context (e.g. 6000), and per-section caps (e.g. code 2000, architecture 500).

### 2. Layer 1: Graph narrowing

- [ ] **resolve_scope(question, optional service/endpoint)**:
  - If service or endpoint is given, find that Service/Endpoint node and collect direct dependencies (get_service_graph).
  - If only question: optionally use keyword extraction (e.g. “login” → find Endpoint with path containing login, then get service and dependencies). v1 can require the client to pass service/endpoint.
  - Output: list of **entity ids** (services, endpoints, functions, tables) that are in scope.
- [ ] **architecture_summary**: Call get_architecture_map for project; filter to in-scope services; format as short text (e.g. “auth-service calls redis, postgres; implements POST /login”). Cap length (e.g. 500 chars or 100 tokens).

### 3. Layer 2: Entity selection (optional simplification)

- [ ] From the scope list, **prioritize**: e.g. take up to N services, M functions, K endpoints. If the list is large, keep the “most relevant” (e.g. the one explicitly mentioned, then direct dependencies). v1: take all in scope up to a hard cap (e.g. 20 entities).

### 4. Layer 3: Symbol extraction and code snippets

- [ ] For each selected Function (and optionally Endpoint), call **get_function_summary(symbol_id)** and optionally **get_code_snippet(symbol_id)**.
- [ ] **Token budget**: Reserve a max number of tokens for “code” (e.g. 2000). Add summaries first; then add snippets until budget is full. Truncate snippets if needed (e.g. max 30 lines per function).
- [ ] If no explicit scope, run **search_code(question, service?, top_k)** with scope from Layer 1; add those chunks to code_summaries/raw_evidence. Cap total code tokens.

### 5. Layer 4: Runtime evidence (stub)

- [ ] Leave **runtime_summary** and **raw_evidence** for logs/traces empty in this cycle. Or add a placeholder: “No runtime data in scope for this cycle.”
- [ ] Schema_snippet: optional — if Cycle 6 (data ingestion) is done, you can add a short “tables used: X, Y” here; else leave empty.

### 6. Layer 5: Compression and final bundle

- [ ] **Assemble** ContextBundle: architecture_summary + entities + code_summaries (+ optional schema_snippet). Compute **token_estimate** (e.g. chars/4 or use a small tokenizer).
- [ ] If over budget: **truncate** (drop lowest-priority snippets or shrink architecture summary); optionally add a line “Context truncated to fit budget.”
- [ ] Return ContextBundle and optionally a **prompt string** (template: system message + “Context: …” + “Question: …”).

### 7. Integration point for LLM

- [ ] Expose a function **build_context(question, scope?) -> ContextBundle** that the orchestrator or a high-level tool can call. Optionally **build_prompt(bundle) -> string** that formats the bundle into the exact prompt text for the LLM.
- [ ] Do not call the LLM inside the funnel; the caller (orchestrator or MCP tool) will send the prompt to the LLM.

### 8. MCP tool: explain_service / explain_endpoint

- [ ] **explain_service(service_name)**:
  - Call funnel with question = “Explain what this service does and its main components.” and scope = service_name.
  - Build ContextBundle; build prompt; call LLM (from config); return LLM response as the tool result.
- [ ] **explain_endpoint(method, path)** (optional):
  - Resolve endpoint to service; same flow with scope = that service/endpoint.
- [ ] Wire tools in MCP server; document that they require LLM endpoint in config.

### 9. Config

- [ ] **llm_endpoint**, **llm_model** (or equivalent) in global config. Funnel caller uses this to call the LLM.
- [ ] **context_budget**: max_tokens, per_section caps (optional).

### 10. Tests

- [ ] Unit test: with a mock graph (one service, two functions), run build_context and assert ContextBundle contains architecture_summary and two code summaries; token_estimate is set.
- [ ] Test truncation: pass a scope with 50 functions; assert output stays under token budget and some snippets are dropped.
- [ ] Integration: run explain_service on a real indexed project; assert response is non-empty and references the right service (no need to assert quality of LLM output).

---

## Deliverables

- Funnel implementation: build_context(question, scope?) -> ContextBundle; optional build_prompt(bundle).
- Token budgeting and truncation so context never exceeds configured cap.
- MCP tools: explain_service(service_name), optionally explain_endpoint(method, path).
- Tests: unit (mock graph), truncation, one integration with real project.

---

## Definition of Done

- [ ] For a given service name, explain_service returns an LLM-generated explanation that is grounded in the graph and code snippets (no hallucinated file names).
- [ ] Context size is always under the configured token budget.
- [ ] Funnel does not call LLM itself; the MCP tool or orchestrator does, using the bundle/prompt from the funnel.

---

## Acceptance Criteria

- Orchestrator (Cycle 9) can call build_context and then send the result to the LLM for “diagnose” flows.
- A user can ask “Explain auth-service” and get a short, coherent answer with correct code references.

---

## Estimated Duration

4–7 days.
