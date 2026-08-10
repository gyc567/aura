# Aura Bench Framework — 评测框架设计

> 🌐 **Language / 语言**: [English](bench-framework.en.md) · [中文](bench-framework.md)

**版本**: v0.1
**日期**: 2026-08-08
**参考**: [Harbor Framework](https://www.harborframework.com) (Terminal-Bench 2.0 harness) + [tbench.ai](https://tbench.ai)

---

## 1. 目的

Aura Bench Framework 为 Aura coding agent 提供**标准化、隔离、可复现**的评测能力。

### 核心问题

- "这次改动让 agent 变好了还是变差了？" → 需要 baseline 对比
- "新工具 / 新策略 / 新模型" 能否提升成功率？ → 需要量化指标
- 哪些任务类型是 Aura 的弱项？ → 需要细粒度诊断

### Harbor 对应关系

| Harbor 概念 | Aura 对应 |
|---|---|
| Terminal-Bench 任务集 | Aura `bench/tasks/` 任务定义 |
| Oracle solution | Reference implementation（人写正确答案） |
| `harbor run -a claude-code` | `aura bench run --agent aura` |
| Daytona sandbox | `aura bench run --sandbox docker` |
| HuggingFace leaderboard | 未来：`aura bench submit` → JSON 报告 |
| pass@k / time-to-solve | `aura bench report` 输出指标 |

---

## 2. 架构概览

```
aura bench
├── run        # 执行任务集
├── report     # 生成指标报告
├── submit     # 发布到 leaderboard（未来）
└── init       # 初始化新任务
```

```
bench/
├── tasks/              # 任务定义（YAML）
│   ├── hello-world.yaml
│   ├── write-tests.yaml
│   ├── fix-bug.yaml
│   └── ...
├── results/            # 每次运行的结果
│   └── 2026-08-08-run1/
│       ├── task-hello-world.json
│       └── ...
└── REFERENCE.md       # 任务编写指南
```

---

## 3. 任务定义（TaskSpec）

每个任务是一个 YAML 文件，定义在 `bench/tasks/` 目录下：

```yaml
# bench/tasks/write-tests.yaml
id: write-tests
name: "Write unit tests for a given module"

description: |
  Given a small Rust module (1-3 files, <100 lines), write comprehensive
  unit tests. Tasks range from testing simple functions to testing
  error handling paths.

difficulty: medium          # easy | medium | hard
category: testing          # testing | refactor | bugfix | feature | docs | infra
skills: ["rust", "testing"]

setup:
  - action: clone
    repo: "file://${AURA_TEMP_DIR}/fixtures/simple-calc"
    depth: 1
  - action: write
    path: "src/lib.rs"
    content: |
      /// Adds two integers.
      pub fn add(a: i32, b: i32) -> i32 { a + b }

      /// Subtracts b from a.
      pub fn sub(a: i32, b: i32) -> i32 { a - b }

instruction: |
  Write comprehensive unit tests for `src/lib.rs` in `tests/unit_test.rs`.
  Cover: add, sub, edge cases (overflow, underflow), and error paths.

verify:
  type: command            # command | file_exists | git_diff | cargo_test
  command: "cargo test --quiet"
  cwd: "${AURA_WORKSPACE}"
  timeout_seconds: 60
  # 退出码 0 = 通过

reference:
  # 人工写的参考答案（可选，用于判断"完美完成"）
  file: "reference/tests/unit_test.rs"
  coverage_target: 85       # 目标覆盖率

tags:
  - rust
  - testing
  - beginner-friendly
```

### verify 类型

| type | 说明 | 成功条件 |
|------|------|----------|
| `command` | 执行 shell 命令 | 退出码 0 |
| `file_exists` | 文件存在且非空 | 文件存在 |
| `git_diff` | 检查 git 变更范围 | diff 匹配 pattern |
| `cargo_test` | 运行 `cargo test` | 退出码 0 |
| `cargo_fmt` | 检查格式 | `cargo fmt --check` 通过 |

### setup 操作类型

| action | 说明 |
|--------|------|
| `clone` | 克隆 git 仓库到 workspace |
| `write` | 写入指定文件内容 |
| `mkdir` | 创建目录 |
| `copy` | 从 fixtures 复制文件 |

---

## 4. 执行流程

```
aura bench run [OPTIONS]
  --tasks <glob>           # 默认 bench/tasks/*.yaml
  --agent <agent-cmd>      # 要评测的 agent 命令，默认 cargo run --bin aura
  --sandbox <mode>         # none | docker | nix（默认 none）
  --output <dir>           # 结果目录，默认 bench/results/<timestamp>/
  --parallel <n>           # 并行任务数（默认 CPU 核数）
  --timeout <seconds>      # 单任务超时（默认 300s）
```

### 执行步骤

```
1. 解析任务定义
2. 为每个任务创建独立 workspace：
   - mkdir <tmpdir>/<task-id>
   - 执行 setup actions（clone / write / copy）
3. 启动 agent 运行任务：
   - cd <tmpdir>/<task-id>
   - 执行 agent 命令（带 instruction）
   - 捕获 stdout/stderr / 退出码 / 耗时 / 使用的 turns
4. 执行 verify 命令：
   - cd <tmpdir>/<task-id>
   - 运行 verify.command（带 timeout）
   - 记录 verify 退出码
5. 收集结果：
   - {task-id}.json（pass/fail、耗时、turns、error message）
   - workspace.tar.gz（可选，保存失败任务的 workspace）
6. 生成汇总报告
```

### 结果文件格式

```json
// bench/results/2026-08-08-run1/summary.json
{
  "run_id": "2026-08-08-run1",
  "timestamp": "2026-08-08T15:30:00Z",
  "agent": "aura (dev)",
  "total_tasks": 12,
  "passed": 8,
  "failed": 4,
  "pass_rate": 0.667,
  "total_wall_time_s": 184.3,
  "tasks": [
    {
      "task_id": "write-tests",
      "task_name": "Write unit tests for a given module",
      "difficulty": "medium",
      "category": "testing",
      "status": "passed",        // passed | failed | timeout | error
      "verify_exit_code": 0,
      "agent_wall_time_s": 12.4,
      "agent_turns": 3,
      "error": null,
      "workspace_snapshot": null  // path to tar.gz if failed
    }
  ],
  "by_category": {
    "testing": { "total": 3, "passed": 2, "rate": 0.667 },
    "bugfix":  { "total": 4, "passed": 3, "rate": 0.750 },
    "refactor": { "total": 2, "passed": 1, "rate": 0.500 }
  },
  "by_difficulty": {
    "easy":   { "total": 5, "passed": 4, "rate": 0.800 },
    "medium": { "total": 5, "passed": 3, "rate": 0.600 },
    "hard":   { "total": 2, "passed": 1, "rate": 0.500 }
  }
}
```

---

## 5. 报告命令

```
aura bench report <results-dir>
```

输出：

```
Aura Bench Report
=================
Run:        2026-08-08T15:30:00Z
Agent:      aura (dev)
Tasks:      12 total, 8 passed, 4 failed
Pass Rate:  66.7%
Wall Time:  184.3s total, 15.4s avg

By Category
  testing   3/3 ████████████ 100%  ▓▓▓▓▓▓▓▓▓▓
  bugfix    3/4 ████████░░░░  75%  ▓▓▓▓▓▓▓▓░░
  refactor  1/2 █████░░░░░░░  50%  ▓▓▓▓▓░░░░░
  feature   1/3 ███░░░░░░░░░  33%  ▓▓▓░░░░░░░

By Difficulty
  easy     4/5  ██████████░░░  80%  ▓▓▓▓▓▓▓▓▓░
  medium   3/5 ███████░░░░░░  60%  ▓▓▓▓▓▓░░░░
  hard     1/2 █████░░░░░░░░  50%  ▓▓▓▓▓░░░░░

Failed Tasks
  [X] fix-deadlock (timeout after 300s)
  [X] migrate-db-schema (verify: cargo test failed)
  [X] write-api-docs (verify: file not created)
  [X] optimize-hot-path (agent error: budget exhausted)
```

---

## 6. 初始任务集（Seed Tasks）

Phase 1 种子任务（计划 8 个，逐步扩展到 20+）：

| 任务 | difficulty | category | 验证方式 | 来源 |
|------|-----------|----------|----------|------|
| `hello-world` | easy | feature | `cargo test` 通过 | 新建 |
| `add-tests-to-lib` | easy | testing | `cargo test` 通过 | 新建 |
| `fix-compile-error` | easy | bugfix | `cargo build` 通过 | 新建 |
| `format-code` | easy | infra | `cargo fmt --check` 通过 | 新建 |
| `readme-from-spec` | medium | docs | 文件存在 | 新建 |
| `write-grep-tool` | medium | feature | `cargo test` + 黑盒测试 | 新建 |
| `refactor-duplication` | medium | refactor | `cargo test` + 编译通过 | 新建 |
| `implement-scratchpad-tests` | medium | testing | `cargo test` 通过 | 新建（基于已有代码） |

---

## 7. 与现有测试体系的关系

```
aura 测试金字塔
├── 单元测试（cargo test）
│   └── 验证单个模块的正确性
├── 集成测试（tests/*.rs）
│   └── 验证 agent 循环逻辑（FakeModel，无网络）
└── 基准测试（aura bench）NEW
    └── 验证 agent 在真实任务上的端到端表现
```

**关键区别**：

| | 现有 tests/ | aura bench |
|---|---|---|
| 执行方式 | `cargo test` | `aura bench run` |
| 网络依赖 | 无（FakeModel） | 真实 HTTP 调用 |
| 任务类型 | 单元/集成测试 | 端到端任务 |
| 隔离性 | 进程内 | 进程 + workspace 隔离 |
| 耗时 | 秒级 | 分钟级 |
| 触发条件 | 每次 CI | 手动 / 发布前 |
| 覆盖率 | 模块级 | 任务级 |

**不替换现有测试** — bench 是补充，不是替代。现有 215 个测试依然是主力质量门禁。

---

## 8. 实现计划

### Phase B1（v1.2，最小可用）

- [ ] `bench/` 模块：`TaskSpec` 解析 + `Workspace` 隔离
- [ ] `aura bench run` CLI 子命令
- [ ] 4 个种子任务
- [ ] 基础报告输出（pass rate + task list）
- [ ] `--parallel` 并行执行

### Phase B2（v1.3）

- [ ] 8 个种子任务（覆盖 easy/medium 各难度）
- [ ] `aura bench report` 美化输出
- [ ] `aura bench init <name>` 创建任务脚手架
- [ ] 结果 diff（对比两次运行的差异）
- [ ] Docker sandbox 模式（可选）

### Phase B3（v1.4+）

- [ ] Reference solution 支持（计算 oracle pass rate）
- [ ] pass@k 统计（k=1,3,5）
- [ ] 任务标签系统（rust/cargo/toml 等）
- [ ] Leaderboard 提交（`aura bench submit`）
- [ ] 任务自动生成（用 LLM 从自然语言生成 TaskSpec）

---

## 9. 关键设计决策

### 决策 1：workspace 隔离方式

**选项 A**：进程级隔离（`std::fs::create_dir` + `std::process::Command`）
- ✅ 简单，Rust 原生
- ❌ 不防恶意 `rm -rf /`

**选项 B**：Docker 容器（`docker run --rm -v <dir>:/workspace`）
- ✅ 安全隔离
- ❌ 需要 Docker daemon，CI 可能不支持

**决策**：Phase B1 用选项 A + 路径校验（workspace 必须在 `/tmp/aura-bench/` 下），Phase B2 加 Docker 选项。

### 决策 2：任务定义格式

**选项 A**：YAML（人写） + Rust serde 解析
- ✅ 人类友好，生态好
- ❌ 需要额外依赖

**选项 B**：JSON
- ✅ 标准库 `serde_json`
- ❌ 人类不友好

**选项 C**：Rust struct 直接定义（`bench_tasks.rs`）
- ✅ 类型安全
- ❌ 不灵活，修改需要重新编译

**决策**：YAML（`serde_yaml`）+ 代码生成 TaskSpec校验。

### 决策 3：Agent 接口

**选项 A**：直接调用 `run_agent()` 函数（in-process）
- ✅ 零开销，可访问内部状态
- ❌ 评测代码与 agent 代码耦合

**选项 B**：进程调用 `cargo run --bin aura`
- ✅ 完全隔离，与发布版本一致
- ✅ 可以评测任意 agent（不只是 Aura）
- ❌ 进程启动开销，JSON 输出解析

**决策**：选项 B（`cargo run --bin aura -- --json ...`），评测的是最终用户体验。

### 决策 4：评测粒度

**选项 A**：pass/fail 二值
- ✅ 简单
- ❌ 信息损失

**选项 B**：pass/fail + 部分分（verify 子步骤）
- ✅ 更细粒度
- ❌ 任务定义复杂

**决策**：Phase B1 二值 + 耗时 + turns；Phase B2 加部分分。

---

## 10. CLI 设计

```
aura bench --help

Aura coding agent benchmark framework

Usage: aura bench <COMMAND>

Commands:
  run     Run benchmark tasks
  report  Generate report from results
  init    Initialize a new task scaffold
  submit  Submit results to leaderboard (future)
  list    List available tasks

Examples:
  aura bench run                    # Run all tasks
  aura bench run --tasks 'tasks/easy-*'  # Run subset
  aura bench run --agent 'claude-code'  # Run external agent
  aura bench report results/latest  # Generate report
```

---

## 11. 风险与缓解

| 风险 | 缓解 |
|------|------|
| 任务过拟合（agent 记住答案） | 每个任务用唯一临时目录，setup 后不保留 |
| 评测泄露（verify 命令本身有答案） | verify.command 参考实现需人工审计 |
| 任务膨胀（写任务太慢） | 提供 `aura bench init` + 模板快速生成 |
| 评测环境不一致（不同机器 / OS） | Docker sandbox 强制一致环境（Phase B2） |
| Agent 自评（agent 伪造结果） | 结果文件由 harness 写入，不经 agent 手 |
