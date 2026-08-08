# Aura — 最小化 Rust 编码智能体

一个最小化、可测试的 Rust 编码智能体。接收任务 → 收集工作区上下文 → 运行 `while(tool_use)` 循环 → 修改文件并执行验证 → 输出变更摘要和测试报告。

🌐 **Language / 语言**: [English](README.md) · [中文](README.zh.md)

## 状态

**Phase 1–4 已完成。** v1 进行中。**v1.1 规划中**：会话层 + RLM 式子代理 + 工作记忆 — 详见 [`docs/architecture-roadmap.md`](docs/architecture-roadmap.md)。

- `cargo test`: 198 个测试，全部通过
- `cargo clippy`: 0 警告
- `cargo fmt --check`: 通过
- 覆盖率: ~91% (目标: 100%)

## 快速开始

```bash
# 构建
cargo build --release

# 使用 fake model 运行（不需要 API key，仅供测试）
cargo run --release -- \
  --workspace /tmp/my-project \
  --fake-model \
  "Add a README"

# 使用真实的 OpenAI-compatible 接口
cargo run --release -- \
  --workspace /tmp/my-project \
  --endpoint https://api.openai.com/v1 \
  --model gpt-4o \
  --api-key $OPENAI_API_KEY \
  "Add a README"

# JSON 输出
cargo run --release -- --workspace /tmp/my-project --fake-model --json "Add a README"
```

## 架构

```
┌─────────────────────────────────────────────────────────────┐
│  L1 表现层  CLI (aura-cli)                                  │
│         --workspace --max-turns --policy --resume --json   │
├─────────────────────────────────────────────────────────────┤
│  L2 会话层  Session (v1.1)  — JSONL transcript + artifacts  │
│                    + artifacts (scratchpad, children)     │
├─────────────────────────────────────────────────────────────┤
│  L3 执行层  Agent (while loop driver)                       │
│    while !interrupted && turns < budget && tool_errors < 3 │
│      → model.complete()                                     │
│      → if Decision::Call → registry.execute() → 回填       │
│      → else break (Done/Ask/Fail/Absent)                   │
├─────────────────────────────────────────────────────────────┤
│  L4 能力层  Tool Registry + Policy + Precheck + Reminders    │
├─────────────────────────────────────────────────────────────┤
│  L5 模型层  ModelGateway (OpenAI-compatible HTTP)            │
└─────────────────────────────────────────────────────────────┘
```

> v1 当前为单进程 CLI，单线程 `tokio` 运行时；v1.1 引入 Session 层 + RLM 式子代理，升级为 `multi_thread` 运行时。详见 [`docs/architecture-roadmap.md`](docs/architecture-roadmap.md)。

### 核心循环不变量 (v0.6)

- **唯一退出条件**：SIGINT / 预算耗尽 / `ErrorBudget` 耗尽 / 模型返回非 `Call`
- **工具错误回填给模型**：通过 `ErrorBudget`（默认 3 次），让模型自行修正；预算防止循环失控
- `recorder.transition()` 失败仅记录，不阻塞执行

## 关键设计决策

| 决策 | 理由 |
|------|------|
| 单一 `while` 循环 | Claude Code 的全部力量来源于此；复杂度在工具而非循环结构 |
| `todo_write` 是主要工具 | 显式规划胜于隐式 |
| 每次工具调用附加回执 | 静态回执胜于一次性的 system prompt |
| `run_command` 使用 regex 预检 | 快速、确定、无需 API 调用 |
| 工具错误回填给模型 + 错误预算 (默认 3 次) | 让模型自行修正；预算防止失控循环 (v0.6 修订, 见 `docs/architecture-roadmap.md` §4.1) |
| `Arc<AtomicBool>` 用于 SIGINT | async 上下文安全，无需 `block_on` |

## 模块

| 模块 | 作用 |
|------|------|
| `domain` | 核心类型: `TaskRequest`, `Decision`, `ToolCall`, `Message` |
| `state` | `AgentState`, `Budget`, `StateMachine`, `StopReason` |
| `model` | `ModelGateway` trait + `ModelRequest` / `ModelResponse` |
| `model_http` | OpenAI-compatible HTTP 适配器，含 SSE 解析 |
| `registry` | `ToolRegistry` trait + `InMemoryRegistry` |
| `tool` | `Tool` trait + `ToolSchema`, `ToolInput`, `ToolOutput` |
| `tools/todo_write` | v1 主要工具: 结构化 TODO 管理 |
| `policy` | 能力门禁 (`FsRead`, `FsWrite`, `Exec`) |
| `precheck` | 基于 regex 的命令风险分析 |
| `reminders` | 工具结果回执 + 系统提醒生成 |
| `context` | 工作区文件收集、敏感路径检测、截断 |
| `event` | `AgentEvent` + `EventSink` 审计流 |
| `agent` | `run()` 异步函数 — while 循环驱动 |
| `session` | (v1.1) `Session` + `Transcript` — 消息历史、工件、可恢复性 |
| `children` | (v1.1) RLM 式子代理 — `ChildRegistry`, admission handle, `agent_message` |
| `tools/scratchpad` | (v1.1) 持久化工作记忆 (`artifacts/scratchpad.json`) |
| `cli` | 基于 clap 的参数解析 |
| `output` | 文本和 JSON 报告格式 |

## 参考项目

- **[Claude Code](https://jannesklaas.github.io/ai/2025/07/20/claude-code-agent-design.html)** — 单循环 + TODO 工具 + 工具回执 + 同实例子智能体
- **[pi_agent_rust](https://github.com/gyc567/pi_agent_rust)** — 能力门禁 + 二阶段执行 + 证据驱动声明
- **[pi](https://github.com/earendil-works/pi)** — TypeScript 参考，模块拆分思路
- **[prime-agent](https://github.com/PrimeIntellect-ai/prime-agent)** — RLM 编程模型、会话/持久化、自改进 harness (v0.6 路线图: [`docs/architecture-roadmap.md`](docs/architecture-roadmap.md))

## 非目标 (v1)

- TUI / 交互模式
- 多 provider 路由 (仅 OpenAI-compatible)
- 会话持久化 — v1.1 通过 Session 层实现 (JSONL transcript + `--resume`), 详见 [`docs/architecture-roadmap.md`](docs/architecture-roadmap.md) §4.3
- 长期记忆 / 知识图谱
- Critic / 自我评审模式
- 远程 RPC 协议

## 开发

```bash
# 测试
cargo test --workspace

# 代码格式与 lint
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings

# 覆盖率 (需 llvm-tools-preview)
cargo llvm-cov test --workspace --fail-under-lines 100 --fail-under-functions 100

# 运行 CLI
cargo run --release -- --help
```

## 设计文档

完整设计思路: [`docs/coding-agent-design.md`](docs/coding-agent-design.md)

---

<a href="README.md">English Version</a>
