# Aura 架构路线图（参考 prime-agent 的优化方案）

- **版本**: v0.6 候选
- **日期**: 2026-08-08
- **状态**: 已由人工拍板（见 §9 决议记录），待按演进路线实施
- **来源**: 参考 [PrimeIntellect-ai/prime-agent](https://github.com/PrimeIntellect-ai/prime-agent) 的 RLM 编程模型、Continual Harness 与分层运行时理念
- **关系**: 本文档是 [`coding-agent-design.md`](coding-agent-design.md) 的补充；主文档 §5.8、§11 已按本文档修订并指向此处

---

## 1. 目的与背景

Aura 目前是"单次任务、单进程、工具即函数"的 CLI 编码智能体（v1 进行中，154 测试 / 0 clippy 警告 / 覆盖率 91%）。prime-agent 是"长期运行、可自改进、Python 驱动的 RLM 智能体"，两者定位不同，**不照搬**其 daemon 多进程与 IPython 内核，而把它的三大理念"降级移植"进 Rust 单进程模型：

1. **RLM 编程模型**：上下文当"变量"、子智能体当"函数调用" → Aura 对应物是**持久工作记忆（scratchpad）**与 **RLM 式子代理**；
2. **Continual Harness**：补充状态可小步、证据驱动地演化 → Aura 对应物是 **Session 持久化** 与远期 **`/refine`-lite**；
3. **分层运行时**：表现 / 协调 / 会话 / 执行分离 → Aura 对应物是 **Session 层**从 `agent::run` 中抽出，使"会话"成为一等公民。

优化主线：在保持 KISS、100% 覆盖率门禁、证据驱动三项项目原则的前提下，引入**会话层、持久工作记忆、RLM 式子代理、弹性错误循环、分层上下文**，并修订现有 Phase 6/7 规划。

---

## 2. 差距矩阵（借鉴点 × Aura 现状）

| prime-agent 理念 | Aura 现状 | 差距 | 建议动作 |
|---|---|---|---|
| 持久计算环境（IPython） | 每轮全量重放上下文 + 工具结果回填 | 中间状态无处安放，上下文膨胀 | 持久工作记忆（scratchpad），§4.6 |
| 子代理 = 原生 `rlm()` 调用，admission handle + 异步 + `agent_message` | Phase 6 规划为同步 `TaskTool::spawn/await`（design §5.8 仍为 `todo!()`） | 同步模型无法并行、无法通信、无法保留 | RLM 式子代理：handle + 后台 task + 父作用域注册表 + 消息队列，§4.2 |
| 工具错误作为结果回填，模型自行修复 | **工具错误立即结束循环**（README 关键决策） | 单点失败即任务失败，模型无法自愈 | 错误回填 + 错误预算（默认 3 次），§4.1 |
| 会话持久化（JSONL transcript + artifacts + 恢复） | v1 非目标；仅有 `AgentEvent` 审计 + 空 `todo_final` | 中断即丢失，无法续跑 | Session 层 + JsonlTranscript + `--resume`，§4.3 |
| 自动 compaction（摘要 + 保留最近） | 截断（丢弃早期消息） | 丢弃信息，长任务失忆 | 分层上下文 + fast-model 摘要 compaction，§4.4 |
| 有界自主模式（turn/token/time + quality gates） | 仅 `max_turns`/`max_context_bytes` | 无验证门禁、无时间/令牌预算、gate 可能重复跑 | Budget 扩展 + QualityGate + 去重，§4.5 |
| skills 可编程、动态加载 | 静态注册表 `InMemoryRegistry`；插件 v2 已规划 | 能力扩展靠重编译 | 维持 v2 插件方案，§4.7 |
| `/refine` 自改进 harness | `LOOP.md`/`STATE.md` 人类驱动的人工自改进 | 智能体自身无反馈回路 | 远期：`/refine`-lite，证据驱动的建议式改进，§4.8 |
| daemon/supervisor 多进程 + agent 间全局通信 | 单进程 CLI | 重；与当前定位不符 | **不引入**，§6 |

---

## 3. 目标架构（五层）

```
┌──────────────────────────────────────────────────────────────┐
│  L1 表现层  cli / output / report（现有，扩展 --resume/--json） │
├──────────────────────────────────────────────────────────────┤
│  L2 会话层  Session（新）                                       │
│    · 会话状态：消息历史 / 工件目录 / 子代理注册表 / goals       │
│    · Transcript trait → JsonlTranscript（append-only，可恢复）  │
│    · 生命周期：start / resume / stop / dump                    │
├──────────────────────────────────────────────────────────────┤
│  L3 执行层  agent::run（while 循环，操作 Session 而非裸 Vec）  │
│    · Budget（turn/token/time）+ QualityGate 去重               │
│    · 上下文生命周期：工作记忆注入 → 核心窗口 → 历史摘要        │
│    · 错误回填 + 错误预算（替代立即终止）                        │
├──────────────────────────────────────────────────────────────┤
│  L4 能力层  ToolRegistry + Tool trait                         │
│    · 收敛为 7 个核心工具 + skills 动态注册（v2 插件）         │
│    · policy 门禁 / precheck 预检 / reminders 不变              │
├──────────────────────────────────────────────────────────────┤
│  L5 模型层  ModelGateway（升级 streaming 为一等公民）           │
└──────────────────────────────────────────────────────────────┘
```

核心变化：**把 `Vec<Message>` 从 `agent::run` 的局部变量提升为 `Session` 的一等状态**，循环只关心"决策 → 执行 → 回填"，其余（持久化、子代理、记忆、预算）由 Session 与外围组件负责。

---

## 4. 核心优化项

### 4.1 弹性错误循环（修订现有关键决策，R1）

**现状**：`src/agent.rs` 工具出错 → `ToolFailed` 立即 break；README 将其列为关键决策。

**目标设计**（已拍板，决议 R1）：

- 工具执行失败 → 构造 `Message::Tool { success: false, output }` 回填，循环继续；
- 引入 `ErrorBudget`（`max_tool_errors`，默认 3）：累计达上限才终止，保留"防失控"；
- 错误回填时附带系统级提示："上一个工具失败，请修正或换方案，不要重复同一调用"。

### 4.2 RLM 式子代理（修订 Phase 6）

```
subagent 工具：  输入 { task, name?, model? } → 立即返回 admission handle
                 { child_id, name, session_dir, status: "running" }
后台：           tokio::spawn 子 agent 任务（独立消息历史、独立 transcript）
ChildRegistry：  父作用域注册表 · list / status / fetch_result / delete
agent_message：  工具：parent → child 定向消息（邮箱队列）
递归：           TaskRequest 增加 max_depth（继承，默认 2）
```

- runtime 从单线程升级为 `rt-multi-thread`；
- 每个子会话写独立 transcript 到 `artifacts/children/<child_id>.jsonl`；
- 父代理在后续轮次用 `subagent_result(child_id)` 收集结果——**不是返回值的同步等待**。

### 4.3 会话层与可恢复性（提前到 v1.1，已拍板，决议 R2）

- `Session` 结构：`session_id`、`workspace`、`messages`、`children: ChildRegistry`、`scratchpad`、`artifacts_dir`、`meta`；
- `Transcript` trait：`append(Message)` / `replay() -> Vec<Message>`；
- `agent::run` 签名改为接收 `&mut Session`；
- CLI 增加 `--resume <session.jsonl>`：重放 transcript 后从断点续跑。

### 4.4 分层上下文（截断 → compaction）

```
每轮注入 = 工作记忆摘要（scratchpad 条目名+大小）
          + 核心窗口（最近 N 条消息，全量）
          + 历史摘要（早期消息，由 fast model 或规则生成，仅一次）
```

- 触发阈值：`Budget.max_context_bytes` 的 80%；
- 摘要写回 Session 持久层。

### 4.5 预算扩展与质量门禁

- `Budget` 扩展：`max_turns` / `max_tokens` / `max_wall_time` / `error_budget`；
- `QualityGate`：`Vec<GateSpec { cmd, timeout, required }>`，在模型声明 `Done` 之前执行；
- **去重**：记录 `(gate_cmd, workspace_state_hash)`，工作区未变不重跑同一 gate。

### 4.6 持久工作记忆

- `scratchpad` 工具：`set(name, value)` / `get(name)` / `append(name, value)` / `list()` / `clear()`，数据存 `artifacts/scratchpad.json`；
- 每轮注入的不是全量内容，而是**摘要索引**（名称 + 字节数 + 更新时间）。

### 4.7 工具系统收敛

最终收敛为 **10 个工具**（v1.1）：`todo_write` / `read_file` / `write_file` / `run_command` / `scratchpad`（新）/ `subagent`（新）/ `agent_message`（新）+ 3 个文件系统工具。

### 4.8 远期自改进（`/refine`-lite，Phase 8+）

任务结束生成 `RunReport` 后，可选调用一次 fast model 产出"harness 改进建议"，**只提议、不自动写入**，由人类在 `LOOP.md` 审批后落盘。

---

## 5. 模块布局（增量）

```
src/
  session/
    mod.rs             # L2: Session 类型与生命周期
    transcript.rs      # Transcript trait + JsonlTranscript
    artifacts.rs       # 工件目录
  compaction.rs        # 新：摘要生成
  children/            # 新（v1.1）
  gates.rs             # 新：QualityGate
  tools/
    scratchpad.rs      # 新（§4.6）
    subagent.rs        # 新（§4.2）
    agent_message.rs   # 新（§4.2）
```

---

## 6. 演进路线

| 阶段 | 内容 |
|---|---|
| **v1 收尾** | 覆盖率 91% → 100% |
| **v1.1 = Phase 6 修订版** | Session 消息管理子集 + 错误回填 + RLM 式子代理 + scratchpad + Budget/gate 基础 |
| **v1.2** | **Bench Framework**：`aura bench run/report/init` + 8 种子任务 |
| **v2 = Phase 7 修订版** | Session 完整生命周期 + `--resume` + 分层 compaction + 插件系统 |
| **v3+（远期）** | goals/heartbeat + `/refine`-lite + TUI（若需要） |

---

## 7. 明确不引入（保持 KISS）

- **不引入 daemon/supervisor 多进程**
- **不引入 IPython/Python 依赖**
- **不引入 agent 间全局消息总线**
- **保留**：100% 覆盖率门禁、证据驱动、`todo!()` 占位先行、`unsafe_code = "warn"`

---

## 8. 风险与权衡

- **错误回填改变循环终止语义**：`tests/loop.rs` 中相关断言需同步修订；
- **multi-thread runtime**：`Arc` 包裹 `ModelGateway`/`Registry` 需确认无状态泄漏；
- **Session 引入增大测试面**：JsonlTranscript 需原子写 + 崩溃恢复测试。

---

## 9. 决议记录（本次拍板）

| # | 问题 | 决议 |
|---|------|------|
| R1 | 工具错误循环语义 | **接受错误回填 + 错误预算（默认 3 次）** |
| R2 | Session 持久化时机 | **提前到 v1.1**（Phase 6 前置） |
| R3 | 方案落盘 | **新增本文档 + 修订 `coding-agent-design.md` §5.8/§11** |
