# Aura 插件系统设计规格（v2 候选）

> 🌐 **Language / 语言**: [English](plugin-spec-v2.en.md) · [中文](plugin-spec-v2.md)

- **版本**：v0.1 候选
- **日期**：2026-08-07
- **状态**：v2 候选规格，**未进入实现**
- **来源**：本文档从 [`docs/coding-agent-design.md`](coding-agent-design.md) §11 Phase 7 拆出，原位置已替换为指向本文档的链接
- **参考**：[`agent-plugins/agent-plugins-spec` v1.0.0](https://github.com/agentplugins/agent-plugins-spec)

---

## 1. 目标与范围

v2 在 v1 基础上引入**目录式插件**与 **MCP 服务器**集成。复用 v1 已落地的能力门禁与命令中介作为安全基础，避免引入新的信任模型。

**v2 不做**（仍延后）：插件签名、来源验证、企业管控、密钥管理服务、依赖解析、审计日志标准化。这些在 v3+ 视需求再立规格。

## 2. 目录结构

```
my-plugin/
├── plugin.json          # 插件清单（符合 agent-plugins.org schema v1.0.0）
└── skills/
    └── my-skill/
        └── SKILL.md     # 技能定义（与 Aura 项目内 SKILL.md 同构）
```

## 3. 插件清单（plugin.json）

```json
{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
  "name": "my-plugin",
  "version": "1.0.0",
  "description": "My aura plugin",
  "author": { "name": "...", "email": "...", "url": "..." },
  "homepage": "...",
  "repository": "...",
  "license": "MIT",
  "keywords": ["coding", "rust"],
  "extensions": {}
}
```

**name 校验正则**：`^(?!.*(?:--|\.\.))[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?$`

## 4. MCP 服务器配置

支持 `mcp.schema.json` 声明的三种传输：

| 类型 | 用途 | 安全约束 |
|------|------|----------|
| `stdio` | 本地进程 | `cwd` 限制在插件目录内；禁止 `PLUGIN_ROOT`/`PLUGIN_DATA` 环境变量 |
| `streamable-http` | HTTP MCP 端点 | 自定义 header；URL 由用户配置时指定 |
| `sse` | SSE 推送端点 | 同上 |

```json
{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
  "mcpServers": {
    "filesystem": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/workspace"],
      "env": {},
      "cwd": "./"
    },
    "http-api": {
      "type": "streamable-http",
      "url": "http://localhost:8080/mcp",
      "headers": { "Authorization": "Bearer ${MCP_API_KEY}" }
    }
  }
}
```

## 5. 技能加载

1. 扫描插件目录下 `skills/*/SKILL.md`
2. 解析 frontmatter：`name` + `description`
3. 注册到 Agent 的 `ToolRegistry`（`skills/` 下每个技能 = 一个 Tool）
4. 模型的工具列表动态扩展

## 6. 安全模型（基于 v1 能力门禁）

| 组件 | 复用 v1 | v2 扩展点 |
|------|---------|-----------|
| Capability gate | ✅ `Policy::evaluate()` | 插件声明所需 capabilities |
| Command mediation | ✅ `CommandMediator` | 新增 `plugin.install` / `plugin.uninstall` |
| 环境变量隔离 | 新增 | 禁止 `PLUGIN_ROOT`/`PLUGIN_DATA` 泄露 |
| cwd 限制 | 新增 | `cwd` 必须为 `./` 或 `${PLUGIN_ROOT}/...` |
| MCP secret 管理 | 新增 | `headers` 中的 `${SECRET}` 由 Agent 运行时注入，不明文存储 |

## 7. 生命周期

| 操作 | 命令 | 状态转移 |
|------|------|----------|
| 安装 | `aura plugin install ./my-plugin` | `Ready → PluginInstalled` |
| 列举 | `aura plugin list` | 只读 |
| 启用/禁用 | `aura plugin enable/disable <name>` | 内存状态变更 |
| 卸载 | `aura plugin uninstall <name>` | `PluginInstalled → Ready` |
| 更新 | `aura plugin update <name>` | 版本校验 + 增量覆盖 |

## 8. 模块布局（v2 新增）

```text
src/
  plugin/
    manifest.rs     # plugin.json 解析 + 校验（符合 agent-plugins.org schema）
    resolver.rs     # 插件目录扫描 + skills/ 发现
    lifecycle.rs    # 安装/卸载/启用/禁用状态机
    mcp.rs          # MCP 服务器配置解析（mcp.schema.json）
    secret.rs       # 密钥注入（${SECRET} 模板替换）
  tools/
    plugin_install.rs
    plugin_list.rs
    plugin_uninstall.rs
```

## 9. 关键验收

- 加载一个符合规范的插件后，其 skills 出现在模型可用工具列表中
- MCP stdio 服务器能在插件目录内 spawn，cwd 限制生效
- `PLUGIN_ROOT`/`PLUGIN_DATA` 环境变量无法被插件的 `env` 覆盖
- 密钥注入：`${MY_KEY}` 在运行时替换，manifest 中不存储明文密钥

## 10. 与主设计文档的关系

- 主文档：[`coding-agent-design.md`](coding-agent-design.md) §11 Phase 7 已替换为本文件链接
- v1 安全模型来源：主文档 §5.3、§6
- v1 工具注册机制：主文档 §5.3.1