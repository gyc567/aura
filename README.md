# Aura — KISS Rust Coding Agent

A minimal, testable Rust coding agent. Receives a task → collects workspace context → runs a `while(tool_use)` loop → modifies files and runs verification → outputs a change summary with test report.

## Status

**Phase 1–4 complete.** v1 in progress.

- `cargo test`: 154 tests, all passing
- `cargo clippy`: 0 warnings
- `cargo fmt --check`: passing
- Coverage: ~80% (target: 100%)

## Quick Start

```bash
# Build
cargo build --release

# Run with fake model (no API key needed, for testing)
cargo run --release -- \
  --workspace /tmp/my-project \
  --fake-model \
  "Add a README"

# Run with real OpenAI-compatible endpoint
cargo run --release -- \
  --workspace /tmp/my-project \
  --endpoint https://api.openai.com/v1 \
  --model gpt-4o \
  --api-key $OPENAI_API_KEY \
  "Add a README"

# JSON output
cargo run --release -- --workspace /tmp/my-project --fake-model --json "Add a README"
```

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                    CLI (aura-cli)                   │
│         --workspace --max-turns --policy            │
└─────────────────────┬───────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────┐
│              Agent (while loop driver)              │
│  while !interrupted && turns < budget               │
│    → model.complete()                              │
│    → if Decision::Call → registry.execute()         │
│    → else break (Done/Ask/Fail/Absent)             │
└──────┬──────────────────────────────────┬───────────┘
       │                                  │
       ▼                                  ▼
┌──────────────┐               ┌──────────────────────┐
│   Model      │               │   Tool Registry      │
│  Gateway     │               │   (capabilities +    │
│  (trait)     │               │    command med.)     │
└──────────────┘               └──────────────────────┘
```

### Core Loop Invariants

- **Only exits on**: SIGINT / budget exhausted / model returns non-`Call` / tool error
- **Tool errors end the loop immediately** — never fed back to the model
- `recorder.transition()` failures are logged but never block execution

## Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| Single `while` loop | Claude Code's entire power comes from this; complexity is in tools, not loop structure |
| `todo_write` is the primary tool | Explicit planning beats implicit |
| Tool result reminders on every call | Static reminders beat one-shot system prompts |
| Regex pre-check for `run_command` | Fast, deterministic, no API call |
| Tool errors end loop immediately | Avoids hallucination from re-prompting on errors |
| `Arc<AtomicBool>` for SIGINT | Safe in async context without `block_on` |

## Modules

| Module | Purpose |
|--------|---------|
| `domain` | Core types: `TaskRequest`, `Decision`, `ToolCall`, `Message` |
| `state` | `AgentState`, `Budget`, `StateMachine`, `StopReason` |
| `model` | `ModelGateway` trait + `ModelRequest` / `ModelResponse` |
| `model_http` | OpenAI-compatible HTTP adapter with SSE parsing |
| `registry` | `ToolRegistry` trait + `InMemoryRegistry` |
| `tool` | `Tool` trait + `ToolSchema`, `ToolInput`, `ToolOutput` |
| `tools/todo_write` | Primary v1 tool: structured TODO list management |
| `policy` | Capability gates (`FsRead`, `FsWrite`, `Exec`) |
| `precheck` | Regex-based risk analysis for commands |
| `reminders` | Tool result reminders + system reminder generators |
| `context` | Workspace file collection, sensitive path detection, truncation |
| `event` | `AgentEvent` + `EventSink` for audit trail |
| `agent` | `run()` async function — the while loop driver |
| `cli` | Clap-based argument parsing |
| `output` | Text and JSON report formatting |

## Reference Projects

- **[Claude Code](https://jannesklaas.github.io/ai/2025/07/20/claude-code-agent-design.html)** — single loop + TODO tool + tool reminders + sub-agents via same instance
- **[pi_agent_rust](https://github.com/gyc567/pi_agent_rust)** — capability gates + two-phase execution + evidence-driven claims
- **[pi](https://github.com/earendil-works/pi)** — TypeScript reference, module decomposition ideas

## Non-Goals (v1)

- TUI / interactive mode
- Multi-provider routing (OpenAI-compatible only)
- Session persistence (JSONL / SQLite)
- Long-term memory / knowledge graphs
- Critic / self-review mode
- Remote RPC protocol

## Development

```bash
# Test
cargo test --workspace

# Lint
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings

# Coverage (requires llvm-tools-preview)
cargo llvm-cov test --workspace --fail-under-lines 100 --fail-under-functions 100

# Binary help
cargo run --release -- --help
```

## Design Document

Full design rationale: [`docs/coding-agent-design.md`](docs/coding-agent-design.md)
