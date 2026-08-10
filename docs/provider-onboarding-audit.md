# Provider Onboarding 审计报告

> **审计日期**: 2026-08-10
> **审计范围**: `docs/provider-onboarding.md`（设计文档，本轮修改的主体）+ `src/setup/` + `src/config.rs` + `src/cli.rs` + `src/main.rs`（该文档所描述的实现）
> **审计方法**: 逐条比对设计文档的每一章节/表格/状态机/失败模式，在源码中验证其实现状态；运行 `cargo test --lib -- setup` 获取测试基线
> **审计人**: loop L1 session

---

## 0. 执行摘要

**核心结论：设计文档与实现之间存在严重偏差。**

本轮修改的主体是 `docs/provider-onboarding.md`（设计文档，435 行 → 681 行），**没有任何 Rust 源码被修改**。但这份文档所描述的系统，在 `src/setup/` 中**已经存在一个 slice 3 级别的实现**（git log 显示 slice 1/1.5/2/3 已落地）。

问题在于：

1. **文档版本落后于实现**：文档在"Audit delta"之前仍标"Draft / not yet implemented"，但代码已经有 `needs_onboarding()`、`run_wizard()`、`keychain.rs`、`providers.rs`、TUI 状态机等完整模块。
2. **本轮文档更新（A1-A15、Q11/Q12 解决）与代码脱节**：文档改为了 MiniMax-M2.5、ConfigConflict 状态、min_probe_tokens、commit 回滚、429 处理等新设计，但代码**完全没有跟上**——有些地方甚至与文档直接矛盾。
3. **实现自身有 2 个测试正在失败**：`config_write::tests::unix_permissions_are_600` 和 `write_overwrites_existing`，说明 slice 3 的"完成"声明不成立。

**一句话：文档在描述 slice 4/5/6 的设计，代码停在 slice 3 且 slice 3 还有坏测试。**

---

## 1. 审计方法

| 步骤 | 内容 |
|------|------|
| 1 | 通读 `docs/provider-onboarding.md` 全文（681 行），提取所有可验证的设计决策 |
| 2 | 在 `src/setup/`、`src/config.rs`、`src/cli.rs`、`src/main.rs` 中 grep 对应结构 |
| 3 | 运行 `cargo test --lib -- setup` 获取测试基线（52 passed / 2 failed） |
| 4 | 运行 `cargo build --bin aura` 确认编译干净 |
| 5 | 运行 `aura setup --help` 确认 CLI 子命令可达 |
| 6 | 对每个设计决策标注：✅ 已实现 / ⚠️ 部分实现 / ❌ 未实现 / 🔴 文档与代码矛盾 |

---

## 2. 文档 vs 实现：逐条对照

### 2.1 触发逻辑（§3）— `needs_onboarding()`

| 文档设计 | 代码实际 | 状态 |
|----------|----------|------|
| 8 行真值表（`--fake-model`、`aura setup`、provider env var、bench 等） | `needs_onboarding()` 硬编码 `return false` | ❌ 未实现 |
| `--fake-model` 跳过向导 | `cli.rs:65` 有 `fake_model` 字段，`main.rs:399` 读取它，但 `needs_onboarding()` 不查它 | ❌ 未实现 |
| `aura setup` 强制进入向导 | `cli.rs:92` 有 `Setup(SetupCli)`，`main.rs:63` 路由到 `run_setup` | ✅ 已实现 |
| provider env var 跳过向导 | 未实现 | ❌ |

**影响**：slice 5 的触发逻辑完全未接入。当前 `aura <task>` 无 key 仍走旧路径（fake mode），不会触发向导——这是**有意的设计**（`mod.rs:7` 注释明确"slice 5 接无参触发"），但文档 §3 的真值表给人"已实现"的错觉。

### 2.2 数据模型（§4）

| 文档设计 | 代码实际 | 状态 |
|----------|----------|------|
| `providers.toml` 含 `min_probe_tokens` 字段（A15/Q12） | `providers.toml` 无此字段，`Provider` struct 无此字段 | ❌ 未实现 |
| `providers.toml` MiniMax = `MiniMax-M2.5`（A2） | `providers.toml` = `MiniMax-Text-01`，`extra_models` 含 `abab6.5s-chat` | 🔴 **矛盾** |
| `chat_url(id)` / `models_url(id)` 派生函数（A1） | `providers.rs` 无此函数，只有 `default_for_id()` 三元组 | ❌ 未实现 |
| keychain legacy fallback `MINIMAX_API_KEY`（A3） | `keychain.rs:load()` 只查 `aura-<id>`，无 fallback | ❌ 未实现 |
| `config.toml` 新增 `provider` 字段（§4.3） | `config.rs:Config` struct 只有 `endpoint`/`model`/`api_key`，**无 `provider` 字段** | 🔴 **矛盾** |
| 6 层 resolve 顺序（CLI > config > provider env > AURA_API_KEY > keychain > catalog） | `config.rs:resolve()` 仍是 3 层（CLI > config > env），无 keychain/catalog 层 | ❌ 未实现 |

**关键矛盾**：文档 §4.3 说 resolve 顺序扩展到 keychain + catalog，但 `config.rs` 的 `Config` 结构体**连 `provider` 字段都没有**。`provider` 是解锁 keychain 查找和 catalog 默认值的钥匙——没有它，keychain 层永远不会被触发。这不是"slice 5 再做"的问题，而是**数据模型根本没到位**。

### 2.3 模块布局（§5）

| 文档设计 | 代码实际 | 状态 |
|----------|----------|------|
| `setup::providers.rs` + `providers.toml` | ✅ 存在 | ✅ |
| `setup::keychain.rs` | ✅ 存在 | ✅ |
| `setup::tui/{mod,app,ui,event,theme}.rs` | ✅ 存在 | ✅ |
| `setup::verify.rs`（probe future） | ❌ 不存在 | ❌ 未实现 |
| `setup::config_write.rs` | ✅ 存在 | ✅ |
| `providers_catalog.toml`（§5 命名） | 实际叫 `providers.toml` | ⚠️ 命名不一致 |

### 2.4 TUI 设计（§6）

| 文档设计 | 代码实际 | 状态 |
|----------|----------|------|
| 状态机 1-9（含 `ConfigConflict`、`Verifying`） | `app.rs` 只有 5 个状态：`PickProvider`/`EnterApiKey`/`Saving`/`Done`/`Error` | ❌ 大部分未实现 |
| `ConfigConflict` 前置状态（A14/Q11） | 不存在 | ❌ |
| `Verifying` 状态 + worker thread + oneshot（A5） | 不存在，`mod.rs:9` 注释"slice 4 接入" | ❌ |
| `event::poll(50ms)` tick（A5） | `mod.rs` 用 `event::read()` 阻塞调用 | 🔴 **矛盾** |
| bracketed paste（A11） | `event.rs` 无 `Paste` 处理，`mod.rs` 的 `Ok(_)` arm 静默吞掉非 Key 事件 | ❌ |
| ratatui 0.30（§6.1） | `Cargo.toml` 实际是 `ratatui = "0.29"` | 🔴 **矛盾** |
| 非 TTY fallback（§6.5） | `mod.rs` 有 `is_terminal()` 检查 + `InvalidRequest` 错误 | ✅ |

**关键矛盾**：文档 §6.2 把事件循环改成 `event::poll(50ms)` + tick + worker thread，但代码是阻塞 `event::read()`。文档描述的是一个异步架构，代码是同步架构。

### 2.5 Key Verification（§7）

| 文档设计 | 代码实际 | 状态 |
|----------|----------|------|
| 1-token chat completion probe | 无 `verify.rs` 模块 | ❌ 未实现 |
| 429 → authenticated（Q12） | 不存在 | ❌ |
| `min_probe_tokens` catalog 字段 | 不存在 | ❌ |
| `/models` fallback | 不存在 | ❌ |

整个 §7 在代码中**完全不存在**。

### 2.6 Failure Modes（§8）

| 文档设计 | 代码实际 | 状态 |
|----------|----------|------|
| commit 回滚：config 失败则删 keychain（A4） | `mod.rs:commit()` 先 keychain 后 config，**config 失败不回滚 keychain** | 🔴 **矛盾** |
| 429 → save + note | 不存在 | ❌ |
| `ConfigConflict` 的 K/R 路径 | 不存在 | ❌ |

**关键矛盾**：文档 §8 的 atomicity 承诺"either both or neither"，但代码的 `commit()` 是"keychain 写了就写了，config 失败不管"。

### 2.7 依赖（§9）

| 文档声明 | 代码实际 | 状态 |
|----------|----------|------|
| `ratatui = "0.30"` | `Cargo.toml:28` = `ratatui = "0.29"` | 🔴 **矛盾** |
| `crossterm` ✅ | ✅ 存在 | ✅ |
| `keyring = "3.x"` | `Cargo.toml:30` = `keyring = "3.6"` | ✅ |

ratatui 版本文档写 0.30，代码实际 0.29（git log 显示从 0.30.2 降版到 0.29.0 是为了 MSRV 1.85 兼容）。

### 2.8 测试策略（§10）

| 文档设计 | 代码实际 | 状态 |
|----------|----------|------|
| `needs_onboarding()` 真值表单测 | 只有 `needs_onboarding_is_false_in_slice1` | ❌ |
| `chat_url()` 无双重 `/v1` 测试 | `providers.rs` 有 `endpoints_are_base_urls_not_paths`（反向测试，无 `chat_url()` 函数） | ⚠️ 部分 |
| `commit()` 回滚测试 | 不存在 | ❌ |
| `ConfigConflict` 状态转移测试 | 不存在 | ❌ |
| 429 → Accepted 测试 | 不存在 | ❌ |
| `min_probe_tokens` 请求体测试 | 不存在 | ❌ |
| 快照测试 | `ui.rs` 有 `done_renders_success` 等，但用的是 `assert!` 而非 insta 快照 | ⚠️ 部分 |

### 2.9 Rollout / Loop Plan（§11）

| 文档声明 | 代码实际 | 状态 |
|----------|----------|------|
| "L1 (this doc, no code) → DONE" | 文档本身确实无代码，但 `src/setup/` 已存在 slice 1-3 的实现 | ⚠️ 误导 |
| Slice 1: `needs_onboarding()` + `aura setup` 桩 | ✅ 已实现（且 `run_wizard` 已远超"桩"） | ✅ 超额 |
| Slice 1.5: ratatui skeleton | ✅ 已实现 | ✅ |
| Slice 2: provider catalog | ✅ 已实现 | ✅ |
| Slice 3: TUI provider picker + keychain write | ✅ 已实现（但有 2 坏测试） | ⚠️ |
| Slice 4: verify probe | ❌ 未实现 | ❌ |
| Slice 5: default `aura` 触发 | ❌ 未实现 | ❌ |
| Slice 6: docs update | ❌ 未实现 | ❌ |

---

## 3. 测试基线

```
$ cargo test --lib -- setup
test result: FAILED. 52 passed; 2 failed; 0 ignored; 122 filtered out

failures:
  setup::config_write::tests::unix_permissions_are_600
  setup::config_write::tests::write_overwrites_existing
```

**失败详情**：

| 测试 | 错误 | 可能原因 |
|------|------|----------|
| `write_overwrites_existing` | `rename .../.config.toml.tmp-42255 -> .../config.toml failed: No such file or directory (os error 2)` | 第二次 write 时 rename 源不见了——可能是 temp 文件被并发测试的 `remove_dir_all` 清掉，或 `write_to` 内 `File::create` + `sync_all` + `rename` 之间有其他测试在操作同一目录 |
| `unix_permissions_are_600` | 同上（rename 失败） | 同上，测试隔离问题 |

这两个失败指向一个**测试隔离 bug**：`config_write.rs` 的 `temp_dir()` 用 `SystemTime::now()` 纳秒做唯一性，但并行测试下可能碰撞，导致一个测试的 `remove_dir_all` 删掉另一个测试的 temp 目录。

---

## 4. 编译与 CLI 状态

| 检查项 | 结果 |
|--------|------|
| `cargo build --bin aura` | ✅ 0.07s 干净 |
| `aura setup --help` | ✅ 输出子命令 `wizard` |
| `aura setup wizard`（非 TTY） | ✅ 报 `InvalidRequest`（non-TTY fallback 生效） |
| `aura --help` | ✅ 含 `--fake-model` |

---

## 5. 风险分级

### 🔴 高（文档与代码直接矛盾，会误导后续开发）

| # | 问题 | 位置 |
|---|------|------|
| R1 | MiniMax 模型：文档 `MiniMax-M2.5` vs 代码 `MiniMax-Text-01` | `providers.toml` / `providers.rs:57` 测试断言 `MiniMax-Text-01` |
| R2 | `config.rs::Config` 无 `provider` 字段，resolve 无 keychain/catalog 层 | `config.rs:23-29` |
| R3 | `commit()` 无回滚（config 失败 keychain 不删） | `mod.rs:commit()` |
| R4 | 事件循环：文档 `event::poll(50ms)` + tick vs 代码 `event::read()` 阻塞 | `mod.rs:event_loop()` |
| R5 | ratatui 版本：文档 0.30 vs 代码 0.29 | `Cargo.toml:28` |

### 🟡 中（文档描述了未实现的功能，但代码注释已标注"slice N 接入"）

| # | 问题 | 位置 |
|---|------|------|
| Y1 | `needs_onboarding()` 硬编码 `false`，真值表单测不存在 | `setup/mod.rs:26` |
| Y2 | `verify.rs` 整个模块不存在 | — |
| Y3 | `ConfigConflict` 状态不存在 | `app.rs:State` |
| Y4 | `min_probe_tokens` 字段不存在 | `providers.rs:Provider` |
| Y5 | keychain legacy fallback 不存在 | `keychain.rs:load()` |
| Y6 | bracketed paste 不存在 | `mod.rs:event_loop()` |

### 🟢 低（命名/版本/注释不一致）

| # | 问题 |
|---|------|
| G1 | 文档 §5 命名 `providers_catalog.toml`，代码实际 `providers.toml` |
| G2 | 文档 §6.1 ratatui 0.30，代码 0.29 |
| G3 | `aura setup` 的 `run_setup` 注释说"slice 1 占位"，实际 slice 3 已超额完成 |

---

## 6. 根因分析

本轮修改暴露的不是"设计文档写得不好"，而是**文档与代码的版本管理脱节**：

1. **实现先于文档落地**：开发者在实现 slice 1-3 时没有同步更新设计文档的状态标记（仍标"Draft"），导致文档整体给人"未实现"的印象。
2. **审计更新只改文档**：本轮审计（A1-A15）和开放问题解决（Q11/Q12）全部落在文档上，没有对应的代码 PR。文档变成了"未来设计稿"而非"实现规格书"。
3. **slice 3 的"完成"不真实**：git log 说 slice 3 完成，但 `config_write` 有 2 个测试失败，且 `commit()` 缺少文档要求的回滚逻辑。

---

## 7. 建议

### 7.1 立即（本次审计关闭前）

| # | 动作 | 理由 |
|---|------|------|
| S1 | **修复 2 个坏测试**：`config_write` 的 `temp_dir()` 改用线程 ID 或 `std::thread::current().id()` 做唯一后缀，避免并行碰撞 | 测试是 CI 门控，失败 = slice 3 实际未完成 |
| S2 | **统一 MiniMax 模型**：要么把代码改成 `MiniMax-M2.5`（并更新 `providers.rs:57` 的测试断言），要么把文档改回 `MiniMax-Text-01` | R1 矛盾会直接导致用户拿到错模型 |
| S3 | **文档顶部状态标记更新**：把"Draft / not yet implemented"改为"Partially implemented (slice 3); slices 4-6 in design" | 避免后续读者误解 |

### 7.2 短期（slice 4 之前）

| # | 动作 |
|---|------|
| S4 | `config.rs::Config` 加 `provider: Option<String>` 字段，`resolve()` 扩展 keychain + catalog 层（§4.3） |
| S5 | `commit()` 加回滚：config 失败时调 `keychain::delete()`（§8 A4） |
| S6 | `keychain::load()` 加 legacy fallback：先查 `aura-<id>`，miss 则查 `<PROVIDER>_API_KEY`（§4.2 A3） |
| S7 | `providers.rs` 加 `chat_url(id)` / `models_url(id)` 派生函数 + 无双重 `/v1` 测试（§4.1 A1） |

### 7.3 中期（slice 4-6）

| # | 动作 |
|---|------|
| S8 | 实现 `verify.rs`：1-token chat probe + `/models` fallback + 429 处理（§7） |
| S9 | `app.rs` 加 `ConfigConflict` 状态 + `Verifying` 状态（§6.2） |
| S10 | `needs_onboarding()` 接入完整真值表（§3） |
| S11 | 事件循环从 `event::read()` 改为 `event::poll(50ms)` + tick（§6.2 A5） |

### 7.4 流程改进

| # | 动作 |
|---|------|
| P1 | **每个 slice 落地时同步更新设计文档的状态标记**，避免文档/代码漂移 |
| P2 | **审计结论（A1-A15）要么伴随代码 PR，要么在文档中标注"设计已决定，未实现"**——不要给人"已实现"的错觉 |
| P3 | **CI 加 `cargo test --lib -- setup` 门控**，2 个坏测试应在 slice 3 关闭前修复 |

---

## 8. 审计结论

`docs/provider-onboarding.md` 是一份**高质量的设计文档**——结构清晰、审计覆盖完整（A1-A15）、开放问题全部解决（Q11/Q12）、测试策略和 rollout 计划都到位。

但它目前**不是实现的准确镜像**。文档描述的是 slice 4-6 的未来状态，代码停在 slice 3 且 slice 3 有缺陷。在把文档状态改为"Implemented"之前，至少需要：

1. 修复 2 个坏测试（S1）
2. 统一 MiniMax 模型（S2）
3. 补齐 `provider` 字段 + keychain/catalog resolve 层（S4）
4. 补齐 `commit()` 回滚（S5）

**审计状态：条件通过（conditional pass）——文档设计合理，实现需补齐。**
