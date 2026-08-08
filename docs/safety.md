# Safety — Loop Engineering 安全策略

> 本文件定义了 Loop 在本项目中的安全边界和约束。

## 危险路径（Denylist）

以下路径**绝对禁止** Loop 编辑或修改：

```
.env
.env.*
.env.local
.env.*.local
auth/
payments/
secrets/
credentials/
**/secrets/**
**/credentials/**
src/auth/**
src/payments/**
```

## 自动合并策略

- **禁止**：Loop 自动合并到 `main` 分支
- **允许**：经过 Human Gate 审核后的 PR

## MCP Scopes

如果使用 MCP 服务器，Scope 限制：

- 只读访问：docs/, tests/
- 读写访问：src/ (需 Human Gate)
- 禁止访问：.env, auth/, payments/

## 测试约束

- **禁止**：禁用测试或跳过测试
- **必须**：所有 PR 必须通过 `cargo test`
- **必须**：所有 PR 必须通过 `cargo clippy`

## 修复尝试限制

- 同一问题最多尝试 3 次
- 3 次失败后自动 Escalate to Human
- 不进行跨文件的 drive-by refactor

## 紧急停止

```bash
# 立即停止所有 Loop 调度
touch loop-pause-all

# 恢复
rm loop-pause-all
```

## 报告安全事件

如果 Loop 尝试访问危险路径或执行禁止操作：

1. 立即终止 Loop
2. 检查 `loop-run-log.md` 中的 token 使用
3. 人工审核受影响文件
4. 在 [issues](https://github.com/your-repo/issues) 报告

## 相关文件

- `loop-constraints.md` — 运行时约束配置
- `loop-budget.md` — Token 预算配置
- `gate.yaml` — 自动合并白名单（待配置）
