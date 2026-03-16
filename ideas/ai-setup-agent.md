# AI Setup Agent – Technical Design (ContextMint)

## 1. Purpose and Scope

The **AI Setup Agent** is an interactive onboarding flow that helps users configure ContextMint for a **new project** with minimal manual work. It:

- Asks a small set of questions (or infers from the environment) about: repo location, languages, log storage, database, tracing.
- Optionally **scans the repo** to detect stack (e.g. Python/Rust/Go from file names and config files).
- Generates a **project manifest** (and optionally adapter hints) that the rest of the system uses to run Code Parser, Indexer, Runtime Ingestion, and Data Ingestion.
- Can trigger the first **index** run so that the user gets a working SKG and MCP tools quickly (e.g. within 3–5 minutes).

The agent uses a **small local or remote LLM** only for interpretation and config generation; it does **not** implement the actual ingestion or graph logic—those remain in the core pipelines.

---

## 2. High-Level Architecture

```text
                    User (developer)
                            │
                            ▼
                    AI Setup Agent
                  (chat-style wizard)
                            │
        ┌───────────────────┼───────────────────┐
        │                   │                   │
   Repo Scanner        Question Flow        Config Generator
   (detect language,   (ask only when       (manifest + optional
    frameworks,         uncertain)           adapter params)
    structure)
        │                   │                   │
        └───────────────────┼───────────────────┘
                            │
                            ▼
                    Project Manifest
                    (YAML/JSON)
                            │
                            ▼
                    Adapters + Pipelines
                    (indexer, ingestion)
                            │
                            ▼
                    System Knowledge Graph
                    + MCP Core ready
```

---

## 3. Flow Overview

1. **Entry**: User runs `ctmint init` (or “Add project” in a UI). Agent starts.
2. **Repo path**: Ask or accept default (e.g. current directory). If path given, run **Repo Scanner**.
3. **Detection**: Scanner inspects:
   - File names: `requirements.txt`, `Cargo.toml`, `go.mod`, `package.json`, `pom.xml` → languages.
   - Directory layout: e.g. `services/auth`, `services/payment` → possible microservices.
   - Config files: `docker-compose.yml`, `k8s/`, `.env.example` → hints for DB, tracing endpoints.
4. **Questions**: Agent asks only when it cannot infer:
   - “Where is your source code?” (if not current dir)
   - “I found Python and Rust. Are these separate services?”
   - “Where are logs stored? (file path, Loki URL, or leave empty to skip)”
   - “Do you use OpenTelemetry or another tracer? If yes, endpoint?”
   - “Which database(s) should we introspect? (connection string or skip)”
5. **Manifest generation**: From answers + detected data, generate **project manifest** (see `mcp-core.md`, `Overview.md`).
6. **Validation**: Optionally validate (e.g. repo path exists, DB connection works with read-only user).
7. **First index**: Optionally run `ctmint index` and `ctmint ingest` (or equivalent) so the user can immediately ask “Why is X failing?” after setup.
8. **Exit**: Save manifest to project config file; print “Setup complete. Run `ctmint serve` to start the MCP server.”

---

## 4. Repo Scanner (Heuristics)

**Input**: Repository root path.

**Checks** (examples):

- **Languages**:
  - `requirements.txt`, `setup.py`, `pyproject.toml` → Python
  - `Cargo.toml` → Rust
  - `go.mod`, `go.sum` → Go
  - `package.json` → Node/TypeScript
  - `pom.xml`, `build.gradle` → Java
- **Structure**:
  - Top-level dirs like `services/`, `packages/`, `apps/` → possible multi-service
  - Monorepo tools: `nx.json`, `lerna.json`, `turbo.json` → multi-package
- **Logging**: Presence of `structlog`, `log4j`, `tracing` crates, or log config in env examples.
- **Tracing**: `opentelemetry`, `jaeger`, `zipkin` in deps or config.
- **Database**: `DATABASE_URL`, `POSTGRES_*`, `MYSQL_*` in `.env.example` or config files.

**Output**: A **detection result** (structured JSON) that the agent uses to prefill answers and to decide what to ask.

---

## 5. Question Flow and LLM Role

The agent can be **rule-based** (fixed sequence of questions with branches) or **LLM-driven** (small model chooses next question and interprets free-form answers).

**LLM responsibilities** (narrow scope):
- **Interpret** user’s natural language answer (e.g. “logs are in /var/log/app” → `logs.provider: file`, `logs.path: /var/log/app/*.log`).
- **Generate** the next question when multiple options exist (e.g. “Do you use one database or multiple?”).
- **Fill** manifest fields from the conversation (and merge with scanner output).

**What the LLM should NOT do**:
- Write custom ingestion code or complex adapter logic; the system uses the manifest to select **existing** adapters and configs.
- Execute arbitrary commands or access production systems beyond what the user explicitly configures (e.g. DB connection string provided by user).

**Model**: A small local model (e.g. 3B–7B via Ollama) or a fast cloud model is sufficient; no need for a large model for this task.

---

## 6. Project Manifest Output

The agent produces a **project manifest** that matches the schema expected by the rest of ContextMint. Example (see also `mcp-core.md`):

```yaml
project: my-app

services:
  - name: auth-service
    repo_path: .   # or path relative to manifest
    language: python
  - name: payment-service
    repo_path: ./services/payment
    language: rust

logs:
  provider: file
  path: /var/log/my-app/*.log
  format: json

tracing:
  provider: otel
  endpoint: http://localhost:4317

database:
  type: postgres
  connection: ${DATABASE_URL}   # or explicit (with env var for secrets)
  schema: public
```

Optional sections: `metrics`, `network`, `kubernetes` (for future capabilities). The pipelines (Code Parser, Indexer, Runtime/Data Ingestion) read this to know **what** to index and **where** to connect.

---

## 7. Adapter Selection from Manifest

The core does **not** contain project-specific code; it has **adapters** that are **configured** by the manifest. For example:

- **Code**: `language: python` → use Python parser adapter; `repo_path` tells where to run it.
- **Logs**: `provider: file`, `path: ...` → use file log adapter; `format: json` → use JSON parser.
- **Tracing**: `provider: otel` → use OTel trace adapter with `endpoint`.
- **Database**: `type: postgres` → use Postgres schema extractor with `connection`.

So the AI Setup Agent only needs to **generate correct manifest keys**; no code generation for adapters is required in v1.

---

## 8. Where the Agent Runs

- **Option A**: Same binary, separate subcommand: `ctmint init` starts an interactive loop (and optionally calls a local LLM via HTTP or in-process).
- **Option B**: Separate lightweight process (e.g. Python script or small Rust CLI) that only does wizard + manifest generation; the user then runs `ctmint index` and `ctmint serve` from the main binary. This keeps the main binary free of LLM dependencies if the core does not call an LLM for setup.
- **Option C**: Remote or IDE-embedded wizard that writes the manifest file; the rest is the same.

Recommendation for v1: **Option A** with an **optional** LLM (if no LLM endpoint is configured, use a fixed question flow and keyword parsing for answers).

---

## 9. Security and Safety

- **Secrets**: Do not store raw passwords in the manifest; use env var references (e.g. `connection: ${DATABASE_URL}`). Agent can remind the user to set env vars.
- **Validation**: If the user provides a DB URL, the agent (or a separate validation step) can test with a read-only ping; do not run destructive operations.
- **Scope**: The agent only writes the project config file and optionally triggers index/ingest; it does not modify source code or production systems.

---

## 10. Observability and Debugging

- **Logs**: Structured log of the flow (e.g. “detected Python, Rust; asked about logs; user said file /var/log”).
- **Output**: Emit the generated manifest to stdout or a file so the user can review and edit before re-running.

---

## 11. Summary

The **AI Setup Agent** shortens time-to-value for new projects by:
- **Detecting** repo structure and stack (languages, logs, DB, tracing).
- **Asking** only when necessary, with a small LLM to interpret answers and generate the next question.
- **Producing** a **project manifest** that drives all ingestion and adapter configuration.
- **Optionally** running the first index so the user can use MCP tools and “Why is X failing?” immediately after setup.

It keeps the core **adapter-based and manifest-driven**, so one ContextMint deployment can support many projects and stacks without hardcoding per-project logic.
