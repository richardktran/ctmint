# Cycle 1 — Onboarding (AI First: Source Code, Logs, DB, Tracing)

## Goal

Implement the **AI Setup Agent** so that a user can run `ctmint init` and, through a short interactive flow driven by an **embedded small AI model that runs locally**, specify:

- **Where is the source code?** (repo path, and which directories correspond to which services)
- **Where are logs located?** (file path, Loki URL, or “none”)
- **Database credentials** (connection string or env var reference, read-only; or “skip”)
- **Tracing** (OpenTelemetry/Jaeger endpoint or “none”)

The output is a valid **project manifest** (`ctmint.yaml`) that all later cycles read. **No user-supplied AI credentials** (no API keys, no external LLM endpoint). The model is **bundled or shipped with the binary** and runs **in-process or via a bundled runtime** on the user’s machine.

---

## Prerequisites

- Cycle 0 done: manifest schema defined and loadable; CLI has `ctmint init` stub.
- No external LLM service or user API keys required.

---

## Tasks (Step-by-Step)

### 1. Embedded small model: choice and packaging

- [ ] **Choose a small model** suitable for onboarding (interpret user answers, pick next question, output JSON):
  - Target size: **~1B–3B parameters** (e.g. TinyLlama, Phi-2 small, or a distilled model) so it runs on typical dev machines without GPU.
  - Format: **GGUF** (for Rust via `llama-cpp-rs` / `candle` / `llama-cpp-2`) or **ONNX** if your stack prefers it. Prefer one that has a pure-Rust or minimal-C dependency story for a single binary.
- [ ] **Decide packaging** (pick one and document):
  - **Option A**: Model file **shipped as separate asset** (e.g. in release tarball or next to the binary, or under `~/.ctmint/models/`). Binary loads it at runtime. First-run: prompt user to run `ctmint download-model` or auto-download from a fixed URL (same org/repo) with checksum.
  - **Option B**: **Download on first `ctmint init`** if model not found: fetch from a known URL, verify checksum, store under `data_dir/models/`. No credentials; URL is public.
  - **Option C**: Small model **bundled inside binary** (e.g. compiled into the binary or as embedded blob). Increases binary size significantly (e.g. +500MB–2GB); only if you want zero network and single-file UX.
- [ ] **No user AI credentials**: Do not ask for or read OpenAI/Anthropic/Ollama API keys or endpoints for the setup agent. All inference is local and credential-free.

### 2. Local inference runtime

- [ ] **Integrate inference** in the binary (or as a minimal in-process library):
  - Use a Rust crate that can load GGUF/ONNX and run inference (e.g. `candle`, `llama-cpp-2`, or `tract` for ONNX). Prefer **in-process** so there is no separate “Ollama must be running” requirement.
- [ ] **Resource limits**: Set max tokens for generation (e.g. 256–512); optional CPU thread count. Ensure init does not hang or OOM on low-memory machines; document minimum RAM (e.g. 4GB).
- [ ] **Fallback**: If model file is missing and download fails (e.g. offline), or inference fails, fall back to **non-AI question flow** (step 4 below) so `ctmint init` still works without the model.

### 3. Repo scanner (detection)

- [ ] Implement **repo scanner** that takes a root path and returns a detection result struct:
  - **Languages**: scan for `requirements.txt`, `pyproject.toml`, `Cargo.toml`, `go.mod`, `package.json`, `pom.xml`, `build.gradle` and set flags (e.g. `languages: ["python", "rust"]`).
  - **Structure**: detect top-level dirs like `services/`, `packages/`, `apps/`; optional monorepo hints (`nx.json`, `lerna.json`).
  - **Logging**: presence of `structlog`, `tracing`, `log4j` in config or dependency files.
  - **Tracing**: presence of `opentelemetry`, `jaeger`, `zipkin` in deps or config.
  - **Database**: grep or parse `.env.example`, `docker-compose.yml`, or config files for `DATABASE_URL`, `POSTGRES_*`, `MYSQL_*`.
- [ ] Return a **DetectionResult** (e.g. JSON or struct) that the wizard uses to prefill and to decide what to ask.
- [ ] Add tests: run scanner on a fixture repo (e.g. a small sample in `tests/fixtures/sample-repo`) and assert detected languages and structure.

### 4. Question flow (fallback, no AI)

- [ ] Define a **linear or branching question flow** used when the embedded model is unavailable (missing model, inference error, or offline):
  1. **Repo path**: “Enter path to source code (default: current directory).”
  2. **Services**: If scanner found multiple dirs (e.g. `services/auth`, `services/payment`), ask: “Treat these as separate services? (y/n)”. If yes, list them as separate `services` entries with `repo_path` each. If single repo, one service with `repo_path` = root.
  3. **Logs**: “Where are logs? (file path, e.g. /var/log/app/*.log | Loki URL | leave empty to skip).” If file path, set `logs.provider: file`, `logs.path: <path>`. Optionally ask “Log format: json / jsonl / text?”
  4. **Database**: “Database connection for schema introspection? (Postgres URL or ${ENV_VAR} or leave empty to skip).” If provided, set `database.type: postgres`, `database.connection: <value>`. Optionally ask schema name (e.g. `public`).
  5. **Tracing**: “OpenTelemetry or Jaeger endpoint? (URL or leave empty to skip).” Set `tracing.provider` and `tracing.endpoint`.
- [ ] Each answer is parsed (regex or keyword) into manifest fields. Store in an in-memory manifest struct.
- [ ] Validate: required fields present (at least `project`, `services` with at least one entry). Optional: test DB connection with read-only ping if user provided one.

### 5. AI-assisted flow (embedded local model)

- [ ] **Default path**: Use the **embedded small model** (from step 1–2) for:
  - **Interpret**: Given the user’s free-form answer and the current question, run a short prompt: “Extract into JSON: repo_path, log_path_or_url, database_url, tracing_endpoint. User said: <answer>. Return only valid JSON.” Parse the model output and fill manifest fields.
  - **Next question**: For ambiguous cases, prompt the model: “Given detection <DetectionResult> and answers so far <partial_manifest>, what single short question should we ask next? Options: ask_services, ask_logs, ask_database, ask_tracing, done. Return one word.” Then ask the corresponding question or finish.
- [ ] **No external calls**: All inference uses the local model only; no HTTP to OpenAI/Ollama/etc. No user API keys or credentials.
- [ ] **Fallback**: If the model is missing, download fails, or inference errors, switch to the **non-AI question flow** (step 4) so the user can still complete init.
- [ ] **Safety**: Use model output only as **data** (strings, JSON). All file writes and config decisions are done by our code; never execute shell commands or arbitrary code from model output.

### 6. Manifest writing

- [ ] After the flow completes, **write** the manifest to:
  - Default: `./ctmint.yaml` or `./.ctmint/project.yaml`.
  - Or path given by `--output` or “Where should I save the config?”
- [ ] Use the same schema and serialization as Cycle 0 so that `config::load_manifest(path)` can load it.
- [ ] Print success message: “Config written to … Run `ctmint index` to index the codebase.”

### 7. Init subcommand wiring

- [ ] `ctmint init [--path <repo>] [--output <file>]`:
  - If `--path` given, use it as repo root for scanner; otherwise use current directory.
  - Run scanner → **AI-assisted flow** (embedded model) if model is available, else **fallback question flow** → write manifest.
- [ ] Optional: `ctmint download-model` (or first-run auto-download) to fetch the onboarding model if using Option A or B in step 1.
- [ ] If manifest already exists at output path, ask “Overwrite? (y/n)” or support `--force`.

### 8. Documentation and UX

- [ ] Add a short “First-time setup” section to README: run `ctmint init` (no API keys required); optionally run `ctmint download-model` once if model is not bundled. Then run `ctmint index`.
- [ ] Document that the setup agent runs **fully locally** and does not send any data to external AI services.
- [ ] Optional: `ctmint init --demo` that generates a sample manifest without prompting (for CI or quick test).
- [ ] Optional: `ctmint init --no-ai` to force the non-AI question flow (useful for scripting or when model cannot run).

---

## Deliverables

- **Embedded small model** packaged with or downloadable for the binary; runs locally with **no user AI credentials**.
- `ctmint init` runs an interactive wizard (AI-assisted when model available, fallback otherwise) and produces a valid `ctmint.yaml`.
- Repo scanner detects languages and structure; wizard uses it to prefill and reduce questions.
- AI interprets free-form answers and suggests next question using the local model only.
- At least one test: run init in a fixture repo (non-interactive or with canned answers) and assert output manifest contains expected project, services, and optional logs/database/tracing.
- At least one test: run init with model disabled (e.g. `--no-ai` or missing model) and assert fallback flow produces valid manifest.

---

## Definition of Done

- [ ] User can run `ctmint init` **without entering any AI/LLM credentials**; the setup agent uses only the embedded local model.
- [ ] When the model is available, free-form answers are interpreted and the flow feels conversational; when the model is missing or fails, the fallback question flow still completes successfully.
- [ ] Manifest is loadable by the same loader from Cycle 0; all later cycles can read repo path, logs, DB, tracing from it.
- [ ] No credentials stored in plain text in docs; connection strings use env var references where appropriate (e.g. `connection: ${DATABASE_URL}`).
- [ ] Scanner + wizard tested on at least two layouts: single-service repo and multi-service (e.g. `services/a`, `services/b`).

---

## Acceptance Criteria

- New user can set up a project in under 10 minutes with **zero AI API keys or accounts** and have a correct `ctmint.yaml` for use in Cycle 2 and Cycle 3.
- If the user skips logs/DB/tracing, manifest still validates and later cycles can treat those as “not configured.”
- Documentation states clearly that onboarding runs fully on-device and does not require cloud AI services.

---

## Estimated Duration

5–9 days (2–3 for model choice, packaging, and local inference; 2–3 for repo scanner and fallback flow; 2–3 for AI-assisted flow and tests).
