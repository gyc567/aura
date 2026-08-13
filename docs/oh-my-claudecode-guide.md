# oh-my-claudecode 完全指南

> 原文：[oh-my-claudecode](https://github.com/Yeachan-Heo/oh-my-claudecode) · 38.5k Stars · 多智能体编排框架

---

## 目录

1. [这是什么](#这是什么)
2. [安装](#安装)
3. [首次配置](#首次配置)
4. [核心概念](#核心概念)
5. [快速上手命令](#快速上手命令)
6. [团队模式详解](#团队模式详解)
7. [HUD 状态栏](#hud-状态栏)
8. [通知集成](#通知集成)
9. [高级用法](#高级用法)
10. [故障排除](#故障排除)

---

## 这是什么

**oh-my-claudecode (OMC)** 是 Claude Code 的多智能体编排框架。它让 Claude Code 可以协调多个专业智能体（agent）同时工作，像一个团队一样分工合作。

### 能做什么

| 场景 | 传统方式 | OMC 方式 |
|------|----------|----------|
| 修复 10 个 Bug | 逐个修复，耗时 2 小时 | `/team 3:executor "fix bugs"` 10 分钟搞定 |
| 重构大型模块 | 担心改坏，不敢下手 | `/ralph "refactor module"` 持续验证直到完成 |
| 快速开发功能 | 手动规划、编码、测试 | `/autopilot "build REST API"` 全自动搞定 |
| 代码审查 | 单一视角，容易遗漏 | `/ccg review this PR` 三模型交叉验证 |

---

## 安装

### 方式一：npm（推荐）

```bash
npm i -g oh-my-claude-sisyphus@latest
omc setup
```

### 方式二：Claude Code 插件市场

```bash
/plugin marketplace add https://github.com/Yeachan-Heo/oh-my-claudecode
/plugin install oh-my-claudecode
/setup
```

> **提示**：安装完成后会看到 `omc setup` 向导，会引导你配置全局还是本地模式。

---

## 首次配置

运行 `omc setup` 后，向导会问你几个问题：

### 1. 配置范围

- **Global（全局）**：所有项目都使用 OMC
- **Local（本地）**：仅当前项目使用 OMC

### 2. 默认并行模式

- **ultrawork**（推荐）：最大并行度，使用所有智能体层级
- **ralph**：持久验证循环，适合需要严格验证的任务

### 3. Agent Teams（可选）

是否启用实验性的多智能体协作功能。启用后可使用 `/team` 命令。

### 4. HUD 状态栏（可选）

实时显示 OMC 状态、上下文用量、智能体数量等。

---

## 核心概念

### 智能体类型

OMC 内置 **19 个专业智能体**：

| 智能体 | 用途 |
|--------|------|
| `architect` | 架构设计 |
| `planner` | 任务规划 |
| `executor` | 代码实现 |
| `code-reviewer` | 代码审查 |
| `debugger` | 调试排错 |
| `test-engineer` | 测试编写 |
| `security-reviewer` | 安全审计 |
| `explore` | 代码库搜索 |
| `writer` | 文档编写 |
| `designer` | UI/UX 设计 |
| `critic` | 批判性分析 |
| `verifier` | 结果验证 |
| `qa-tester` | QA 测试 |
| `tracer` | 因果追踪 |
| `scientist` | 数据研究 |
| `document-specialist` | 外部文档 |
| `code-simplifier` | 代码简化 |
| `git-master` | Git 操作 |
| `analyst` | 需求分析 |

### 模型路由

OMC 自动根据任务复杂度选择模型：

| 模型 | 用途 |
|------|------|
| `haiku` | 快速查找、简单任务 |
| `sonnet` | 标准任务 |
| `opus` | 架构设计、深度分析 |

### 编排模式

```
┌─────────────────────────────────────────────┐
│                  Team 模式                    │
│  (推荐，完整流水线)                          │
├─────────────────────────────────────────────┤
│ plan → prd → exec → verify → fix            │
└─────────────────────────────────────────────┘

┌─────────────────────────────────────────────┐
│              Autopilot 模式                  │
│  (单智能体自主执行)                          │
└─────────────────────────────────────────────┘

┌─────────────────────────────────────────────┐
│               Ralph 模式                     │
│  (持续验证修复循环，直到完成)                │
└─────────────────────────────────────────────┘

┌─────────────────────────────────────────────┐
│              Ultrawork 模式                  │
│  (最大并行度)                               │
└─────────────────────────────────────────────┘
```

---

## 快速上手命令

### 基础命令

| 命令 | 说明 |
|------|------|
| `/autopilot "任务描述"` | 全自动执行任务 |
| `/ralph "任务描述"` | 持续验证直到完成 |
| `/ultrawork "任务描述"` | 最大并行度执行 |
| `/team N:agent "任务"` | N 个智能体协作 |
| `/ralplan "任务描述"` | 迭代式规划 |

### 自动触发关键词

不需要记住所有命令，说出关键词就会自动匹配：

| 你说的话 | 触发的功能 |
|----------|-----------|
| `"autopilot..."` | 自动驾驶模式 |
| `"ralph..."` | 持久验证循环 |
| `"ulw..."` | 最大并行度 |
| `"plan..."` | 规划模式 |
| `"tdd..."` | TDD 模式 |
| `"deep interview..."` | 深度访谈需求澄清 |
| `"deepsearch..."` | 代码库搜索 |
| `"ultrathink..."` | 深度推理 |
| `"ccg..."` | Codex+Gemini 三模型审查 |
| `"cancelomc"` | 取消当前执行 |

### 示例

```bash
# 自动驾驶：构建一个 REST API
/autopilot "build a REST API for managing tasks"

# 团队协作：3 个执行者修复所有错误
/team 3:executor "fix all TypeScript errors"

# 持久模式：重构认证模块
/ralph "refactor the authentication module"

# 最大并行：快速修复所有问题
/ultrawork "fix all linting errors"

# 三模型审查
/ccg review this PR
```

---

## 团队模式详解

### 工作流程

```
┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐    ┌──────────┐
│  Plan   │ -> │   PRD    │ -> │   Exec   │ -> │  Verify  │ -> │   Fix   │
└──────────┘    └──────────┘    └──────────┘    └──────────┘    └──────────┘
   规划          需求文档        执行           验证            修复
```

### Team 命令语法

```bash
/team N:agent-type "task description"
```

| 参数 | 说明 |
|------|------|
| `N` | 智能体数量（1-5） |
| `agent-type` | 智能体类型（executor, debugger, coder-reviewer 等） |

### 示例

```bash
# 3 个执行者并行修复错误
/team 3:executor "fix all TypeScript errors in src/"

# 5 个调试者处理构建错误
/team 5:debugger "fix build errors in src/"

# 架构师 + 执行者协作
/team 2:architect "design the new payment module"
```

### CLI 团队模式

使用 `omc team` 在 tmux 中启动外部 AI 提供者的 worker：

```bash
# 使用 Codex workers
omc team 3:codex "implement the new feature"

# 使用 Gemini workers
omc team 3:gemini "review this codebase"

# 混合模式
omc team 2:claude 1:codex "build the API"
```

---

## HUD 状态栏

OMC 提供实时状态栏，显示当前执行状态。

### 状态元素

```
[OMC] repo:myproject branch:main | ralph:3/10 | ctx:67% | agents:2 | bg:3/5 | todos:2/5
```

| 元素 | 说明 |
|------|------|
| `repo:name` | Git 仓库名 |
| `branch:name` | 当前分支 |
| `ralph:3/10` | Ralph 循环进度 |
| `ctx:67%` | 上下文窗口用量 |
| `agents:2` | 运行中的智能体数量 |
| `bg:3/5` | 后台任务槽位 |
| `todos:2/5` | 任务完成进度 |

### 显示预设

| 预设 | 说明 |
|------|------|
| `minimal` | 仅显示 essentials |
| `focused`（默认） | 显示所有相关元素 |
| `full` | 显示完整多行详情 |

### 命令

```bash
/oh-my-claudecode:hud           # 查看状态
/oh-my-claudecode:hud setup     # 安装/修复
/oh-my-claudecode:hud minimal   # 切换到 minimal
/oh-my-claudecode:hud focused   # 切换到 focused
/oh-my-claudecode:hud full      # 切换到 full
```

---

## 通知集成

### 支持的平台

- **Telegram**
- **Discord**
- **Slack**

### 配置命令

```bash
# Telegram
omc config-stop-callback telegram --enable --token YOUR_BOT_TOKEN

# Discord
omc config-stop-callback discord --enable --webhook YOUR_WEBHOOK_URL

# Slack
omc config-stop-callback slack --enable --webhook YOUR_WEBHOOK_URL
```

### 在任务中使用

在任务描述中加上标签：

```
"fix the auth bug [telegram]" 
"build the API [discord]"
"refactor module [slack]"
```

---

## 高级用法

### 自定义技能

在项目 `.omc/skills/` 目录下创建自定义技能：

```bash
mkdir -p .omc/skills
# 创建 my-skill/SKILL.md
```

### 多仓库工作区

在多个相关仓库的父目录创建 `.omc-workspace` 标记文件：

```bash
# 在父目录
echo "workspace-name:myproject" > .omc-workspace
```

然后 OMC 会识别这个工作区，在所有子仓库间协调工作。

### Skillify

将当前会话中的重复工作流转换为可复用技能：

```bash
# 在会话中说
"skillify: 这是一个数据库迁移检查流程"
```

### 外部 AI 提供者集成

| 提供者 | 命令 | 用途 |
|--------|------|------|
| Codex | `/ask codex "..."` | 架构验证 |
| Gemini | `/ask gemini "..."` | UI 一致性 |
| Antigravity | `/ask antigravity "..."` | 设计评审 |
| Grok | `/ask grok "..."` | 代码审查交叉验证 |

### `omc ask` 命令

```bash
omc ask claude "解释这段代码的架构"
omc ask codex "review this PR"
omc ask gemini "generate UI mockup"
```

---

## 故障排除

### 常见问题

#### Q: Ralph 模式不工作？

确保系统安装了 Ruby：

```bash
# macOS
brew install ruby

# Ubuntu/Debian
sudo apt update && sudo apt install ruby-full
```

#### Q: 团队模式报错？

确保在 `~/.claude/settings.json` 中启用了实验性功能：

```json
{
  "env": {
    "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS": "1"
  }
}
```

#### Q: HUD 不显示？

```bash
# 重新安装 HUD
/oh-my-claudecode:hud setup

# 重启 Claude Code
```

### 诊断工具

```bash
# 运行诊断
/oh-my-claudecode:omc-doctor
```

### 更新 OMC

```bash
# npm 更新
npm i -g oh-my-claude-sisyphus@latest

# 更新配置（不重新运行向导）
omc setup
```

---

## 参考链接

- [GitHub 仓库](https://github.com/Yeachan-Heo/oh-my-claudecode)
- [官方文档](https://oh-my-claudecode.dev)
- [npm 包](https://www.npmjs.com/package/oh-my-claude-sisyphus)

---

*最后更新：2026-08-13 · 适用版本：4.15.10*
