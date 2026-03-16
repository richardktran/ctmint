# Context Funnel – Technical Design (ContextMint)

## 1. Purpose and Scope

The **Context Funnel** is the component that reduces **system-wide data** (codebase, logs, traces, schema) into a **compact, task-specific context** that fits inside the LLM’s context window (e.g. 4k–32k tokens). It implements a **multi-layer reduction** so that the AI receives the right level of detail: architecture and summaries first, then targeted evidence (code snippets, log excerpts, trace summaries), rather than raw dumps.

Principle: **problem size → context size**, not **repo size → context size**.

---

## 2. High-Level Architecture

```text
                    Context Funnel (this component)
                                │
   Input: User question + Execution results (tool outputs) + Optional prior state
                                │
        ┌───────────────────────┼───────────────────────┐
        │                       │                       │
   Layer 1: Graph           Layer 2: Entity         Layer 3: Symbol
   narrowing                selection              extraction
   (SKG traversal)           (which services,       (which functions,
                             endpoints, tables)     files, spans)
        │                       │                       │
        └───────────────────────┼───────────────────────┘
                                │
        ┌───────────────────────┼───────────────────────┐
        │                       │                       │
   Layer 4: Runtime         Layer 5: Compression    Output: ContextBundle
   evidence                 (summarize, truncate,   (architecture + summaries
   (logs, traces,            token budget)           + snippets + metadata)
   metrics in scope)
        │                       │                       │
        └───────────────────────┼───────────────────────┘
                                │
                                ▼
                        LLM prompt construction
```

---

## 3. Funnel Layers in Detail

### 3.1 Layer 1: Graph Narrowing (SKG)

- **Input**: User question (and optionally entities already mentioned: service name, endpoint, table).
- **Action**: Traverse the SKG to determine **relevant entities**:
  - If the user says “login API”: find Endpoint `POST /login` → Service (e.g. auth-service) → dependencies (other services, redis, postgres).
  - If the user says “payment-service”: find Service payment-service → endpoints, dependencies, and tables used.
- **Output**: A set of **entity IDs** (and types): e.g. `[auth-service, redis, postgres, users table, login_user function, POST /login endpoint]`. This is the **scope** for all subsequent layers.

**Query patterns**: Same as in `knowledge-graph.md` (service topology, endpoint → implementation, service → tables).

---

### 3.2 Layer 2: Entity Selection and Prioritization

- **Input**: Scope from Layer 1; optional relevance hints from tool results (e.g. “traces show redis timeouts”).
- **Action**: Optionally **rank or filter** entities (e.g. if there are 20 tables, keep the 5 most relevant to the question or to recent errors). May use simple heuristics (e.g. tables that appear in recent error logs) or a tiny model.
- **Output**: A **reduced set** of entities that will be used to pull symbols and runtime evidence. Goal: keep the set small (e.g. 5–15 entities) so that Layers 3–4 stay bounded.

---

### 3.3 Layer 3: Symbol and Snippet Extraction

- **Input**: Entity set from Layer 2 (services, endpoints, functions, tables).
- **Action**:
  - For **code**: Load **semantic summaries** (Layer 2 representation) for the selected functions/modules; optionally load **raw code** (Layer 0) for a small number of critical functions (e.g. the handler for the failing endpoint).
  - For **schema**: Load table and column names, index names for the selected tables (from SKG or cached schema).
- **Output**: Structured **code/schema context**: e.g. “Function login_user: summary …; signature …; [optional] 20 lines of code.” Same for 2–3 more functions and 1–2 tables.
- **Token budget**: Reserve a fixed budget for this layer (e.g. 1500 tokens); truncate or drop lowest-priority symbols if over.

---

### 3.4 Layer 4: Runtime Evidence

- **Input**: Same entity set (services, endpoints); time range (from question or default “last 15 minutes”).
- **Action**:
  - **Logs**: Retrieve recent log lines (or already-fetched tool output) for the scoped services; filter by level (e.g. errors) if relevant.
  - **Traces**: Retrieve recent traces for the scoped endpoints/services; summarize (e.g. “gateway → auth-service → redis; auth-service span 2.3s”).
  - **Metrics**: Optional; e.g. “p99 latency for POST /login: 1.2s (up from 0.2s).”
- **Output**: Short **runtime summary** (e.g. “Redis timeout errors: 142 in last 10 min; trace shows auth-service → redis 2.1s.”) plus a few **representative raw lines** (e.g. 5–10 log lines, 1–2 trace summaries).
- **Token budget**: Cap total (e.g. 1000 tokens for runtime evidence).

---

### 3.5 Layer 5: Compression and Token Budget

- **Input**: All of the above: architecture snippet, entity list, symbol summaries/snippets, runtime summary and raw evidence.
- **Action**:
  - **Summarize** if still over budget: e.g. collapse 50 log lines into “Repeated ‘connection refused’ to Redis; 12 occurrences.”
  - **Truncate** code snippets to a max number of lines.
  - **Order** sections in a fixed template: e.g. (1) Question, (2) Architecture summary, (3) Relevant services/endpoints, (4) Code summaries and snippets, (5) Runtime evidence, (6) Instructions for the LLM.
- **Output**: **ContextBundle** (see below) and the **final prompt string** (or structured message list) for the LLM, under a **global token budget** (e.g. 6k–8k tokens for context, leaving room for system prompt and response).

---

## 4. ContextBundle Structure

The funnel produces a **ContextBundle** that the Orchestrator can pass to the LLM or to downstream tools.

**Suggested shape** (conceptual):

```text
ContextBundle:
  question: string
  architecture_summary: string       # Layer 3 style: "auth-service calls redis, postgres; implements POST /login"
  entities: List[{ id, type, name }] # Selected services, endpoints, functions, tables
  code_summaries: List[{ symbol_id, name, summary, signature?, snippet? }]
  schema_snippet: string             # Relevant tables and columns (and indexes if needed)
  runtime_summary: string            # Aggregated logs/traces/metrics in scope
  raw_evidence: List[{ type: log|trace|metric, content: string }]  # Small number of representative items
  metadata: { time_range, projects, token_estimate }
```

The Orchestrator (or a dedicated “prompt builder”) turns this into the actual **prompt** (system + user message with context + question).

---

## 5. Integration with Orchestrator and Tools

- **Orchestrator** calls the funnel **after** (or in parallel with) tool execution:
  - Planner decides which tools to call (e.g. get_service_graph, search_logs, query_traces).
  - Tool results are passed to the funnel as **raw evidence** and **hints** (e.g. “these traces mention redis timeouts”).
  - Funnel also queries **SKG** and **Vector Index** (via internal clients) to get graph scope and optional semantic hits within that scope.
- **Funnel** does **not** call the LLM; it only builds the context. The Orchestrator calls the LLM with the funnel’s output.

---

## 6. Token Budgets and Limits

Configurable per deployment or per request:

- **Total context budget**: e.g. 6000 tokens (so with 2k system + 1k response, total request stays under 10k).
- **Per-section caps**: e.g. code 2000, runtime 1500, architecture 500, summaries 1000.
- **Fallback**: If after compression the bundle still exceeds budget, funnel **drops** lowest-priority sections (e.g. raw evidence first, then extra code snippets) and optionally adds a line “Context truncated due to length.”

---

## 7. Caching and Optimizations

- **Architecture summary**: Can be **cached** per project (or per service) and invalidated on re-index.
- **Symbol summaries**: Already stored in SKG or a summary store; funnel just loads them (no recompute).
- **Runtime evidence**: Usually not cached (fresh per question); but if the same question is asked repeatedly in a short window, optional short TTL cache.

---

## 8. Error Handling

- **Missing graph data**: If SKG has no entry for the mentioned service/endpoint, funnel reports “No graph data for X” and continues with whatever scope it can derive (e.g. from tool results only).
- **Empty tool results**: If search_logs returns nothing, funnel still builds context from code and schema and notes “No recent logs in scope.”
- **Over budget**: Always truncate/summarize to stay under cap; never exceed the configured token limit.

---

## 9. Observability

- **Metrics**: Input entity count, output token count, time per layer, cache hit rate for summaries.
- **Structured logs**: Which entities were selected, which sections were truncated, and why (for debugging poor answers).

---

## 10. Summary

The **Context Funnel** turns a user question and raw tool/SKG/vector results into a **small, structured context** (ContextBundle) and ultimately a **prompt** that fits the LLM’s window. It does this by **graph narrowing** (Layer 1), **entity selection** (Layer 2), **symbol and snippet extraction** (Layer 3), **runtime evidence** (Layer 4), and **compression** (Layer 5). The result is **problem-sized context** instead of system-sized dump, so the AI can reason effectively without context explosion.
