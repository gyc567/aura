# Rust 编码智能体完整设计方案（v0.5）

> 🌐 **Language / 语言**: [English](coding-agent-design.md) · [中文](coding-agent-design.zh.md)

- **版本**：v0.5 设计稿
- **日期**：2026-08-07
- **状态**：待评审；本文档与 Phase 1 baseline 代码逐项对齐
- **基线参考**：
  - [`earendil-works/pi`](https://github.com/earendil-works/pi) — TypeScript 原版
  - [`gyc567/pi_agent_rust`](https://github.com/gyc567/pi_agent_rust) — Rust 原生端口
  - [Claude Code agent design lessons](https://jannesklaas.github.io/ai/2025/07/20/claude-code-agent-design.html)
- **配套规格**：[`plugin-spec-v2.md`](plugin-spec-v2.md)

---

## 1. 目标与非目标

### 1.1 目标

构建一个简单、可维护、可测试的 Rust 编码智能体。接收用户需求 → 收集受控工作区上下文 → 运行 `while(tool_use)` 循环 → 通过工具修改文件并运行验证 → 输出变更摘要 + 测试报告。

### 1.2 非目标（v1 明确不做）

| 能力 | 延后到 | 原因 |
|------|--------|------|
| 完整 TUI / 自动补全 | v2+ | KISS 优先验证非交互闭环 |
| 扩展/插件体系 | v2 | 独立规格 |
| 多 provider 路由 | v1 仅 OpenAI-compatible | — |
| 会话持久化 | v1.1 | Session 层落地后续跑 |
| 远程 RPC 协议 | v3+ | 需要稳定 wire 格式 |
| 长会话自动压缩 | v2+ | 与持久化绑定 |
| Critic / self-review 模式 | 不做 | Claude Code 实战证明不需要 |
| 长期记忆数据库 | 不做 | 同上 |

---

## 2. 设计原则

1. **KISS**：优先标准库与少量稳定依赖；一个模块只解决一个问题。
2. **高内聚、低耦合**：领域对象保持纯数据；外部 IO 通过窄接口注入。
3. **显式能力边界**：每个工具显式声明所需 capability。
4. **二阶段执行保护**：执行类工具先 capability gate，再命令中介按危险模式分类阻断。
5. **工具结果回执**：每个工具的结果都附带固定 `&'static str` 提醒。
6. **静态系统提醒**：根据工具类型 + TODO 状态静态生成系统提醒。
7. **可测试优先**：新增行为必须有单元测试；模型调用默认使用确定性 fake。
8. **增量兼容**：先识别现有项目接口，再以新增模块方式接入。
9. **证据驱动声明**：任何对外陈述必须能指向仓库内的 evidence artifact。
10. **Graceful 中断**：循环必须在收到 SIGINT 时优雅停止。
11. **参数校验先行**：工具执行前必须校验参数 schema。

---

## 3. 参考项目的取舍

| 维度 | pi（TS） | pi_agent_rust | Claude Code | 本方案 v0.5 |
|------|----------|----------------|-------------|------------|
| 核心循环 | while + 工具 | while + 工具 | while + 工具 | **while + 工具（采纳）** |
| TODO 规划 | 无显式 | 无 | **`TodoWrite` 是头号工具** | **`todo_write` v1 必含（采纳）** |
| 工具结果回执 | 无 | 无 | **每个工具结果后附固定提醒** | **v1 实现（采纳）** |
| 子智能体 | 外部 | 强类型 | **`Task` 工具，相同实例** | **v1.1 task 工具（采纳）** |
| 工具错误 | 立即终止 | 立即终止 | 回填给模型修正 | **v0.6 修订：回填 + ErrorBudget** |
| Critic | 无 | 无 | 无 | **明确不做** |
| 长期记忆 | 无 | JSONL | 无 | **明确不做** |

---

## 4. 总体架构

```
CLI (Non-interactive)
        │
        ▼
Agent (while loop driver)
  + Arc<StateMachine> recorder
  + Arc<AtomicBool> interrupt
        │
        ▼
  while !interrupted && budget.ok && decision.is_call()
    · model.complete() → decision
    · registry.execute(call) → output
    · messages.push(reminded_output)
        │
        ▼
Model Gateway ← HTTP / Fake
Tool Registry ← capability + command mediation
```

---

## 5. 模块设计

| 模块 | 作用 |
|------|------|
| `domain.rs` | `TaskRequest` / `Message` / `Decision` / `ToolCall` |
| `state.rs` | `AgentState` / `StateMachine` / `Budget` / `StopReason` |
| `model.rs` | `ModelGateway` trait + `ModelRequest` / `ModelResponse` |
| `model_http.rs` | OpenAI-compatible HTTP 适配器，含 SSE 解析 |
| `registry.rs` | `ToolRegistry` trait + `InMemoryRegistry` |
| `tool.rs` | `Tool` trait + `ToolSchema` / `ToolInput` / `ToolOutput` |
| `tools/todo_write.rs` | 头号工具：结构化 TODO 管理 |
| `tools/read_file.rs` | `FsRead` 能力 |
| `tools/write_file.rs` | `FsWrite` 能力，确认模式 |
| `tools/run_command.rs` | `Exec` 能力，四步执行 |
| `tools/list_dir.rs` / `grep_files.rs` / `find_files.rs` | `FsRead` 能力 |
| `reminders.rs` | 工具结果回执 + 静态系统提醒 |
| `context.rs` | 上下文收集与截断 |
| `policy.rs` | capability + command mediation |
| `precheck.rs` | regex 前置预检 |
| `event.rs` | `AgentEvent` / `EventSink` 审计流 |
| `error.rs` | `AgentError` 变体 |
| `cli.rs` | CLI 入口与参数 |
| `output.rs` | 文本和 JSON 报告格式 |

---

## 6. 工具清单（v1）

| 工具 | capability | `needs_confirmation` |
|------|------------|---------------------|
| `todo_write` | （无） | false |
| `read_file` | `FsRead` | false |
| `write_file` | `FsWrite` | true |
| `run_command` | `Exec` | true |
| `list_dir` | `FsRead` | false |
| `grep_files` | `FsRead` | false |
| `find_files` | `FsRead` | false |

---

## 7. Agent while 循环（v0.5 可编译形态）

核心循环伪代码：

```
loop {
    if interrupted.load() → return Aborted
    budget.check_turns()?

    req = ModelRequest::new(system_prompt, messages)
    resp = model.complete(req).await?

    call = resp.decision.into_tool_call()?
    output = registry.execute(call, ctx)
    messages.push(reminded_output(call, output))
    used_turns += 1
}
```

**唯一退出条件**：SIGINT / 预算耗尽 / `ErrorBudget` 耗尽 / 模型返回非 `Call`。

---

## 8. 错误处理

`AgentError` 变体（驱动 CLI 退出码）：

| 变体 | 退出码 |
|------|--------|
| `Config(String)` | 2 |
| `PathPolicy(String)` / `CommandPolicy(String)` | 3（策略拒绝） |
| 其他 | 1 |

---

## 9. 测试策略

| 层级 | 内容 |
|------|------|
| 领域单元测试 | 验证 `TaskRequest` / `Decision` / `Message` 等核心类型 |
| 工具单元测试 | 路径穿越、大小限制、命令预检、超时、输出截断 |
| 回执测试 | 校验每个工具的 reminder 完整性 |
| while 循环测试 | 6 类场景：成功 / Ask / Done / Fail / Absent / 预算耗尽 |
| 上下文测试 | 优先级、敏感文件、截断 |
| CLI smoke test | 退出码、stdout/stderr、配置读取 |

---

## 10. 覆盖率要求

`cargo llvm-cov --all-features --workspace --fail-under-lines 100 --fail-under-functions 100 --fail-under-regions 100`

Phase 1 已达到。

---

## 11. 实施阶段

| Phase | 内容 |
|-------|------|
| Phase 1 | 纯领域核心（已就位，38 测试） |
| Phase 2 | 工具 + 上下文 + 策略 + 回执 |
| Phase 3 | 模型适配 + while 循环 |
| Phase 4 | CLI 与报告 |
| Phase 5 | 质量门禁（fmt / test / clippy / llvm-cov / audit） |
| Phase 6（v1.1） | Session 地基 + 弹性循环 + 子代理 + scratchpad |
| Phase 7（v2） | Session 完整化 + compaction + 插件系统 |

---

## 12. 关键设计决策

| # | 问题 | 决议 |
|---|------|------|
| 1 | 交互式确认？ | v1 Non-interactive，`--yes` 跳过 |
| 2 | 子代理工具 | v1.1 启用，opt-in，构造期静态剥离，默认递归深度 2 |
| 3 | 命令中介 tier 默认值 | v1 用 balanced |
| 4 | `#![forbid(unsafe_code)]` | v1 启用 |
| 5 | 工具错误循环语义 | v0.6 修订：错误回填 + 错误预算（默认 3 次） |
| 6 | Session 持久化时机 | 提前到 v1.1（Phase 6 前置） |
