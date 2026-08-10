# Provider Onboarding — 设计文档

> 状态：**草案**（loop L1；尚未实现）
> 作者：2026-08-10 loop session
> 审核：2026-08-10 loop audit（本修订版）— 见下"审核增量"
> 目标：aura v0.2 — 首次运行向导 + 多 Provider 钥匙串
> 参考：<https://pi.dev/docs/latest/providers#api-keys>

---

## 审核增量（2026-08-10，L1 review）

针对实际代码库（`src/model_http.rs`、`src/config.rs`、`src/cli.rs`、`src/main.rs`、`src/bench/runner.rs`、`STATE.md`、`loop-run-log.md`）的 loop-engineering 审查。以下发现均已折叠到正文；本块为摘要。

| # | 严重性 | 发现 | 修复（已落入正文） |
|---|--------|------|----------------|
| A1 | **高** | Catalog 端点包含 `/v1`，但 `HttpConfig::url()` 会追加 `/v1/chat/completions` → **双重 `/v1/v1`** URL（`src/model_http.rs:url()`）。`config.example.toml` 和 `tests/config.rs:17` 也存在同样问题。 | Catalog 存储**不含路径的 base URL**；chat URL 和 probe URL 均派生（`{base}/v1/...`）。§1、§4.1、§7。添加 `chat_url()` 单元测试，固定 no-double-`/v1` 不变量。 |
| A2 | **中** | MiniMax 默认模型 `MiniMax-Text-01` / `abab6.5s-chat` 已过时 — 真实 E2E 和 mock 测试使用的是 **`MiniMax-M2.5`**（`src/model_http.rs:575`，`STATE.md`）。 | 默认值改为 `MiniMax-M2.5`；从 extras 中删除 `abab6.5s-chat`。开放问题 #2 已解决。 |
| A3 | **中** | macOS 钥匙串中已存在真实 key，service 为 `MINIMAX_API_KEY`，account 为 `aura`（E2E，2026-08-09）。文档建议 service 为 `aura-minimax` → 向导会**孤立已有 key**。 | 新的写入使用 `aura-<id>`；`keychain::load` 优先尝试 `aura-<id>`，找不到时回退到 legacy service name。§4.2。 |
| A4 | **中** | §2 快乐路径先写 keychain **后写** config，§8 说先写 **config**；且 §8 的"保持文件原样"顺序违反其自身的"两者同时或不同时"声明。 | 单个 `commit()` 合约：`keychain.set` → config 临时文件 + rename → config 失败时**删除 keychain 条目（回滚）**。§2 和 §8 对齐。 |
| A5 | **中** | `Verifying` 状态需要等待 HTTP future，但事件循环指定为同步 `event::read()`；并发模型未指定。"~30 fps" 对阻塞读取循环也是错误的。 | 事件循环 = `event::poll(50ms)` tick；验证运行在 worker 线程；每个 tick 轮询 `oneshot` 通道（驱动 spinner）。Raw mode 下 Ctrl+C 作为**按键事件**到达，而非 SIGINT — 两条路径都映射到 `Msg::Abort`。§6.2/§6.3。 |
| A6 | **中** | **Fake 模式回归**：当前 `aura "task"` 无 key 时以 fake 模式运行并带警告；`aura bench run`（非 TTY）依赖此行为。Onboarding 后，无 key 非 TTY 运行 exit 1 → **bench/CI 在新环境会破坏**。 | `needs_onboarding()` 在设置了 `--fake-model` 时返回 false；`aura setup` 始终进入向导。§3 真值表 + §11 slice-5 人工门控 + 回归测试。 |
| A7 | **中** | §13 说"现有 Config 解析顺序不变"，但 §4.3 添加了 keychain + catalog 默认值 → 矛盾。此外 `AURA_API_KEY` 和 provider 专属 env var 之间的优先级也不明确。 | 精确重述解析顺序：CLI > config > provider env var > `AURA_API_KEY` > keychain；catalog 默认值仅作为 endpoint/model 的最低层。§4.3、§13。 |
| A8 | **低/中** | Verify probe `GET {endpoint}/models` 假设每个 provider 都实现了 `/models`（MiniMax 的 OpenAI 兼容面以 chat 为主；可能 404）。 | Probe = **1-token chat completion**（`POST {base}/v1/chat/completions`，`max_tokens=1`）— 与真实使用相同的代码路径，对所有 OpenAI 兼容 provider 均有效；`/models` 仅作为快速路径回退。§7。 |
| A9 | **低/中** | `.env` 回退与 `loop-constraints.md`（"禁止编辑 .env…"）冲突，且"在运行 shell 中 source"对二进制不可行。 | Linux 回退 = `~/.config/aura/keys/<provider_id>.key`（chmod 600），二进制在解析时读取；无需 shell source。§4.2、§8。 |
| A10 | 低 | 已有配置冲突：用户有 `endpoint`/`model` 但无 `provider`/key → 向导写入 `provider` 但保留了一个外来 endpoint → key 用在错误 endpoint 上。 | 向导只写入 **`provider`**（加 key）；endpoint/model 仅在 config 中缺失时才从 catalog 解析；向导在已有 endpoint ≠ provider 默认值时发出警告。§4.3、§8。 |
| A11 | 低 | 粘贴的 key（60+ 字符）带尾部换行符会触发 Enter 中途输入；隐藏输入无 trim。 | 启用 bracketed paste，strip CR/LF，trim 输入。§6.4。 |
| A12 | 低 | Linux 上 `keyring` 会拉取 `zbus`（不小的依赖树）；文档声称"低风险"但未验证。 | 保留 keyring 跨平台，但让 slice 1.5（含依赖）显式检查 `cargo tree` 增长 + MSRV。§9、§11。 |
| A13 | 低 | 触发表缺少 `--fake-model` 和 `aura setup` 行（已被 A6 覆盖）。测试面缺少真值表本身。 | 将 §3 真值表移植为单元测试（最有价值的可测试产物）。§10。 |
| A14 | 低 | Q11（开放）：已有配置的 keep-or-replace UX 未指定。 | 具体 `ConfigConflict` 前导状态：显示检测到的 endpoint/model；`K` → 保留为自定义（写入 `provider = "custom"`，规范化尾部 `/v1`），`R` → 正常选择器（用 provider 默认值覆盖 endpoint/model）。§2/§6.2/§8。Q11 已解决。 |
| A15 | 低 | Q12（开放）：`max_tokens=1` probe 可能被某些 provider 拒绝。 | Catalog 增加 `min_probe_tokens`（默认 1，在 slice-4 E2E 校准）；probe 阶梯：chat → `/models`；**429 = 已认证 → 保存并附注**。§4.1/§7/§8。Q12 已解决。 |

未更改：ratatui 决定（用户指定）、键盘输入、无 spinner crate、无 `color-eyre`、v0.2 无 `--from-stdin`、3 provider 范围。

---

## 1. 目标

**当用户运行 `aura` 时无参数且无现有 config，二进制引导用户选择模型 provider 并存储 API key — 无需任何 flag、环境变量或手动编辑 `config.toml`。**

向导是一等的 **ratatui** TUI（交替屏幕、raw mode、panic-safe 清理），而非行打印提示 — 颜色、布局和进度状态都是用户可见的，而非仅仅是 stdout 上的文本。框架选择见 §6， rollout 见 §11。

v0.2 所需的三个 provider（用户指定）：

| Provider | 显示名 | Base URL（无路径） | 默认模型 | Env 变量 | Keychain 服务 |
|----------|--------|-------------------|---------|---------|--------------|
| DeepSeek | DeepSeek | `https://api.deepseek.com` | `deepseek-chat` | `DEEPSEEK_API_KEY` | `aura-deepseek` |
| MiniMax  | MiniMax（海螺 AI） | `https://api.minimaxi.com` | `MiniMax-M2.5` | `MINIMAX_API_KEY` | `aura-minimax` |
| Kimi     | Kimi (Moonshot) | `https://api.moonshot.cn` | `moonshot-v1-8k` | `KIMI_API_KEY` | `aura-kimi` |

**端点约定（audit A1）：** catalog 存储**不含任何路径后缀的 base URL**。
`HttpConfig::url()` 已追加 `/v1/chat/completions`（`src/model_http.rs`），因此将
`https://api.deepseek.com/v1` 写入 catalog 会产生 `https://api.deepseek.com/v1/v1/chat/completions`。
所有派生 URL（chat、probe）均构建为 `{base}/v1/...`。同一规则必须应用于
`config.example.toml` 和 `tests/config.rs`（它们当前记录的是带 `/v1` 后缀的形式）。

---

## 2. 用户流程（快乐路径）

```
┌─ Aura 0.2 — 首次设置 ─────────────────────────────────────┐
│                                                             │
│  未找到 API key。请选择一个 provider 继续：                  │
│                                                             │
│    1) DeepSeek         https://api.deepseek.com            │
│    2) MiniMax          https://api.minimaxi.com            │
│    3) Kimi / Moonshot  https://api.moonshot.cn            │
│    4) Other (OpenAI-compatible)                            │
│    5) 我已在 shell 中设置了 AURA_API_KEY — 跳过            │
│                                                             │
│  选择 [1-5]: 1                                             │
│  > _                                                        │
│                                                             │
│  粘贴你的 DeepSeek API key（输入已隐藏）：                   │
│  > sk-********************************                      │
│                                                             │
│  [ 正在验证 key … ]                                        │
│  [ ✓ key 已接受，已保存至 keychain (aura-deepseek) ]       │
│  [ ✓ 已写入 ~/.config/aura/config.toml (provider = deepseek) ] │
│                                                             │
│  下一步：运行 `aura "<your task>"` 开始。  [ 按任意键 ]     │
└─────────────────────────────────────────────────────────────┘
```

注意（audit A4/A10）：

- **写入顺序**：keychain **先**，config **后**；如果 config 写入失败，删除 keychain 条目（回滚）— 无部分状态。§8。
- 向导**只写入 `provider = "deepseek"`** 到 config（key 写入 keychain）。`endpoint`/`model` 在运行时从 catalog 解析，因此 provider 修复（如重命名模型）不会让用户 config 中留下过时 URL。
- 如果用户 config 已有 `endpoint`/`model` 但无 `provider`/key，向导以 `ConfigConflict` 前导状态启动（Q11）：

  ```
  检测到 ~/.config/aura/config.toml 当前指向：
    endpoint = https://api.openai.com           （尾部 /v1 已规范化）
    model    = gpt-4o
  [K]eep these as-is (custom provider)   [R]eplace with provider defaults   [Esc] abort
  ```

  `K` → keep-as-custom：编辑/确认 endpoint+model，然后 key 条目，保存写入 `provider = "custom"`（+ endpoint/model），key 落在 `aura-custom`。`R` → 正常选择器；保存时 endpoint/model 被**覆盖**为 provider 默认值（永远不会将外来 endpoint 与新 provider key 静默混合 — A10）。

---

## 3. 触发逻辑

新模块 `setup::needs_onboarding()` 决定是否进入向导。**真值表**（此表原样移植到单元测试 — 见 §10）：

| CLI 参数 | env `AURA_API_KEY` | config.toml `api_key` | keychain 条目 | 动作 |
|----------|-------------------|----------------------|----------------|------|
| `--fake-model`（任何任务） | 任意 | 任意 | 任意 | **跳过**（显式 fake；A6 — 保留 bench/CI 行为） |
| `aura bench …` / `aura --version` / `aura --help` / `aura --json …` | 任意 | 任意 | 任意 | **跳过**（子命令/非任务调用） |
| `aura setup`（子命令） | 任意 | 任意 | 任意 | **始终进入向导**（显式意图：切换 provider / 轮换 key） |
| 任何任务附带 `--api-key` | 任意 | 任意 | 任意 | **跳过**（flag 优先；key 不触碰磁盘） |
| 任何任务，env `AURA_API_KEY` 已设置 | 已设置 | 任意 | 任意 | **跳过**（env 优先于向导） |
| 任何任务，provider env var 已设置（`DEEPSEEK_API_KEY` 等） | 未设置 | 任意 | 任意 | **跳过**（provider 专用 var 优先于向导） |
| **以上皆非，无 key** | 未设置 | 不存在 | 不存在 | **进入向导** |
| 给定任务但缺 key | 未设置 | 不存在 | 不存在 | **进入向导**（不静默失败） |

**行为变化 vs 今天（audit A6）：** 当前 `aura "task"` 无 key 时以 fake 模式运行并带警告，`aura bench run`（非 TTY）依赖此行为。本变更后，无 key **交互式** 运行进入向导，无 key **非 TTY** 运行 exit 1（见 §6.5）。`--fake-model` 仍作为脚本化/bench 使用的显式 opt-in。在 slice-5 人工门控（§11）处特别说明。

---

## 4. 数据模型

### 4.1 `providers.toml`（随二进制发布，只读）

编译时通过 `include_str!`  baked into 二进制的静态 catalog。每 provider 一节：

```toml
# aura 内置 provider catalog。在 src/ 中编辑此文件以添加 provider。
# 每个 provider 是一个 OpenAI-compatible chat completion 端点。
#
# 注意（audit A1）：`base_url` 不含路径后缀。所有派生 URL：
#   chat_url   = base_url + "/v1/chat/completions" （匹配 HttpConfig::url()）
#   models_url = base_url + "/v1/models"            （probe 快速路径）
# 不要在这里写 "https://api.X.com/v1" — HttpConfig 自己追加 /v1。
#
# Probe 调优（audit A15/Q12）：`min_probe_tokens` 是 verify probe 使用的 max_tokens（§7）。
# 默认 1；仅当其 API 拒绝 max_tokens=1 时才调整（slice-4 E2E 期间观察 400，
# 然后在此处 bump 并附注释 — 代码中不特殊处理任何 provider）。

[[providers]]
id = "deepseek"
display_name = "DeepSeek"
base_url = "https://api.deepseek.com"
default_model = "deepseek-chat"
env_var = "DEEPSEEK_API_KEY"
keychain_service = "aura-deepseek"
extra_models = ["deepseek-reasoner"]
min_probe_tokens = 1

[[providers]]
id = "minimax"
display_name = "MiniMax"
base_url = "https://api.minimaxi.com"
default_model = "MiniMax-M2.5"          # audit A2：2026-08-09 真实 E2E 验证
env_var = "MINIMAX_API_KEY"
keychain_service = "aura-minimax"
extra_models = ["MiniMax-M2.5"]
min_probe_tokens = 1

[[providers]]
id = "kimi"
display_name = "Kimi / Moonshot"
base_url = "https://api.moonshot.cn"
default_model = "moonshot-v1-8k"
env_var = "KIMI_API_KEY"
keychain_service = "aura-kimi"
extra_models = ["moonshot-v1-8k", "moonshot-v1-32k", "moonshot-v1-128k"]
min_probe_tokens = 1

[[providers]]
id = "custom"
display_name = "Other (OpenAI-compatible)"
base_url = ""               # 用户填写（裸 host，无 /v1）
default_model = ""          # 用户填写
env_var = "AURA_API_KEY"
keychain_service = "aura-custom"
extra_models = []
min_probe_tokens = 1
```

### 4.2 Keychain 条目（macOS / Linux Secret Service）

| 服务 | 账户 | 存储 |
|------|------|------|
| `aura-deepseek` | `aura` | DeepSeek API key |
| `aura-minimax`  | `aura` | MiniMax API key |
| `aura-kimi`     | `aura` | Kimi API key |
| `aura-custom`   | `aura` | "Other" API key |
| `aura-gh-publish` | `gyc567` | （临时，由 release pipeline 使用；无关） |

**Legacy 条目（audit A3）：** 本机已存在真实 MiniMax key，service 为 `MINIMAX_API_KEY`，account 为 `aura`（2026-08-09 真实模型 E2E 时存储）。`keychain::load` 必须先尝试 `aura-<id>`，再回退到 legacy service name（`MINIMAX_API_KEY`），使已有 key 往返而不需要重新输入。下一次 `aura setup` 重新保存会迁移到 `aura-minimax`。

**无 Secret Service 的 Linux（audit A9）：** 回退到将 key 写入 `~/.config/aura/keys/<provider_id>.key`（chmod 600，一行，无引号/转义语义）。二进制在解析时自行读取此文件 — 无需 shell source，不会写入 `.env` 文件（与 `loop-constraints.md` 一致）。macOS / 带 keyring 的 Linux 走 keychain 路径。Windows 使用 `wincred`。

### 4.3 `~/.config/aura/config.toml`（扩展 schema）

当前 schema（3 个字段）**保留；新字段是可选且加性的**：

```toml
# 现有字段 — 不变
endpoint = "https://api.deepseek.com"   # base URL，无 /v1 后缀（audit A1）
model    = "deepseek-chat"
# api_key = "..."   # 不推荐；优先使用 keychain

# 新增（可选）
provider = "deepseek"            # 映射到 providers.toml id
# alias = "my-deepseek"           # 未来：多 profile
```

**解析顺序（audit A7 — 精确重述；向后兼容）：**

1. CLI flag（`--api-key`、`--endpoint`、`--model`）— **最高**
2. `config.toml`（`endpoint`、`model`、`provider`）
3. Catalog 中的 provider 专用 env var（如 `DEEPSEEK_API_KEY`），**仅在 `provider` 已知时**生效
4. `AURA_API_KEY` env var
5. keychain 查找（`keyring::Entry::get_password`）/ `keys/` 文件回退 — 由 `provider` 驱动
6. **最低层，仅 endpoint/model：** catalog 默认值（`base_url`、`default_model`）— 仅在 config 缺失且 `provider` 已知时填充

---

## 5. 模块 / 文件布局

```
src/
├── main.rs                  # 新增：检测 → 向导 → 现有流程
│   ├── mod.rs               #   needs_onboarding() + run_wizard()（Elm 风格循环驱动）
│   ├── providers.rs         #   Provider catalog（加载 providers.toml；chat_url/models_url）
│   ├── tui/                 #   新增：ratatui 向导 UI
│   │   ├── mod.rs           #     终端 init/restore、panic hook、交替屏幕生命周期
│   │   ├── app.rs           #     向导状态机（App struct + Message enum）
│   │   ├── ui.rs            #     纯 render fn(frame, &app) → () （可快照测试）
│   │   ├── event.rs         #     Key/mouse/resize/tick → Message
│   │   └── theme.rs         #     调色板 + Style helpers（单一真相来源）
│   ├── prompt.rs            #   薄层：仅 masked-text-input widget（委托给 tui/）
│   ├── keychain.rs          #   save(key) / load(provider) 通过 `keyring` crate（+ legacy 回退）
│   └── verify.rs            #   probe future（worker thread + oneshot；见 §6.2/§7）
├── config.rs                # 扩展：添加 `provider` 字段；resolve() 按 §4.3 扩展
├── providers.toml   # 新增：通过 include_str! 嵌入；见 §4.1
```

CLI：添加一个非默认子命令 `aura setup` 以重新运行向导（例如切换 provider 或轮换 key）。默认行为（`aura` 无参数）按 §3 触发向导。

---

## 6. TUI 设计

### 6.1 TUI 框架

向导使用 **ratatui 0.29**（默认 **`crossterm`** backend — Linux + macOS + Windows 开箱即用，`setup/` 中无平台条件代码）：

```toml
# Cargo.toml — 默认（crossterm）
ratatui = "0.29"

# 可选 backend，选其中一个：
# ratatui = { version = "0.29", default-features = false, features = ["termion"] }
# ratatui = { version = "0.29", default-features = false, features = ["termwiz"]  }
# ratatui = { version = "0.29", default-features = false, features = ["termina"]  }
```

### 6.2 App 架构

向导遵循 ratatui 的 **Elm 架构**：

```text
                ┌───────────────────────┐
                │   Event (key/mouse)   │
                └──────────┬────────────┘
                           ▼
                ┌───────────────────────┐
                │   update(msg, app)    │  → new App
                └──────────┬────────────┘
                           ▼
                ┌───────────────────────┐
                │   ui::render(frame, &app) │  (pure, no IO)
                └───────────────────────┘
                           ▼
                ┌───────────────────────┐
                │   crossterm backend    │  → terminal
                └───────────────────────┘
```

**事件循环与并发（audit A5）：** 循环是 tick 驱动的事件循环，**不是** 30 fps 渲染循环，**也不是** 阻塞 `event::read()`：

```text
loop {
    match event::poll(Duration::from_millis(50))? {   // 50ms tick
        true  → msg = event::read()?                    // key / paste / resize
        false → msg = Msg::Tick                          // spinner 帧推进
    }
    if let Ok(result) = verify_rx.try_recv() { msg = Msg::VerifyDone(result) }  // §7
    app = update(msg, app)?;
    terminal.draw(|f| ui::render(f, &app))?;
}
```

- **`Verifying` 状态**：HTTP probe 运行在 detached worker 线程（std thread + `oneshot` 通道），因为主线程忙于同步事件循环。通道在每个 tick 被轮询。向导内无需 tokio runtime — 这使 `aura setup` 独立于 agent-mode runtime。
- **Ctrl+C**：raw mode 下是按键事件（`KeyCode::Char('c')` + `CONTROL`），**不是** SIGINT。该按键事件和外部 SIGINT（如 `kill`）都映射到 `Msg::Abort`。

### 6.3 终端生命周期

### 6.4 输入细节

- **Provider 选择**：数字键 `1..=5`（或方向键 + Enter）。越界 → 无操作 + 状态行闪烁。
- **API key 输入**：任何可打印字符 → 渲染为 `*`，真实字符存入 app state。Backspace → 删除。Ctrl-U → 清空。Esc → `Aborted`。Enter → `Verifying`。
- **粘贴（audit A11）**：启用 bracketed paste；`Paste` 事件过滤为可打印字符（strip CR/LF），附加到输入缓冲区**不触发** Enter。
- **鼠标**：v0.2 不启用（保持 TUI 在 SSH 上可预测）。

### 6.5 非 TTY 回退（CI、管道 stdin）

在 `init()` 前，向导检查 `stdin().is_tty() && stdout().is_tty()`。任一为 false：
- 默认：`aura` exit 1 并输出一行纯文本：*"aura: no TTY detected; run `aura setup` interactively or set AURA_API_KEY=<key>"*
- **回归保护（audit A6）：** `needs_onboarding()` 对 `--fake-model` 和子命令返回 false，因此 `aura bench run` 和脚本化 fake-mode 运行不会走到此路径。

---

## 7. Key 验证（可选但推荐）

用户粘贴 key 后，保存前做一次廉价 probe（audit A8）：

```http
POST {base}/v1/chat/completions
Authorization: Bearer {key}
{ "model": "{default_model}", "messages": [{"role":"user","content":"ping"}], "max_tokens": 1 }
```

- 200 → 保存
- 401/403 → 重新提示 *"key rejected by server — paste again or Ctrl-C to abort"*
- 429 → **已认证**（限流发生在认证之后）→ 保存，附状态说明
- 400/404/405 on chat → 快速路径回退：`GET {base}/v1/models`；两者都失败 → *"couldn't verify key; save anyway? [y/N]"* — 默认 No
- 网络错误 / 超时 → 询问 *"couldn't reach {base}; save anyway? [y/N]"* — 默认 No

Probe 使用 `max_tokens = provider.min_probe_tokens`（catalog 中，默认 1；§4.1）。如果某 provider 拒绝（HTTP 400），在 slice-4 E2E 期间调整 `min_probe_tokens` — 代码中不特殊处理任何 provider。

---

## 8. 失败模式（显式）

| 失败 | 检测 | 行为 |
|------|------|------|
| stdin 不是 TTY | `!stdin.is_tty()` | exit 1 + 消息，建议 `--api-key` / `AURA_API_KEY` |
| 用户粘贴空 key | `trim().is_empty()` | 重新提示，最多 3 次 |
| provider endpoint 不可达 | `verify.rs` 超时 | 提示"仍然保存？" 默认 No |
| provider rate-limit probe | HTTP 429 | key 已**认证** → 保存 + 状态说明 |
| keychain 写入失败（如 Linux 无 Secret Service） | `keyring::Error::NoStorage` | 回退到 `~/.config/aura/keys/<provider_id>.key`（chmod 600）+ 警告 |
| config 文件写入失败 | IO 错误 | **回滚**：删除刚写入的 keychain 条目，exit 1 — 无部分状态（A4） |
| 用户中止（Ctrl-C / SIGINT） | 按键事件或信号 | exit 130，无写入 |
| TTY init 失败（无 TTY、无 `/dev/tty`） | `enable_raw_mode()` / `EnterAlternateScreen` 错误 | 一行 stderr 消息，exit 1 |
| TUI 事件循环内 panic | ratatui `install_panic_hook` | 恢复终端，离开交替屏幕，然后重新 panic — 终端永远不会留在 raw mode |
| config.toml 无法解析 | 解析错误 | fail fast（当前行为）；`aura setup` 显示解析错误，不覆盖 |

**原子性合约（audit A4）：** **无部分状态。** `commit(provider, key, model)` 执行：

```
keychain::save(provider, key)          // 1. 凭据 — 关键产物
config::write_temp_then_rename(provider)  // 2. 临时文件 + 原子 rename
step-2 失败 → keychain::delete(provider)  // 3. 回滚
```

---

## 9. 依赖（仅新增）

| Crate | 版本 | 原因 | 风险 |
|-------|------|------|------|
| `ratatui` | `0.29` | TUI 框架 | 低 — 主流，MSRV 1.74 |
| `crossterm` | （经由 ratatui） | 跨平台终端 backend | 低 — 已由 ratatui 使用 |
| `keyring` | `3.x` | 跨平台凭据存储（macOS keychain / Linux Secret Service / Windows wincred） | **中（audit A12）** — Linux 上拉取 `zbus`；在含依赖的 slice（1.5）中验证 `cargo tree` 增长 + MSRV |

---

## 10. 测试策略

单元测试（无网络、无真实 TTY）：

- `setup::needs_onboarding()` — **将 §3 真值表移植为代码并测试每一行**
- `setup::providers::lookup(id)` / `default_for_id(id)` / `chat_url(id)` / `models_url(id)`
- `setup::keychain::save/load`（使用 keyring mock；+ legacy 服务回退）
- `setup::commit()` — **回滚测试**：keychain 写入 ok，config 写入失败 → keychain 条目被删除
- `setup::tui::app::update(msg, app)` — 纯状态转换测试
- `setup::verify` — probe 语义（含 mock HTTP server）

快照测试（ratatui `TestBackend`）：
- `setup::tui::ui::render(frame, &app)` 对每个 `App` 状态断言渲染 `Buffer` 与存储快照匹配

集成测试：
- 非 TTY 检测 → 向导在 `init()` 前退出
- 端到端事件序列 → 驱动 `App` 从 `PickProvider` 到 `Saving`，断言 `keychain.save` 被调用

回归测试：
- `aura bench run` 和 `aura --fake-model "task"` 无 key/env → `needs_onboarding()` 为 false（A6）

---

## 11. Rollout / Loop 计划

| 阶段 | 内容 | 可见性 |
|------|------|--------|
| **Slice 1** | `setup` 模块骨架 + `needs_onboarding()`（完整 §3 真值表）+ `aura setup` 子命令 stub | 无行为变化 |
| **Slice 1.5** | ratatui 骨架 — 添加依赖，创建 `setup::tui/{mod,app,ui,event,theme}.rs` 空模块 | 无用户可见变化 |
| **Slice 2** | Provider catalog（`providers.toml` + `setup::providers`） | 无用户可见变化 |
| **Slice 2.5** | 扩展解析顺序 + `keychain.rs` + `commit()` 含回滚 | 无用户可见变化 |
| **Slice 3** | TUI provider 选择器 + keychain 写入（第一个 provider） | **首个用户可见变化**；需要真实 key 测试 |
| **Slice 4** | 完整 3-provider 选择器 + verify probe | 用户可见 |
| **Slice 5** | 默认 `aura`（无 args）→ 向导触发；`--api-key` / `--fake-model` / 子命令跳过。**人工门控：确认行为变化** | 用户可见 |
| **Slice 6** | 文档更新（README "First run" + `config.example.toml` 端点约定修复 + 本文档标记"已实现"） | 文档 |

每 slice 结尾：所有测试 green（397+ 且增长）、`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`。

---

## 12. 开放问题（人工审核前）

1. **Kimi 默认模型**：推荐 `moonshot-v1-8k`（最小可用默认值）。
2. **MiniMax 默认模型**：~~`abab6.5s-chat`~~ → **已解决（audit A2）**：`MiniMax-M2.5`。
3. **多 profile**：本设计留 `alias = "..."` 为 v0.3+ 字段。确认 v0.2 不需要。
4. **Custom provider 路径**：在 v0.2 选择器中包含"Other (OpenAI-compatible)"。
5. **Linux 无 Secret Service**：`keys/` 文件回退可接受。
6. **Backend 选择**：ratatui 0.29 默认 `crossterm`，确认坚持默认选择。
7. **`color-eyre` / `better-panic`？** v0.2 不带；仅在真实用户遇到难以阅读的 panic 消息时才 revisit。
8. **鼠标支持？** v0.2 键盘专属（SSH 上可预测）。
9. **Spinner during `Verifying`？** 手写 4 帧 spinner（~15 行，无新 dep）— 推荐。
10. **`--from-stdin` 非交互模式？** v0.3 再说。
11. **已有配置冲突** → **已解决**：`ConfigConflict` 前导状态（§2/§6.2）。
12. **Verify probe 预算** → **已解决**：`min_probe_tokens`（默认 1）；429 → 已认证；两者都 400 → "仍然保存？"

---

## 13. 本文档**不**涉及的内容

- OAuth / 订阅 provider
- 模型选择器（provider 的 `default_model` 之外）
- 遥测 / phone-home
- **Config 解析顺序是扩展而非重写** — 无 `provider` 的 legacy config 行为与今天完全一致
- 本文档中无代码（按用户指示）
