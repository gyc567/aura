# Aura 架构路线图（参考 prime-agent 的优化方案）

> 🌐 **Language / 语言**: [English](architecture-roadmap.en.md) · [中文](architecture-roadmap.md)

- **版本**:v0.6 候选
- **日期**:2026-08-08
- **状态**:已由人工拍板(见 §9 决议记录),待按演进路线实施
- **来源**:参考 [PrimeIntellect-ai/prime-agent](https://github.com/PrimeIntellect-ai/prime-agent) 的 RLM 编程模型、Continual Harness 与分层运行时理念
- **关系**:本文档是 [`coding-agent-design.md`](coding-agent-design.md) 的补充;主文档 §5.8、§11 已按本文档修订并指向此处

---

## 1. 目的与背景

Aura 目前是"单次任务、单进程、工具即函数"的 CLI 编码智能体(v1 进行中,154 测试 / 0 clippy 警告 / 覆盖率 91%)。prime-agent 是"长期运行、可自改进、Python 驱动的 RLM 智能体",两者定位不同,**不照搬**其 daemon 多进程与 IPython 内核,而是把它的三大理念"降级移植"进 Rust 单进程模型:

1. **RLM 编程模型**:上下文当"变量"、子智能体当"函数调用" → Aura 对应物是**持久工作记忆(scratchpad)**与 **RLM 式子代理**;
2. **Continual Harness**:补充状态可小步、证据驱动地演化 → Aura 对应物是 **Session 持久化** 与远期 **`/refine`-lite**;
3. **分层运行时**:表现 / 协调 / 会话 / 执行分离 → Aura 对应物是 **Session 层**从 `agent::run` 中抽出,使"会话"成为一等公民。

优化主线:在保持 KISS、100% 覆盖率门禁、证据驱动三项项目原则的前提下,引入**会话层、持久工作记忆、RLM 式子代理、弹性错误循环、分层上下文**,并修订现有 Phase 6/7 规划。

---

## 2. 差距矩阵(借鉴点 × Aura 现状)

| prime-agent 理念 | Aura 现状 | 差距 | 建议动作 |
|---|---|---|---|
| 持久计算环境(IPython) | 每轮全量重放上下文 + 工具结果回填 | 中间状态无处安放,上下文膨胀 | 持久工作记忆(scratchpad),§4.6 |
| 子代理 = 原生 `rlm()` 调用,admission handle + 异步 + `agent_message` | Phase 6 规划为同步 `TaskTool::spawn/await`(design §5.8 仍为 `todo!()`) | 同步模型无法并行、无法通信、无法保留 | RLM 式子代理:handle + 后台 task + 父作用域注册表 + 消息队列,§4.2 |
| 工具错误作为结果回填,模型自行修复 | **工具错误立即结束循环**(README 关键决策) | 单点失败即任务失败,模型无法自愈 | 错误回填 + 错误预算(默认 3 次),§4.1 |
| 会话持久化(JSONL transcript + artifacts + 恢复) | v1 非目标;仅有 `AgentEvent` 审计 + 空 `todo_final` | 中断即丢失,无法续跑 | Session 层 + JsonlTranscript + `--resume`,§4.3 |
| 自动 compaction(摘要 + 保留最近) | 截断(丢弃早期消息) | 丢弃信息,长任务失忆 | 分层上下文 + fast-model 摘要 compaction,§4.4 |
| 有界自主模式(turn/token/time + quality gates) | 仅 `max_turns`/`max_context_bytes` | 无验证门禁、无时间/令牌预算、gate 可能重复跑 | Budget 扩展 + QualityGate + 去重,§4.5 |
| skills 可编程、动态加载 | 静态注册表 `InMemoryRegistry`;插件 v2 已规划 | 能力扩展靠重编译 | 维持 v2 插件方案,§4.7 |
| `/refine` 自改进 harness | `LOOP.md`/`STATE.md` 人类驱动的人工自改进 | 智能体自身无反馈回路 | 远期:`/refine`-lite,证据驱动的建议式改进,§4.8 |
| daemon/supervisor 多进程 + agent 间全局通信 | 单进程 CLI | 重;与当前定位不符 | **不引入**,§6 |

---

## 3. 目标架构(五层)

```
┌──────────────────────────────────────────────────────────────┐
│  L1 表现层  cli / output / report(现有,扩展 --resume/--json)  │
├──────────────────────────────────────────────────────────────┤
│  L2 会话层  Session(新)                                       │
│    · 会话状态:消息历史 / 工件目录 / 子代理注册表 / goals       │
│    · Transcript trait → JsonlTranscript(append-only,可恢复)   │
│    · 生命周期:start / resume / stop / dump                    │
├──────────────────────────────────────────────────────────────┤
│  L3 执行层  agent::run(while 循环,操作 Session 而非裸 Vec)    │
│    · Budget(turn/token/time) + QualityGate 去重               │
│    · 上下文生命周期:工作记忆注入 → 核心窗口 → 历史摘要        │
│    · 错误回填 + 错误预算(替代立即终止)                        │
├──────────────────────────────────────────────────────────────┤
│  L4 能力层  ToolRegistry + Tool trait                         │
│    · 收敛为 7 个核心工具 + skills 动态注册(v2 插件)           │
│    · policy 门禁 / precheck 预检 / reminders 不变              │
├──────────────────────────────────────────────────────────────┤
│  L5 模型层  ModelGateway(升级 streaming 为一等公民)           │
└──────────────────────────────────────────────────────────────┘
```

核心变化:**把 `Vec<Message>` 从 `agent::run` 的局部变量提升为 `Session` 的一等状态**,循环只关心"决策 → 执行 → 回填",其余(持久化、子代理、记忆、预算)由 Session 与外围组件负责。与 prime-agent"worker 拥有 session、AgentSession 拥有执行"的分界一致,但收敛在单进程内。

---

## 4. 核心优化项

### 4.1 弹性错误循环(修订现有关键决策, R1)

**现状**:`src/agent.rs` 工具出错 → `ToolFailed` 立即 break;README 将其列为关键决策("避免从错误重新提示产生幻觉")。

**问题**:工具错误(编译失败、文件不存在、命令超时)在真实任务中占多数,立即终止等于一次失败就放弃。prime-agent 与 Claude Code 均将错误作为工具结果回填,让模型修正参数或换路径。

**prime-agent 借鉴**:prime-agent 的 worker loop 遇到 tool error 时构造 error result message 回填给模型,模型可以 self-correct。Aura 借鉴此机制但增加 `ErrorBudget` 封顶。

**目标设计**(已拍板,决议 R1):

- 工具执行失败 → 构造 `Message::Tool { success: false, output }` 回填,循环继续;
- 引入 `ErrorBudget`(`max_tool_errors`,默认 3):累计达上限才终止,保留"防失控";
- 错误回填时附带系统级提示(复用 `reminders` 机制):"上一个工具失败,请修正或换方案,不要重复同一调用"。

**行为对策表**:

| 错误类型 | v0.6 前 (工具错误→) | v0.6 修订 (R1) |
|----------|---------------------|---------------|
| `PathPolicy` (路径越界) | 立即终止 | 回填,模型换路径 |
| `CommandPolicy` (高危命令) | 立即终止 | 回填,模型换方法 |
| `ToolFailed` (执行失败) | 立即终止 | 回填,模型修正 |
| `InvalidArguments` (参数错误) | 立即终止 | 回填,模型修正 |
| `ErrorBudget` 耗尽 | N/A (无此机制) | 终止,写入 `ToolFailed` |

**权衡**:牺牲一点确定性换取自愈能力;错误预算保证上界。`tests/loop.rs` 中"工具失败即停"用例改为"错误预算耗尽即停",相关断言同步修订。

### 4.2 RLM 式子代理(修订 Phase 6)

**现状**:design §5.8 的 `TaskTool` 是同步 spawn/await 占位;runtime 是单线程,无法并行子代理。

**目标设计**(对齐 prime-agent `rlm()` 语义):

```
subagent 工具:  输入 { task, name?, model? } → 立即返回 admission handle
                 { child_id, name, session_dir, status: "running" }
后台:           tokio::spawn 子 agent 任务(独立消息历史、独立 transcript)
ChildRegistry:  父作用域注册表(Arc<Mutex<HashMap<ChildId, ChildHandle>>>)
                · list / status / fetch_result / delete
agent_message:  工具:parent → child 定向消息(邮箱队列);child 通过同一工具回复 parent
递归:           TaskRequest 增加 max_depth(继承,默认 2);深度 0 时 subagent 工具不可用
```

- runtime 从单线程升级为 `rt-multi-thread`(Cargo.toml 已含该 feature;`#[tokio::main(flavor = "multi_thread")]`);
- 每个子会话写独立 transcript 到 `artifacts/children/<child_id>.jsonl`;
- 父代理在后续轮次用 `subagent_result(child_id)` 收集结果——**不是返回值的同步等待**,与 prime-agent "results arrive only through explicit replies or files" 一致;
- `StateMachine` 增加 `RunningChild` 状态(可选);子代理轮次计入父会话预算;
- 保留 v0.5 决议 #6:构造期静态剥离 `subagent` 工具(深度 0 / 未 opt-in 时)。
- **prime-agent 对应**:`rlm(task, name?, model?)` 返回 admission handle,子代理后台运行,parent 通过 `agent_message` 收集结果。Aura 的 `subagent` + `agent_message` 工具直接镜像这一语义,但使用 `tokio::spawn` + `ChildRegistry` 而非 prime-agent 的 asyncio 子进程模型。

### 4.3 会话层与可恢复性(提前到 v1.1,已拍板,决议 R2)

**目标设计**:

- `Session` 结构:`session_id`、`workspace`、`messages`、`children: ChildRegistry`、`scratchpad`、`artifacts_dir`、`meta(created_at/model/config)`、`goal`(远期);
- `Transcript` trait:`append(Message)` / `replay() -> Vec<Message>`;实现 `JsonlTranscript`(append-only,原子写)与 `InMemoryTranscript`(测试用);
- `agent::run` 签名改为接收 `&mut Session`,循环内 `session.messages_mut()` 取代局部 `messages`;
- CLI 增加 `--resume <session.jsonl>`:重放 transcript 后从断点续跑(跳过已完成的工具调用);
- 现有 `EventSink`/`AgentEvent` 保留为**审计流**,与 transcript(完整消息记录)职责分开;
- 该层落地后 `RunReport.todo_final` 不再是空 `Vec<()>`,可直接从 Session 恢复。

### 4.4 分层上下文(截断 → compaction)

**现状**:`context::truncate_messages` 按优先级丢弃早期消息。长任务中"早期对话"往往含任务约束与已做决策,丢弃后模型失忆。

**目标设计**:

```
每轮注入 = 工作记忆摘要(scratchpad 条目名+大小)
          + 核心窗口(最近 N 条消息,全量)
          + 历史摘要(早期消息,由 fast model 或规则生成,仅一次)
```

- 触发阈值沿用 `Budget.max_context_bytes`(触发值 80%,而非全满);
- 摘要生成可用配置的 **fast model**(复用 Phase 6 已规划的 fast model 预检),无 fast model 时退化为现有截断(§2.5);
- 摘要写回 Session 持久层,支持审计:`AgentEvent::ContextCompacted { from_bytes, to_bytes, summary }`;
- compaction 不是完成信号:不终止 goals、子代理或后续轮次。

### 4.5 预算扩展与质量门禁

**现状**:`Budget { max_turns, max_context_bytes }`,仅 `check_turns`/`check_context`。

**目标设计**(对齐 prime-agent autonomous mode):

- `Budget` 扩展:`max_turns` / `max_tokens`(累计估算)/ `max_wall_time` / `error_budget`;
- `QualityGate`:`Vec<GateSpec { cmd, timeout, required }>`,在模型声明 `Done` 之前执行;
  - gate 失败 → 结果回填给模型继续修(复用 §4.1 错误回填);
  - **去重**:记录 `(gate_cmd, workspace_state_hash)`,工作区未变不重跑同一 gate(prime-agent 明确规避的行为);
- CLI:`--max-time`、`--gate "cargo test"` 等。

### 4.6 持久工作记忆(Rust 化的 RLM "上下文即变量")

不引入 IPython,给模型一个**跨轮次、可命名、落盘**的便签:

- `scratchpad` 工具:`set(name, value)` / `get(name)` / `append(name, value)` / `list()` / `clear()`,数据存 `artifacts/scratchpad.json`;
- 每轮注入的不是全量内容,而是**摘要索引**(名称 + 字节数 + 更新时间),模型按需 `get`;
- 典型用途:文件清单、解析结果、待办状态、命令输出片段——避免模型重复 `find_files`/重读文件,压缩上下文增长曲线;
- 与 `todo_write` 分工:`todo_write` 管计划,`scratchpad` 管数据。

### 4.7 工具系统收敛

v1 已有 7 个内置工具(`todo_write`/`read_file`/`write_file`/`run_command`/`find_files`/`grep_files`/`list_dir`)。最终收敛为 **7 个核心工具** + `scratchpad` + `subagent` + `agent_message` = **10 个**（v1.1) + 动态 skills:

```
todo_write · read_file · write_file · run_command · scratchpad(新)
subagent(新,v1.1) · agent_message(新,v1.1)
```

新增能力优先走"参数化 schema"(如 `run_command` 增加 `--check` 验证模式),其次才新增工具——对齐 prime-agent "不需要为每个能力建一个模型工具"。v2 插件系统(`plugin-spec-v2.md`)使 skills 动态注册到 `ToolRegistry`,能力扩展不再依赖重编译。

### 4.8 远期自改进(`/refine`-lite,Phase 8+)

任务结束生成 `RunReport` 后,可选调用一次 fast model 产出"harness 改进建议"(记忆/skill 草案),**只提议、不自动写入**,由人类在 `LOOP.md` 审批后落盘。与现有的人类门控循环天然兼容,且不违反 prime-agent "不覆盖不可变基础系统提示词" 的原则。

---

## 5. 模块布局(增量)

```
src/
  domain.rs            # 不变;新增 ChildId / AgentMessage / GateSpec 类型
  state.rs             # 不变;AgentState 可选增 RunningChild
  agent.rs             # 改为操作 Session;错误回填逻辑
  session/             # 新:L2
    mod.rs             #   Session 类型与生命周期
    transcript.rs      #   Transcript trait + JsonlTranscript
    artifacts.rs       #   工件目录:scratchpad / children / summaries
  context.rs           # 升级:分层注入 + 触发 compaction
  compaction.rs        # 新:摘要生成(fast model / 规则 fallback)
  children/            # 新(v1.1):ChildRegistry / handle / message / subagent tool
  gates.rs             # 新:QualityGate 执行 + (gate, hash) 去重
  budget.rs            # 新:turn/token/time/error 预算(或并入 state.rs)
  model.rs             # 升级:streaming 一等公民;token 估算
  tools/
    todo_write.rs      # 不变
    scratchpad.rs      # 新(§4.6)
    subagent.rs        # 新(§4.2)
    agent_message.rs   # 新(§4.2)
    ...其余不变
  plugin/              # v2 保留(plugin-spec-v2.md 已定)
```

`domain`/`state`/`precheck`/`policy`/`reminders`/`event` 的纯函数性质**保持不变**——这是 100% 覆盖率门禁能延续的前提。

---

## 6. 演进路线

| 阶段 | 内容 | 对现有规划的修订 |
|---|---|---|
| **v1 收尾** | 覆盖率 91% → 100%(cli/main/model_http) | 不变 |
| **v1.1 = Phase 6 修订版** | ① Session 消息管理子集(消息历史 + JsonlTranscript,§4.3)② 错误回填 + ErrorBudget(§4.1)③ RLM 式子代理 + multi-thread runtime(§4.2)④ scratchpad 工作记忆(§4.6)⑤ Budget/gate 基础(§4.5) | Phase 6 原只有子代理 + fast model 预检;Session 为公共地基,先行落地 |
| **v1.2** | **Bench Framework**（`docs/bench-framework.md`）：`aura bench run/report/init` + 8 种子任务 + 隔离 workspace + 量化指标 | 新增：评测体系是证据驱动改进的前提 |
| **v2 = Phase 7 修订版** | ① Session 完整生命周期 + `--resume`(§4.3)② 分层 compaction(§4.4)③ 插件系统(现 spec 照旧,复用 Session 做安装状态持久化) | 原 Phase 7 只有插件;Session 先行 |
| **v3+(远期)** | goals/heartbeat/调度(若出现长期任务场景);`/refine`-lite(§4.8);TUI(若需要) | 依赖 v2 Session 层 |
依赖顺序:`Session` 是子代理、compaction、refine 的公共地基,**即使从 v1.1 开工,也先落 Session 的消息管理子集,再叠其余**。

---

## 7. 明确不引入(保持 KISS)

- **不引入 daemon/supervisor 多进程**:Aura 是单次任务 CLI,`--resume` 已满足可恢复性;多进程只在该项目未来做常驻/TUI 时才值得;
- **不引入 IPython/Python 依赖**:工作记忆(§4.6)是最小等价物;
- **不引入 agent 间全局消息总线**:只有多会话常驻才需要;父-子定向消息(§4.2)已覆盖核心场景;
- **保留**:100% 覆盖率门禁、证据驱动、`todo!()` 占位先行、报告 schema `aura.report.v1`、`unsafe_code = "warn"`。

---

## 8. 风险与权衡

- **错误回填改变循环终止语义**:`tests/loop.rs` 中"工具失败即停"用例需改为"错误预算耗尽即停",相关断言同步修订;
- **multi-thread runtime**:`Arc<AtomicBool>` SIGINT 模型仍适用;工具 trait 已 `Send + Sync`,子代理共享 `ModelGateway`/`Registry` 引用需确认无状态泄漏(推荐 `Arc` 包裹);
- **Session 引入增大测试面**:JsonlTranscript 需原子写 + 崩溃恢复测试,建议 `tempfile` 夹具;
- **compaction 的 fast model 调用增加成本**:必须可配置开关,默认规则摘要(现有截断逻辑)兜底,避免覆盖率门禁依赖外部网络。

---

## 9. 决议记录(本次拍板)

| # | 问题 | 决议 |
|---|------|------|
| R1 | 工具错误循环语义 | **接受错误回填 + 错误预算(默认 3 次)**,修订"工具错误立即终止"决策(§4.1) |
| R2 | Session 持久化时机 | **提前到 v1.1**(Phase 6 前置),先落消息管理子集(§4.3) |
| R3 | 方案落盘 | **新增本文档 + 修订 `coding-agent-design.md` §5.8/§11 与 README 对应章节** |

---

## 10. 状态

- 本文档为架构路线图,非实施清单;各阶段实施时以本文档 + 主设计文档为准。
- 相关:`coding-agent-design.md` §5.8(子智能体)、§11(实施阶段)、§16(参考项目对照)已按本文档修订。
