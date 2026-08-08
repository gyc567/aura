# Loop Budget — YOUR_PROJECT

> Primary loop: **Daily Triage** (scaffolded by loop-init)

## Daily limits

| Loop | Max runs/day | Max tokens/day | Max sub-agent spawns/run |
|------|--------------|----------------|--------------------------|
| Daily Triage | 2 | 100k | 0 (L1) / 2 (L2) |

## On budget exceed

1. Pause schedulers (`scheduler_delete` or disable automations)
2. Append event to `loop-run-log.md`
3. Notify human (Slack / issue / STATE.md High Priority)

## Kill switch

- Command or issue label: `loop-pause-all`
- Resume only after human clears the flag in STATE.md

## Estimate spend

```bash
npx @cobusgreyling/loop-cost --pattern daily-triage
```

## Threshold adjustments

- 2026-08-07: `loop-constraints.md` budget threshold raised from 80% → **95%** of daily cap.
  实际使用 ~39%（5h 内）显著低于原阈值；新阈值留 5% buffer 给后续 report-only 模式。
