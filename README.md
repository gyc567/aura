# Aura

A minimal, testable **coding agent** written in Rust. Give it a natural-language
instruction and a workspace; it runs a single `while(tool_use)` loop — collecting
context, calling tools (read/write/run/grep/…), fixing its own errors up to a
budget, and reporting what it changed.

> KISS: one loop, seven core tools, evidence-driven reports, zero-warning clippy.

## Status

| Area | Status |
|------|--------|
| Core loop (v1) | ✅ done — 345 tests, fmt/clippy clean |
| Bench framework (v1.2) | ✅ done — `aura bench run/report/init/list/diff` + 8 seed tasks |
| Session layer (Phase 7) | ✅ done — JSONL transcript + `--resume` + compaction |
| RLM subagents (Phase 6) | ✅ done — `subagent` / `agent_message` tools |
| Plugin v2 | ✅ done — dynamic skill loading (`src/plugin/`) |

## Install

### Option 1 — one-line install script (Linux/macOS)

```bash
curl -sSL https://raw.githubusercontent.com/gyc567/aura/main/release/install.sh | bash
```

Detects your OS/arch, downloads the matching release asset, and installs to
`~/.local/bin` (override with `AURA_INSTALL_DIR`). See
[`release/install.sh`](release/install.sh) for env overrides.

### Option 2 — from source

```bash
cargo install --path .      # installs the `aura` binary
cargo install --git https://github.com/gyc567/aura
```

Prebuilt binaries for macOS (ARM/x64), Linux (ARM64/x64) and Windows (x64) are
published as GitHub Releases (`aura-<os>-<arch>.tar.gz` / `.zip`), each bundled
with `README.md`, `LICENSE` and `config.example.toml`.

Requires Rust ≥ 1.85 (edition 2024) for source installs.

## Configuration

Aura reads `~/.config/aura/config.toml` (or `$XDG_CONFIG_HOME/aura/config.toml`,
or a custom path via `AURA_CONFIG`). Precedence:

```
CLI flags  >  config file  >  environment (AURA_API_KEY)
```

```toml
# ~/.config/aura/config.toml
endpoint = "https://api.openai.com/v1"
model = "gpt-4o"
# api_key = "sk-..."   # prefer AURA_API_KEY or --api-key
```

See [`config.example.toml`](config.example.toml) for the annotated template.

## Quick start

### 1. Fake model — no API key, deterministic (testing / CI)

```bash
aura --workspace /tmp/my-project --fake-model "Plan the work via todo_write"
```

The fake model replays a fixed script (a `todo_write` call, then `Done`), so you
can exercise the full loop without network access.

### 2. Real model — any OpenAI-compatible endpoint

```bash
export AURA_API_KEY=sk-...            # or pass --api-key

aura --workspace /tmp/my-project \
  --endpoint https://api.openai.com/v1 \
  --model gpt-4o \
  "Add a README"
```

### 3. Machine-readable output

```bash
aura --workspace /tmp/my-project --fake-model --json "Plan the work"
```

## CLI reference

```
aura [OPTIONS] <INSTRUCTION>
aura bench <run|report|init|list|diff>
```

| Option | Description | Default |
|--------|-------------|---------|
| `INSTRUCTION` | Natural-language task | required (unless `bench`) |
| `--workspace <PATH>` | Absolute path to target dir | current dir |
| `--max-turns <N>` | Max loop iterations | `12` |
| `--policy <strict\|balanced\|permissive>` | Tool policy level | `balanced` |
| `--tools <LIST>` | Tool whitelist (comma-separated) | `read_file,write_file,run_command,list_dir,grep_files,find_files,todo_write` |
| `--json` | Structured JSON report instead of text | off |
| `--resume <FILE>` | Resume a session from a JSONL transcript | — |
| `--fake-model` | Deterministic scripted model (no network) | off |
| `--endpoint <URL>` | OpenAI-compatible base URL | — |
| `--model <NAME>` | Model name | — |
| `--api-key <KEY>` | API key (or `AURA_API_KEY` env) | — |

### Bench framework

```bash
aura bench list                    # list seed tasks
aura bench run --parallel 4        # run all tasks, report pass/fail
aura bench run --tasks 'bench/tasks/*.yaml' --output bench/results/latest
aura bench report bench/results/latest
aura bench diff <base_dir> <current_dir>
aura bench init my-new-task        # scaffold a task spec
```

See [`docs/bench-framework.md`](docs/bench-framework.md) for the task-spec format.

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│  L1  Presentation   CLI (aura) — --workspace --json --resume │
├──────────────────────────────────────────────────────────────┤
│  L2  Session        Session + Transcript (JSONL, resumable)  │
├──────────────────────────────────────────────────────────────┤
│  L3  Execution      agent::run — while(tool_use) loop        │
│                     Budget(turns) + ErrorBudget(3)           │
├──────────────────────────────────────────────────────────────┤
│  L4  Capability     ToolRegistry + Policy + Precheck + Reminders │
│                     scratchpad · subagent · agent_message    │
├──────────────────────────────────────────────────────────────┤
│  L5  Model          ModelGateway (OpenAI-compatible HTTP/SSE)│
└──────────────────────────────────────────────────────────────┘
```

Core-loop invariants:

- **Only exit conditions**: SIGINT / budget exhausted / `ErrorBudget` exhausted /
  model returns something other than `Call`.
- **Tool errors are fed back to the model** via `ErrorBudget` (default 3) so it
  can self-correct; the budget prevents runaway loops.
- Compaction summarizes early context instead of dropping it; `--resume` replays
  a transcript from a checkpoint.

## Modules

| Module | Role |
|--------|------|
| `domain` | Core types: `TaskRequest`, `Decision`, `ToolCall`, `Message` |
| `state` | `AgentState`, `Budget`, `StateMachine`, `StopReason` |
| `agent` | `run()` — the async while-loop driver |
| `model` / `model_http` | `ModelGateway` trait + OpenAI-compatible HTTP adapter (SSE) |
| `session` | `Session` + `Transcript` — message history, artifacts, resume |
| `compaction` | Layered-context summarization (fast-model or rule fallback) |
| `children` | RLM-style subagents — `ChildRegistry`, handles, `agent_message` |
| `tools` | `todo_write`, `read_file`, `write_file`, `run_command`, `list_dir`, `grep_files`, `find_files`, `scratchpad`, `subagent`, `agent_message` |
| `bench` | TaskSpec / Runner / Summary — `aura bench` |
| `plugin` | Dynamic skill loading (plugin spec v2) |
| `policy` / `precheck` | Capability gates + regex command-risk analysis |
| `context` | Workspace collection, sensitive-path detection, truncation |
| `output` | Text and JSON report formats |

## Development

```bash
cargo test --workspace
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## Documentation

- [Design](docs/coding-agent-design.md) — full design rationale
- [Architecture roadmap](docs/architecture-roadmap.md) — v0.6+ evolution plan
- [Bench framework](docs/bench-framework.md) — task-spec format & runner
- [Plugin spec v2](docs/plugin-spec-v2.md) — dynamic skills
- [Loop engineering](docs/loop-engineering-tutorial.md) — project loop setup

## Inspired by

- [Claude Code](https://jannesklaas.github.io/ai/2025/07/20/claude-code-agent-design.html) — single loop + TODO tool + tool receipts
- [pi_agent_rust](https://github.com/gyc567/pi_agent_rust) — capability gates + evidence-driven reports
- [prime-agent](https://github.com/PrimeIntellect-ai/prime-agent) — RLM programming model, session persistence (see `docs/architecture-roadmap.md`)

🌐 **Language / 语言**: [中文版](README.zh.md)
