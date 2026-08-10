# 循环配置 — 最小化 Triage

## 活跃循环

| 模式 | 节奏 | 状态 | 命令 |
|------|------|------|------|
| Daily Triage | 1天 | L1 仅报告 | 见 README |

## 人工门控

- L2 检查单完成前不自动修复
- 所有高风险路径：需人工审核

## 预算

- 每次运行最大子代理数：0 (L1) / 2 (L2)
- 每日最大 Token：100k（见 `loop-budget.md`）
- 每次运行追加到 `loop-run-log.md`；开始/结束时使用 `loop-budget` skill
- Kill Switch：`loop-pause-all` — 暂停所有调度器并通知人工
- 估算：`npx @cobusgreyling/loop-cost --pattern daily-triage`

## 相关链接

- 模式：[daily-triage](../../patterns/daily-triage.md)
- 检查单：[loop-design-checklist](../../docs/loop-design-checklist.md)
