# Loop Engineering 教程

> 本项目采用 [Loop Engineering](https://github.com/cobusgreyling/loop-engineering) 作为 AI 编码智能体的编排框架。

## 什么是 Loop Engineering？

**核心理念**："Stop prompting. Design the loop. Get a score."

Loop Engineering 是一种系统化设计 AI 智能体循环的方法论。传统做法是每次手动给 Agent 发 prompt，而 Loop Engineering 则是**预先设计好循环结构**，让 Agent 按固定节奏自主运行、报告、修复。

```
┌─────────────────────────────────────────────────────┐
│  Schedule / Automation                               │
│  (cron / CI webhook / 定时任务)                      │
└──────────────────┬──────────────────────────────────┘
                   ▼
┌─────────────────────────────────────────────────────┐
│  Triage Skill                                        │
│  收集信号：CI 失败 / Issues / Commits / 对话        │
└──────────────────┬──────────────────────────────────┘
                   ▼
┌─────────────────────────────────────────────────────┐
│  STATE.md                                           │
│  更新 High Priority / Watch / Noise 列表            │
└──────────────────┬──────────────────────────────────┘
                   ▼
         ┌─────────┴─────────┐
         ▼                   ▼
┌─────────────────┐  ┌─────────────────┐
│ Implementer      │  │ Verifier        │
│ (最小化修复)     │  │ (独立验证)       │
└─────────────────┘  └─────────────────┘
         │                   │
         └─────────┬─────────┘
                   ▼
         ┌─────────────────┐
         │ Human Gate      │
         │ (人工审核)       │
         └─────────────────┘
```

## 本项目的 Loop 配置

### 当前状态

```
Pattern:  daily-triage
Ready:    100/100 (L3)
State:    STATE.md · 2026-08-07 (in progress)
Budget:   loop-budget.md · gate=no · constraints=yes
```

### 相关文件

| 文件 | 用途 |
|------|------|
| `LOOP.md` | 循环的配置文件，定义模式和人机交互规则 |
| `STATE.md` | 状态文件，追踪当前优先级、监控列表、最近噪音 |
| `loop-budget.md` | Token 预算配置，每日上限和 kill switch |
| `loop-run-log.md` | 运行日志，每次循环的记录 |
| `loop-constraints.md` | 安全约束，定义禁止编辑的路径 |
| `.grok/skills/` | Grok 智能体的技能目录 |

## 快速开始

### 1. 检查 Loop 健康状态

```bash
# 推荐：统一入口（包含 audit + sync）
npx @cobusgreyling/loop doctor .

# 或单独运行
npx @cobusgreyling/loop audit . --suggest
npx @cobusgreyling/loop sync .
```

**Doctor 输出含义**：
- `Exit 0` = 健康
- `Exit 1` = 有警告
- `Exit 2` = 阻塞（需要修复才能运行）

### 2. 手动运行一次 Triage

```bash
# Grok
/loop 1d Run loop-triage. Update STATE.md. No auto-fix in week one.

# Claude Code
/loop 1d Run $loop-triage. Read STATE.md. Merge findings into High Priority and Watch List. Update Last run. Do not edit code.
```

### 3. 查看运行结果

```bash
# 查看当前状态
npx @cobusgreyling/loop status .

# 查看 STATE.md
cat STATE.md

# 查看运行日志
cat loop-run-log.md
```

## 技能（Skills）

### 已安装的技能

| 技能 | 用途 |
|------|------|
| `loop-triage` | 收集信号，生成优先级报告 |
| `loop-budget` | 运行前后检查 token 预算 |
| `loop-constraints` | 加载安全约束，检查 denylist |
| `minimal-fix` | 生成最小化修复方案（L2） |
| `loop-verifier` | 独立验证修复方案（L2） |

### 技能存放位置

```
.grok/skills/          # Grok 技能目录
├── loop-triage/       # 每日 triage
├── loop-budget/       # Token 预算守卫
├── loop-constraints/  # 安全约束加载
├── minimal-fix/       # 最小化修复（L2）
└── loop-verifier/     # 独立验证（L2）
```

### loop-triage 输出格式

```markdown
## High Priority
- [CI-#123] Test suite failing on main (5 failures)

## Watch
- [ISSUE-#45] Performance regression in hot path
- [ISSUE-#46] Memory leak suspected

## Noise
- Dependabot PRs surfaced again (add to ignore list)
- 1 CI flake (known flaky test)

## State Updates
- Last run: 2026-08-07T10:00Z
```

## L1 → L2 → L3 演进路线

### L1: 报告模式

- **目标**：学习循环，积累数据
- **Token 预算**：低（report only）
- **操作**：Triage + STATE.md 更新
- **禁止**：自动修复、自动合并

### L2: 辅助修复

- **条件**：Score ≥ 50 + 人工审核流程就绪
- **新增**：`minimal-fix` + `loop-verifier`
- **操作**：提出修复建议，人工审核后执行
- **隔离**：使用 git worktree 隔离修复尝试

### L3: 无人值守（当前）

- **条件**：Score ≥ 80 + gate.yaml 配置完成
- **新增**：`loop-gate` 自动检查
- **操作**：自动修复 + 自动合并（在 allowlist 内）
- **保障**：circuit breaker 防止无限重试

## 预算管理

### 查看预算使用

```bash
npx @cobusgreyling/loop cost --pattern daily-triage --level L1 --cadence 1d
```

### 预算检查点

`loop-budget` 技能在每次运行前后检查：

1. **80% 预算耗尽** → 切换到 report-only 模式
2. **90% 预算耗尽** → 挂起，等人工授权
3. **100% 预算耗尽** → 立即停止

### Kill Switch

```bash
# 暂停所有调度（紧急停止）
touch loop-pause-all

# 恢复
rm loop-pause-all
```

## 安全约束

`loop-constraints.md` 定义了安全规则：

### 禁止编辑的路径

```
.env, .env.*, auth/, payments/, secrets/, credentials/
```

### 禁止的操作

- 自动合并到 main
- 禁用测试
- 跳过 lint 检查

### 自定义约束

编辑 `loop-constraints.md` 添加项目特定规则。

## L2 进阶：修复工作流

### 使用 git worktree 隔离修复

```bash
# 创建隔离工作区
npx @cobusgreyling/loop-worktree create --run-id pr-217-fix-1 --pattern ci-sweeper

# 在 worktree 中进行修复...

# 验证器拒绝 → 标记
npx @cobusgreyling/loop-worktree mark --run-id pr-217-fix-1 --status rejected

# 清理旧的 rejected/escalated worktrees
npx @cobusgreyling/loop-worktree cleanup --older-than 24h

# 查看活跃的 worktrees
npx @cobusgreyling/loop-worktree list
```

### 沙箱隔离运行

```bash
# 在临时 worktree 中运行 agent 命令
npx @cobusgreyling/loop-sandbox run -- npx my-agent

# 审查生成的 patch
npx @cobusgreyling/loop-sandbox review

# 应用 patch
git apply .loop-sandbox/patches/<patch-id>.patch
```

## MCP 服务（可选）

让 Agent 按需查询 patterns、skills、state：

```bash
LOOP_PROJECT_ROOT=. npx @cobusgreyling/loop-mcp-server
```

## 常见问题

### Q: 如何修改 Triage 的优先级规则？

编辑 `.grok/skills/loop-triage/SKILL.md` 中的 section 权重。

### Q: 如何添加新的检查源？

在 `loop-triage/SKILL.md` 中添加对应的信号收集逻辑。

### Q: 如何关闭特定检查？

在 `loop-constraints.md` 中添加豁免规则。

### Q: Token 预算超支怎么办？

1. 检查 `loop-run-log.md` 定位异常
2. 人工审核 `loop-budget.md` 调整上限
3. 使用 `loop-pause-all` 暂停调度

## 参考资源

- [Loop Engineering 主仓库](https://github.com/cobusgreyling/loop-engineering)
- [Quickstart 文档](https://github.com/cobusgreyling/loop-engineering/blob/main/docs/QUICKSTART.md)
- [Pattern 选择器](https://github.com/cobusgreyling/loop-engineering/blob/main/docs/pattern-picker.md)
- [失败模式分析](https://github.com/cobusgreyling/loop-engineering/blob/main/docs/failure-modes.md)
- [Loop Design Checklist](https://github.com/cobusgreyling/loop-engineering/blob/main/docs/loop-design-checklist.md)
