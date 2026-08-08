# Aura — Aura 编码智能体

A minimal, testable Rust coding agent. Receives a task → collects workspace context → runs a `while(tool_use)` loop → modifies files and runs verification → outputs a change summary with test report.

## Status

**Phase 1–4 complete.** v1 in progress. **v1.1 planned**: Session layer + RLM-style subagents + working memory — see [`docs/architecture-roadmap.md`](docs/architecture-roadmap.md).

- `cargo test`: 198 tests, all passing
- `cargo clippy`: 0 warnings
- `cargo fmt --check`: passing
- Coverage: ~91% (target: 100%)

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
┌─────────────────────────────────────────────────────────────┐
│  L1 表现层  CLI (aura-cli)                                  │
│         --workspace --max-turns --policy --resume --json   │
├─────────────────────────────────────────────────────────────┤
│  L2 会话层  Session (v1.1)  — JSONL transcript + artifacts │
│                                                             │
├─────────────────────────────────────────────────────────────┤
│  L3 执行层  Agent (while loop driver)                       │
│    while !interrupted && turns < budget && tool_errors < 3 │
│      → model.complete()                                     │
│      → if Decision::Call → registry.execute() → 回填       │
│      → else break (Done/Ask/Fail/Absent)                   │
├─────────────────────────────────────────────────────────────┤
│  L4 能力层  Tool Registry + Policy + Precheck + Reminders   │
├─────────────────────────────────────────────────────────────┤
│  L5 模型层  ModelGateway (OpenAI-compatible HTTP)            │
└─────────────────────────────────────────────────────────────┘
```

> v1 当前为单进程 CLI，单线程 `tokio` 运行时；v1.1 引入 Session 层 + RLM 式子代理，升级为 `multi_thread` 运行时。详见 [`docs/architecture-roadmap.md`](docs/architecture-roadmap.md)。

### Core Loop Invariants (v0.6)

- **Only exits on**: SIGINT / budget exhausted / `ErrorBudget` exhausted / model returns non-`Call`
- **Tool errors feed back to the model** with `ErrorBudget` (default 3) — lets the model self-correct; budget prevents runaway loops
- `recorder.transition()` failures are logged but never block execution

## Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| Single `while` loop | Claude Code's entire power comes from this; complexity is in tools, not loop structure |
| `todo_write` is the primary tool | Explicit planning beats implicit |
| Tool result reminders on every call | Static reminders beat one-shot system prompts |
| Regex pre-check for `run_command` | Fast, deterministic, no API call |
| Tool errors feed back to the model with an error budget (default 3) | Lets the model self-correct; the budget prevents runaway loops (v0.6 revision, see `docs/architecture-roadmap.md` §4.1) |
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
| `session` | (v1.1) `Session` + `Transcript` — message history, artifacts, resumability |
| `children` | (v1.1) RLM-style subagents — `ChildRegistry`, admission handle, `agent_message` |
| `tools/scratchpad` | (v1.1) Persistent named working memory (`artifacts/scratchpad.json`) |
| `cli` | Clap-based argument parsing |
| `output` | Text and JSON report formatting |

## Reference Projects

- **[Claude Code](https://jannesklaas.github.io/ai/2025/07/20/claude-code-agent-design.html)** — single loop + TODO tool + tool reminders + sub-agents via same instance
- **[pi_agent_rust](https://github.com/gyc567/pi_agent_rust)** — capability gates + two-phase execution + evidence-driven claims
- **[pi](https://github.com/earendil-works/pi)** — TypeScript reference, module decomposition ideas
- **[prime-agent](https://github.com/PrimeIntellect-ai/prime-agent)** — RLM programming model, session/persistence, self-improving harness (v0.6 roadmap: [`docs/architecture-roadmap.md`](docs/architecture-roadmap.md))

## Non-Goals (v1)

- TUI / interactive mode
- Multi-provider routing (OpenAI-compatible only)
- Session persistence — v1.1 via the Session layer (JSONL transcript + `--resume`), see [`docs/architecture-roadmap.md`](docs/architecture-roadmap.md) §4.3
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
