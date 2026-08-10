# Loop 预算 — Aura 项目

> 主要循环：**每日 Triage**（由 loop-init 脚手架生成）

## 每日限额

| 循环 | 最大运行次数/天 | 最大 Token/天 | 最大子代理启动数/次 |
|------|--------------|--------------|-------------------|
| 每日 Triage | 2 | 100k | 0 (L1) / 2 (L2) |

## 超出预算时

1. 暂停所有调度器（`scheduler_delete` 或禁用自动化）
2. 追加事件到 `loop-run-log.md`
3. 通知人工（Slack / issue / STATE.md High Priority）

## Kill Switch

- 命令或 issue label：`loop-pause-all`
- 仅在人工清除 STATE.md 中的标志后才能恢复

## 估算支出

```bash
npx @cobusgreyling/loop-cost --pattern daily-triage
```

## 阈值调整

- 2026-08-07：`loop-constraints.md` 预算阈值从 80% 提高到 **95%**。
  实际使用 ~39%（5h 内）远低于原限额；新阈值留 5% buffer 给后续仅报告模式。
