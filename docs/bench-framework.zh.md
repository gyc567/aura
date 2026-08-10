# Aura Bench Framework — 评测框架设计

**版本**: v0.1
**日期**: 2026-08-08
**参考**: [Harbor Framework](https://www.harborframework.com) + [tbench.ai](https://tbench.ai)

---

## 1. 目的

Aura Bench Framework 为 Aura coding agent 提供**标准化、隔离、可复现**的评测能力。

### 核心问题

- "这次改动让 agent 变好了还是变差了？" → 需要 baseline 对比
- "新工具 / 新策略 / 新模型"能否提升成功率？ → 需要量化指标
- 哪些任务类型是 Aura 的弱项？ → 需要细粒度诊断

### Harbor 对应关系

| Harbor 概念 | Aura 对应 |
|---|---|
| Terminal-Bench 任务集 | Aura `bench/tasks/` 任务定义 |
| Oracle solution | Reference implementation（人写正确答案） |
| `harbor run -a claude-code` | `aura bench run --agent aura` |
| Daytona sandbox | `aura bench run --sandbox docker` |
| pass@k / time-to-solve | `aura bench report` 输出指标 |

---

## 2. 架构概览

```
aura bench
├── run        # 执行任务集
├── report     # 生成指标报告
├── submit     # 发布到 leaderboard（未来）
└── init       # 初始化新任务

bench/
├── tasks/              # 任务定义（YAML）
│   ├── hello-world.yaml
│   └── ...
├── results/            # 每次运行的结果
└── REFERENCE.md       # 任务编写指南
```

---

## 3. 任务定义（TaskSpec）

每个任务是 `bench/tasks/` 下的一个 YAML 文件：

```yaml
id: write-tests
name: "Write unit tests for a given module"
difficulty: medium          # easy | medium | hard
category: testing          # testing | refactor | bugfix | feature | docs | infra

setup:
  - action: write
    path: "src/lib.rs"
    content: |
      pub fn add(a: i32, b: i32) -> i32 { a + b }

instruction: |
  Write comprehensive unit tests for `src/lib.rs`.

verify:
  type: command
  command: "cargo test --quiet"
  timeout_seconds: 60
```

### verify 类型

| type | 成功条件 |
|------|----------|
| `command` | 退出码 0 |
| `file_exists` | 文件存在且非空 |
| `git_diff` | diff 匹配 pattern |
| `cargo_test` | `cargo test` 退出码 0 |
| `cargo_fmt` | `cargo fmt --check` 通过 |

---

## 4. 执行流程

```
aura bench run [OPTIONS]
  --tasks <glob>           # 默认 bench/tasks/*.yaml
  --agent <agent-cmd>      # 默认 cargo run --bin aura
  --output <dir>           # 结果目录
  --parallel <n>           # 并行任务数
  --timeout <seconds>       # 单任务超时（默认 300s）
```

步骤：解析任务 → 创建隔离 workspace → 执行 setup → 启动 agent → 运行 verify → 收集结果 → 生成汇总报告。

---

## 5. 报告命令

```
aura bench report <results-dir>
```

输出格式：

```
Aura Bench Report
=================
Pass Rate:  66.7%  (8/12 passed)
Wall Time:  184.3s total, 15.4s avg

By Category
  testing   3/3  ████████████ 100%
  bugfix    3/4  ████████░░░░  75%
  refactor  1/2  █████░░░░░░░  50%
```

---

## 6. 初始任务集（8 个种子任务）

| 任务 | difficulty | category | 验证方式 |
|------|-----------|----------|----------|
| `hello-world` | easy | feature | `cargo test` |
| `add-tests-to-lib` | easy | testing | `cargo test` |
| `fix-compile-error` | easy | bugfix | `cargo build` |
| `format-code` | easy | infra | `cargo fmt --check` |
| `readme-from-spec` | medium | docs | 文件存在 |
| `write-grep-tool` | medium | feature | `cargo test` |
| `refactor-duplication` | medium | refactor | `cargo test` |
| `implement-scratchpad-tests` | medium | testing | `cargo test` |

---

## 7. 测试金字塔

```
aura 测试金字塔
├── 单元测试（cargo test）
│   └── 验证单个模块的正确性
├── 集成测试（tests/*.rs）
│   └── 验证 agent 循环逻辑（FakeModel，无网络）
└── 基准测试（aura bench）NEW
    └── 验证 agent 在真实任务上的端到端表现
```

**bench 是补充，不是替代** — 现有 215 个测试依然是主力质量门禁。

---

## 8. 实现计划

| 阶段 | 内容 |
|------|------|
| **Phase B1（v1.2）** | TaskSpec 解析 + `aura bench run` + 4 个种子任务 + 基础报告 |
| **Phase B2（v1.3）** | 8 个种子任务 + `aura bench report` 美化 + `aura bench init` + diff |
| **Phase B3（v1.4+）** | Reference solution + pass@k 统计 + Leaderboard 提交 |

---

## 9. 关键设计决策

| 决策 | 选项 | 选择 |
|------|------|------|
| Workspace 隔离 | 进程级 vs Docker | **进程级**（Phase B1）+ 路径校验；Phase B2 加 Docker 选项 |
| 任务定义格式 | YAML vs JSON vs Rust | **YAML**（`serde_yaml`） |
| Agent 接口 | 函数调用 vs 进程调用 | **进程调用**（`cargo run --bin aura -- --json`）|
| 评测粒度 | 二值 vs 部分分 | **二值 + 耗时 + turns**（Phase B1） |
