# Provider Onboarding — Design Doc

> 🌐 **Language / 语言**: [English](provider-onboarding.md) · [中文](provider-onboarding.zh.md)

> Status: **Draft** (loop L1; not yet implemented)
> Author: 2026-08-10 loop session
> Reviewed: 2026-08-10 loop audit (this revision) — see "Audit delta" below
> Target: aura v0.2 — first-run wizard + multi-provider keychain
> Reference: <https://pi.dev/docs/latest/providers#api-keys>

---

## Audit delta (2026-08-10, loop L1 review)

Loop-engineering review against the actual codebase (`src/model_http.rs`, `src/config.rs`,
`src/cli.rs`, `src/main.rs`, `src/bench/runner.rs`, `STATE.md`, `loop-run-log.md`).
Every finding below is already folded into the body of this doc; this block is the summary.

| # | Severity | Finding | Fix (in this revision) |
|---|----------|---------|------------------------|
| A1 | **HIGH** | Catalog endpoints include `/v1`, but `HttpConfig::url()` appends `/v1/chat/completions` → **double `/v1/v1`** URL on every real request (`src/model_http.rs:url()`). Same latent inconsistency exists in `config.example.toml` and `tests/config.rs:17`. | Catalog stores **base URLs without path**; chat URL and probe URL are derived (`{base}/v1/...`). §1, §4.1, §7. Add a `chat_url()` unit test that pins the no-double-`/v1` invariant. |
| A2 | **MEDIUM** | MiniMax default model `MiniMax-Text-01` / `abab6.5s-chat` is stale — the real E2E and mock tests use **`MiniMax-M2.5`** (`src/model_http.rs:575`, `STATE.md`). | Default = `MiniMax-M2.5`; drop `abab6.5s-chat` from extras. Open question #2 resolved. |
| A3 | **MEDIUM** | A real key already exists in macOS keychain as service `MINIMAX_API_KEY`, account `aura` (E2E, 2026-08-09). Doc proposed service `aura-minimax` → the wizard would **orphan the existing key**. | Keep `aura-<id>` for new writes; `keychain::load` falls back to the legacy service name. §4.2. |
| A4 | **MEDIUM** | §2 happy-path sketch writes keychain **before** config, §8 says write config **first**; and §8's "leave file as-is" order violates its own "either both or neither" claim. | Single `commit()` contract: `keychain.set` → config temp-file + rename → on config failure **delete keychain entry (rollback)**. §2 and §8 aligned. |
| A5 | **MEDIUM** | `Verifying` state needs to await an HTTP future, but the event loop is specified as synchronous `event::read()`; the concurrency model is unspecified. "~30 fps" is also wrong for a blocking-read loop. | Event loop = `event::poll(50ms)` tick; verify runs on a worker thread; `oneshot` channel polled each tick (drives the spinner). Raw-mode Ctrl+C arrives as a **key event**, not SIGINT — both paths map to `Msg::Abort`. §6.2/§6.3. |
| A6 | **MEDIUM** | **Fake-mode regression**: today `aura "task"` with no key runs in fake mode with a warning; `aura bench run` (non-TTY) relies on this. After onboarding, no-key non-TTY runs exit 1 → **bench/CI break** in fresh environments. | `needs_onboarding()` returns false when `--fake-model` is set; `aura setup` always enters the wizard. §3 truth table + §11 slice-5 human gate + regression tests. |
| A7 | **MEDIUM** | §13 says "no changes to existing Config resolve order", but §4.3 adds keychain + catalog defaults → contradiction. Also precedence between `AURA_API_KEY` and provider-specific vars was ambiguous. | Resolution order restated precisely: CLI > config > provider env var > `AURA_API_KEY` > keychain; catalog defaults are the lowest layer for endpoint/model only. §4.3, §13. |
| A8 | **LOW/MED** | Verify probe `GET {endpoint}/models` assumes every provider implements `/models` (MiniMax's OpenAI-compat surface is chat-first; may 404). | Probe = **1-token chat completion** (`POST {base}/v1/chat/completions`, `max_tokens=1`) — same code path as real use, works on all OpenAI-compatible providers; `/models` as fast-path fallback only. §7. |
| A9 | **LOW/MED** | `.env` fallback conflicts with `loop-constraints.md` ("Never edit .env…") and "source it in the run shell" is impossible for a binary. | Linux fallback = `~/.config/aura/keys/<provider_id>.key` (chmod 600), read by the binary at resolution time; no shell sourcing. §4.2, §8. |
| A10 | LOW | Existing-config conflict: user has `endpoint`/`model` but no `provider`/key → wizard writes `provider` but leaves a foreign endpoint → key used against the wrong endpoint. | Wizard writes **only `provider`** (plus key); endpoint/model resolve from catalog only when absent from config; wizard warns when existing endpoint ≠ provider default. §4.3, §8. |
| A11 | LOW | Pasted keys (60+ chars) with trailing newline fire `Enter` mid-paste; hidden input with no trim. | Enable bracketed paste, strip CR/LF, trim input. §6.4. |
| A12 | LOW | `keyring` on Linux pulls `zbus` (a non-trivial dep tree); doc claimed "low risk" without checking. | Keep keyring cross-platform, but make slice 1.5 (dependency-bearing) explicitly check `cargo tree` growth + MSRV. §9, §11. |
| A13 | LOW | Trigger table missing `--fake-model` and `aura setup` rows (covered by A6). Test surface missing the truth table itself. | Port the §3 truth table to unit tests (the highest-value testable artifact). §10. |
| A14 | LOW | Q11 (open): keep-or-replace UX for an existing config was unspecified. | Concrete `ConfigConflict` preamble state: detected endpoint/model shown; `K` → keep-as-custom (writes `provider = "custom"`, normalizes trailing `/v1`), `R` → normal picker (overwrites endpoint/model with provider defaults). §2/§6.2/§8. Q11 resolved. |
| A15 | LOW | Q12 (open): `max_tokens=1` probe may be rejected by some providers. | Catalog gains `min_probe_tokens` (default 1, calibrated in slice-4 E2E); probe ladder chat → `/models`; **429 = authenticated → save with note**. §4.1/§7/§8. Q12 resolved. |

Not changed: ratatui decision (user-specified), keyboard-only input, no spinner crate, no `color-eyre`,
no `--from-stdin` in v0.2, 3-provider scope.

---

## 1. Goal

**When the user runs `aura` with no args and no existing config, the binary walks them through picking a model provider and storing an API key — without any flags, env vars, or manual `config.toml` editing.**

The wizard is a first-class **ratatui** TUI (alternate screen, raw mode, panic-safe teardown) rather than a line-printed prompt — colors, layout, and progress states are user-visible, not just text on stdout. See §6 for the framework choice and §11 for the rollout.

The three providers required for v0.2 (user-specified):

| Provider | Display name | Base URL (no path) | Default model | Env var | Keychain service |
|----------|--------------|--------------------|---------------|---------|------------------|
| DeepSeek | DeepSeek | `https://api.deepseek.com` | `deepseek-chat` | `DEEPSEEK_API_KEY` | `aura-deepseek` |
| MiniMax  | MiniMax (海螺 AI) | `https://api.minimaxi.com` | `MiniMax-M2.5` | `MINIMAX_API_KEY` | `aura-minimax` |
| Kimi     | Kimi (Moonshot) | `https://api.moonshot.cn` | `moonshot-v1-8k` | `KIMI_API_KEY` | `aura-kimi` |

**Endpoint convention (audit A1):** catalog stores **base URLs without any path suffix**.
`HttpConfig::url()` already appends `/v1/chat/completions` (`src/model_http.rs`), so writing
`https://api.deepseek.com/v1` into the catalog would produce `https://api.deepseek.com/v1/v1/chat/completions`.
All derived URLs (chat, probe) are built as `{base}/v1/...`. The same rule must be applied to
`config.example.toml` and `tests/config.rs` (which currently document the `/v1`-suffixed form).

These names/URLs are based on the public docs as of writing and the real-model E2E (MiniMax
`MiniMax-M2.5`, 2026-08-09); final list lives in a single `providers.toml` shipped with the binary (see §4.1).

---

## 2. User flow (happy path)

```text
┌─ Aura 0.2 — first-time setup ─────────────────────────────────┐
│                                                               │
│  No API key found. Pick a provider to continue:               │
│                                                               │
│    1) DeepSeek         https://api.deepseek.com               │
│    2) MiniMax          https://api.minimaxi.com               │
│    3) Kimi / Moonshot  https://api.moonshot.cn                │
│    4) Other (OpenAI-compatible)                               │
│    5) I already have AURA_API_KEY in my shell — skip          │
│                                                               │
│  Choose [1-5]: 1                                              │
│  > _                                                        │
│                                                               │
│  Paste your DeepSeek API key (input is hidden):               │
│  > sk-********************************                        │
│                                                               │
│  [ Verifying key against DeepSeek … ]                         │
│  [ ✓ key accepted, saved to keychain (aura-deepseek) ]        │
│  [ ✓ Wrote ~/.config/aura/config.toml (provider = deepseek) ] │
│                                                               │
│  Next: run `aura "<your task>"` to start.  [ press any key ]  │
└───────────────────────────────────────────────────────────────┘
```

Notes (audit A4/A10):

- **Write order**: keychain **first**, config second; if the config write fails, the keychain entry is
  deleted (rollback) — no partial state. §8.
- The wizard writes **only `provider = "deepseek"`** to config (and the key to the keychain).
  `endpoint`/`model` are resolved from the catalog at run time, so provider fixes (e.g. a renamed
  model) never strand a stale URL in the user's config.
- If the user's config already has `endpoint`/`model` but no `provider`/key, the wizard starts in a
  `ConfigConflict` preamble (Q11):

  ```text
  Detected ~/.config/aura/config.toml currently points at:
    endpoint = https://api.openai.com           (trailing /v1 normalized away)
    model    = gpt-4o
  [K]eep these as-is (custom provider)   [R]eplace with provider defaults   [Esc] abort
  ```

  `K` → keep-as-custom: edit/confirm endpoint+model, then key entry, then save writes
  `provider = "custom"` (+ endpoint/model) so the key lands in `aura-custom`. `R` → normal
  picker; on save, endpoint/model are **overwritten** with the picked provider's defaults
  (never a silent mix of a foreign endpoint with a new provider key — A10).
- If the picked provider's env var is already set in the shell (e.g. `DEEPSEEK_API_KEY`), the list
  marks it "(already set)" and the key-entry step is skipped — the wizard only writes config.

After the wizard exits, the TUI alternate screen closes, the terminal
restores to its prior state, and the user is back in their normal shell.

---

## 3. Trigger logic

A new module `setup::needs_onboarding()` decides whether to enter the wizard. **Truth table**
(this table is ported verbatim to unit tests — see §10):

| CLI args | env `AURA_API_KEY` | config.toml `api_key` | keychain entry | Action |
|----------|-------------------|----------------------|----------------|--------|
| `--fake-model` (any task) | any | any | any | **skip** (explicit fake; A6 — preserves bench/CI behavior) |
| `aura bench …` / `aura --version` / `aura --help` / `aura --json …` | any | any | any | **skip** (subcommand/non-task invocations) |
| `aura setup` (subcommand) | any | any | any | **enter wizard always** (explicit intent: switch provider / rotate key) |
| any task with `--api-key` given | any | any | any | **skip** (flag wins; key never touches disk) |
| any task, env `AURA_API_KEY` set | set | any | any | **skip** (env beats wizard) |
| any task, provider env var set (`DEEPSEEK_API_KEY` etc.) | unset | any | any | **skip** (provider-specific var beats wizard) |
| **none of the above, no key** | unset | absent | absent | **enter wizard** |
| task given but key missing | unset | absent | absent | **enter wizard** (do not silently fail) |

**Behavior change vs today (audit A6):** currently `aura "task"` with no key runs in fake mode with a
warning, and `aura bench run` (non-TTY) depends on that. After this change, a no-key **interactive**
run enters the wizard, and a no-key **non-TTY** run exits 1 (see §6.5). `--fake-model` remains the
explicit opt-in for scripted/bench use. This is called out at the slice-5 human gate (§11).

The wizard itself is **non-blocking for the user** — `Ctrl+C` (raw-mode key event **or** external
SIGINT) exits with code 130; partial state is not persisted (atomic file write + keychain write at
the end only).

---

## 4. Data model

### 4.1 `providers.toml` (shipped, read-only)

A static catalog baked into the binary at compile time via `include_str!`. One stanza per provider:

```toml
# aura built-in provider catalog. Edit this file in src/ to add a provider.
# Each provider is an OpenAI-compatible chat completion endpoint.
#
# NOTE (audit A1): `base_url` carries NO path suffix. Derive everything from it:
#   chat_url  = base_url + "/v1/chat/completions"   (matches HttpConfig::url())
#   models_url = base_url + "/v1/models"            (probe fast path)
# Do NOT write "https://api.X.com/v1" here — HttpConfig appends /v1 itself.
#
# Probe tuning (audit A15/Q12): `min_probe_tokens` is the max_tokens used by the verify probe
# (§7). Default 1; raise per provider ONLY if its API rejects max_tokens=1 (observe the 400
# during slice-4 E2E, then bump here with a comment — never special-case in code).

[[providers]]
id = "deepseek"
display_name = "DeepSeek"
base_url = "https://api.deepseek.com"
default_model = "deepseek-chat"
env_var = "DEEPSEEK_API_KEY"
keychain_service = "aura-deepseek"
# Optional: model catalog for picker (empty = use default_model only)
extra_models = ["deepseek-reasoner"]
min_probe_tokens = 1

[[providers]]
id = "minimax"
display_name = "MiniMax"
base_url = "https://api.minimaxi.com"
default_model = "MiniMax-M2.5"          # audit A2: model verified in real E2E (2026-08-09)
env_var = "MINIMAX_API_KEY"
keychain_service = "aura-minimax"
extra_models = ["MiniMax-M2.5"]          # re-verify against live docs before shipping
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
base_url = ""               # user fills in (bare host, no /v1)
default_model = ""          # user fills in
env_var = "AURA_API_KEY"    # falls through to existing var
keychain_service = "aura-custom"
extra_models = []
min_probe_tokens = 1
```

`setup::providers` exposes `chat_url(id)` / `models_url(id)` as the **only** way to build URLs from a
provider — unit-tested against the double-`/v1` invariant.

### 4.2 Keychain entries (macOS / Linux Secret Service)

| Service | Account | Stores |
|---------|---------|--------|
| `aura-deepseek` | `aura` | API key for DeepSeek |
| `aura-minimax`  | `aura` | API key for MiniMax |
| `aura-kimi`     | `aura` | API key for Kimi |
| `aura-custom`   | `aura` | API key for "Other" |
| `aura-gh-publish` | `gyc567` | (transient, used by release pipeline; unrelated) |

**Legacy entry (audit A3):** a real MiniMax key already exists on this machine as service
`MINIMAX_API_KEY`, account `aura` (stored during the 2026-08-09 real-model E2E). `keychain::load`
must try `aura-<id>` first, then fall back to the legacy service name (`MINIMAX_API_KEY`), so the
existing key round-trips without re-entry. The next `aura setup` re-save migrates it to `aura-minimax`.

**Linux without Secret Service (audit A9):** fall back to writing the key to
`~/.config/aura/keys/<provider_id>.key` (chmod 600, one line, no quoting/escaping semantics). The
binary reads this file itself at resolution time — there is no shell sourcing, and no `.env` file is
ever written (consistent with `loop-constraints.md`). Only on macOS / Linux-with-keyring is the
keychain path taken. Windows uses `wincred`.

Cross-platform keychain access goes through the `keyring` crate (Rust standard for this — no new dep
on a homegrown abstraction).

### 4.3 `~/.config/aura/config.toml` (extended schema)

Current schema (3 fields) is **kept; new fields are optional and additive**:

```toml
# Existing fields — unchanged
endpoint = "https://api.deepseek.com"   # base URL, NO /v1 suffix (audit A1)
model    = "deepseek-chat"
# api_key = "..."   # discouraged; keychain preferred

# New (optional)
provider = "deepseek"            # maps to providers.toml id
# alias = "my-deepseek"           # future: multi-profile
```

**Resolution order (audit A7 — restated precisely; backward compatible):**

1. CLI flag (`--api-key`, `--endpoint`, `--model`) — **highest**
2. `config.toml` (`endpoint`, `model`, `provider`)
3. Provider-specific env var from catalog (e.g. `DEEPSEEK_API_KEY`), **only when `provider` is known**
   (more specific beats the generic var)
4. `AURA_API_KEY` env var
5. keychain lookup (`keyring::Entry::get_password`) / `keys/` file fallback — driven by `provider`
6. **Lowest layer, endpoint/model only:** catalog defaults (`base_url`, `default_model`) — filled in
   only when config leaves them absent and `provider` is known

Notes:

- `provider` is what unlocks keychain lookup and catalog defaults. A **legacy config without
  `provider`** keeps today's exact behavior (CLI > config > `AURA_API_KEY`; keychain never consulted).
- **Never print, log, or echo the key** at any layer; `HttpConfig`'s `Debug` already masks it.
- `config.toml` parse failure: still fail fast (current behavior) — the wizard must not run on a
  broken config; `aura setup` shows the parse error instead.

---

## 5. Module / file layout

```
src/
├── main.rs                  # add: detect → wizard → existing flow
│   ├── mod.rs               #   needs_onboarding() + run_wizard() (the Elm-style loop driver)
│   ├── providers.rs         #   Provider catalog (loads providers.toml; chat_url/models_url)
│   ├── tui/                 #   NEW: ratatui-based wizard UI
│   │   ├── mod.rs           #     Terminal init/restore, panic hook, alternate-screen lifecycle
│   │   ├── app.rs           #     Wizard state machine (App struct + Message enum)
│   │   ├── ui.rs            #     Pure render fn(frame, &app) -> ()  (snapshot-testable)
│   │   ├── event.rs         #     Key/mouse/resize/tick → Message
│   │   └── theme.rs         #     Color palette + Style helpers (single source of truth)
│   ├── prompt.rs            #   THIN: only the masked-text-input widget (delegated to tui/)
│   ├── keychain.rs          #   save(key) / load(provider) via `keyring` crate (+ legacy fallback)
│   └── verify.rs            #   probe future (worker thread + oneshot; see §6.2/§7)
├── config.rs                # EXTEND: add `provider` field; resolve() extended per §4.3
├── providers.toml   # NEW: embedded via include_str!; see §4.1
```

CLI: add a single non-default subcommand `aura setup` to re-run the wizard (e.g. to switch provider
or rotate a key). Default behavior (`aura` with no args) is what triggers the wizard per §3.

---

## 6. TUI design

### 6.1 TUI framework

Wizard renders in **ratatui 0.30** (chosen per <https://ratatui.rs/installation/>) using the
default **`crossterm`** backend — works on Linux + macOS + Windows out of the box, no
platform-conditional code in `setup/`. Backend is selectable at compile time:

```toml
# Cargo.toml — default (crossterm)
ratatui = "0.30"

# alternative backends, pick exactly one:
# ratatui = { version = "0.30", default-features = false, features = ["termion"] }
# ratatui = { version = "0.30", default-features = false, features = ["termwiz"]  }
# ratatui = { version = "0.30", default-features = false, features = ["termina"]  }
```

Why ratatui over a hand-rolled stdin reader:
- Real layout primitives (`Layout::vertical`, `Constraint::Percentage`) — we don't hand-draw boxes
- Alternate-screen mode + cursor hide/show is one call pair, not 20 lines of escape codes
- `TestBackend` makes the whole UI snapshot-testable without a TTY (§10)
- Mainstream crate, frequent releases (0.30.2 current), well-documented recipes

### 6.2 App architecture

The wizard follows ratatui's **Elm architecture** (per
<https://ratatui.rs/concepts/application-patterns/the-elm-architecture/>):

```text
                ┌───────────────────────┐
                │   Event (key/mouse)   │
                └──────────┬────────────┘
                           ▼
                ┌───────────────────────┐
                │   update(msg, app)    │  → new App
                └──────────┬────────────┘
                           ▼
   ┌───────────────────────────────────────┐
   │   ui::render(frame, &app)             │  (pure, no IO)
   └───────────────────────────────────────┘
                           ▼
                ┌───────────────────────┐
                │   crossterm backend    │  → terminal
                └───────────────────────┘
```

**Event loop and concurrency (audit A5):** the loop is event-driven with a tick, **not** a 30 fps
render loop and **not** a blocking `event::read()`:

```text
loop {
    match event::poll(Duration::from_millis(50))? {   // 50 ms tick
        true  → msg = event::read()?                    // key / paste / resize
        false → msg = Msg::Tick                          // spinner frame advance
    }
    if let Ok(result) = verify_rx.try_recv() { msg = Msg::VerifyDone(result) }  // §7
    app = update(msg, app)?;
    terminal.draw(|f| ui::render(f, &app))?;
}
```

- **`Verifying` state**: the HTTP probe runs on a detached worker thread (std thread + `oneshot`
  channel), because the main thread is busy in the synchronous event loop. The channel is polled on
  every tick. No tokio runtime is needed inside the wizard — this keeps `aura setup` independent of
  the agent-mode runtime.
- **Ctrl+C in raw mode is a key event** (`KeyCode::Char('c')` + `CONTROL`), **not** a SIGINT.
  Both that key event and an external SIGINT (e.g. `kill`) map to `Msg::Abort`. `main.rs`'s tokio
  SIGINT handler is only installed in agent mode, so there is no double-handling.
- **Paste** (`EnableBracketedPaste`): a pasted multi-line buffer arrives as one `Paste` event and is
  filtered to printable chars (CR/LF stripped) — see §6.4.

States in the state machine:

1. `ConfigConflict`  — preamble, only when config has `endpoint`/`model` but no `provider`/key:
   shows the detected values; `K` → keep-as-custom path (`PickCustomUrl` prefilled),
   `R` → `PickProvider` (replace; overwrites endpoint/model on save), Esc → `Aborted` (Q11)
2. `PickProvider`    — highlight provider list, Enter to confirm; marks "(already set)" for env-backed providers
3. `EnterApiKey`     — masked text input (each char renders as `*`)
4. `Verifying`       — spinner (tick-driven) + status line (await `verify.rs` via oneshot, §6.2)
5. `Saving`          — write keychain → write config (atomic, §8)
6. `Done`            — success summary + "[ press any key ]"
7. `Error`           — recoverable errors (retry / abort) — never crashes the TUI
8. `PickCustomUrl`   — "Other (OpenAI-compatible)" and the keep-as-custom path — endpoint + model text inputs
9. `Aborted`         — user pressed Esc / Ctrl-C — clean exit

### 6.3 Terminal lifecycle

`setup::tui::mod` owns this, called from `setup::run_wizard`:

```text
init()  →  enable_raw_mode()        (crossterm)
         →  EnterAlternateScreen
         →  EnableBracketedPaste     (audit A11)
         →  Hide cursor
         →  install panic hook      (restores terminal on panic)
         →  install SIGINT handler  (translates to Msg::Abort; external kill only — see §6.2)

loop    →  event::poll(50ms tick) → update() → render()

exit()  →  Show cursor
         →  LeaveAlternateScreen
         →  disable_raw_mode()
         →  ALWAYS run (Drop impl + panic hook)
```

If `init()` fails (e.g. `/dev/tty` not available) → fall back to a one-line
plain-text prompt to stderr (`aura setup: terminal unavailable; export
AURA_API_KEY=<key> and re-run`). Never panics inside the TUI; all error paths
lead to the `Error` state which renders a recoverable message.

### 6.4 Input specifics

- **Provider pick**: number keys `1..=5` (or arrow keys + Enter). Out-of-range → no-op + status line flash.
- **API key input**: any printable char → `*` rendered, real char stored in app state. Backspace →
  erase. Ctrl-U → clear. Esc → `Aborted`. Enter → `Verifying`.
- **Paste (audit A11)**: bracketed paste enabled; a `Paste` event is filtered to printable chars,
  CR/LF stripped, then appended to the input buffer **without** triggering Enter. Input is trimmed
  (leading/trailing whitespace) before verify.
- **Mouse**: not enabled in v0.2 (keeps the TUI predictable over SSH).
- **Resize** (`SIGWINCH` via crossterm): re-render at new size; no state change.

### 6.5 Non-TTY fallback (CI, piped stdin)

Before `init()`, the wizard checks `stdin().is_tty() && stdout().is_tty()`. If either is false:
- Default: `aura` exits 1 with a one-line plain-text message: *"aura: no TTY detected; run `aura setup` interactively or set AURA_API_KEY=<key>"*
- **Regression guard (audit A6):** `needs_onboarding()` returns false for `--fake-model` and for
  subcommands, so `aura bench run` (spawns `aura --json …` in a pipe) and scripted fake-mode runs
  never hit this path.
- `aura setup --non-interactive` (planned for v0.3, not v0.2): read provider + key from stdin as
  `provider\nkey\n` — out of scope this slice.

---

## 7. Key verification (optional but recommended)

After the user pastes a key, do a cheap probe before saving (audit A8):

```http
POST {base}/v1/chat/completions
Authorization: Bearer {key}
{ "model": "{default_model}", "messages": [{"role":"user","content":"ping"}], "max_tokens": 1 }
```

- 200 → save
- 401/403 → re-prompt with *"key rejected by server — paste again or Ctrl-C to abort"*
- 429 → **authenticated** (rate limit comes after auth) → save, with a status note:
  *"key accepted but rate-limited; you may hit throttling on first use"* (Q12)
- 400/404/405 on chat → fast-path fallback: `GET {base}/v1/models` (some providers expose a models
  list but not chat); interpret the same way. If both fail → *"couldn't verify key; save anyway?
  [y/N]"* — defaults to No (Q12)
- network error / timeout → ask *"couldn't reach {base}; save anyway? [y/N]"* — defaults to No

Why a 1-token chat completion instead of `GET /models`: it exercises the **exact code path the agent
will use**, and it works on every OpenAI-compatible provider — several (notably MiniMax) are
chat-first and may not implement `GET /models`, which would falsely report a valid key as
"unreachable". Cost is negligible (`max_tokens=1`).

The probe body uses `max_tokens = provider.min_probe_tokens` from the catalog (default 1; §4.1).
If a provider rejects even that (HTTP 400 mentioning token limits), the field is bumped during the
slice-4 E2E — the code never special-cases a provider (Q12).

The probe is **optional, fast, and idempotent**. Failures don't block; they just inform. Total
budget ≤ 3 s with 2 s timeout. **The key never appears in any log/error message** — only a masked
form like `sk-ab…cd` if ever surfaced (prefer: never).

Rationale: a typo'd key silently written to keychain is much worse than a 1-second network
round-trip. The cost of one bad key (user runs `aura` later, gets cryptic HTTP 401 from a tool call)
outweighs the cost of an upfront check.

---

## 8. Failure modes (explicit)

| Failure | Detection | Behavior |
|---------|-----------|----------|
| stdin not a TTY | `!stdin.is_tty()` | exit 1 + message, suggest `--api-key` / `AURA_API_KEY`; never reached for `--fake-model`/subcommands (A6) |
| user pastes empty key | `trim().is_empty()` | re-prompt, max 3 tries |
| provider endpoint unreachable | `verify.rs` timeout | prompt "save anyway?" default No |
| provider rate-limits the probe | HTTP 429 | key is **authenticated** → save + status note (Q12) |
| provider rejects the probe request (400 token/model limits) | HTTP 400 on chat **and** `/models` | "couldn't verify key; save anyway? [y/N]" default No; fix by raising `min_probe_tokens` or correcting the catalog model in slice-4 E2E (Q12) |
| keychain write fails (e.g. Linux without Secret Service) | `keyring::Error::NoStorage` | fall back to `~/.config/aura/keys/<provider_id>.key` (chmod 600) + warn |
| config file write fails | IO error | **rollback**: delete the just-written keychain entry, exit 1 — no partial state (A4) |
| user aborts (Ctrl-C / SIGINT) | key event or signal | exit 130, no writes (keychain write is the very last step) |
| `--api-key` flag given | CLI parse | **skip wizard** — flag wins, key never touches disk |
| TUI init fails (no TTY, no `/dev/tty`) | `enable_raw_mode()` / `EnterAlternateScreen` error | one-line stderr msg, exit 1 (see §6.3) |
| Panic inside TUI event loop | ratatui's `install_panic_hook` | restore terminal, leave alternate screen, then re-panic with original message — terminal is never left in raw mode |
| Terminal resize during save | SIGWINCH | re-render only; the in-flight save future is unaffected |
| existing config has `endpoint`/`model` but no `provider` | config scan at wizard start | `ConfigConflict` preamble (Q11): `K` → keep-as-custom (writes `provider = "custom"`, normalizes trailing `/v1`), `R` → picker, overwrites endpoint/model with provider defaults; never silently mixes a foreign endpoint with a new provider key (A10) |
| config.toml unparseable | parse error at startup | fail fast (current behavior); `aura setup` shows the parse error, does not overwrite |
| `color_eyre` / `better-panic` hook | startup | **not used in v0.2** (keeps dep tree small); revisit if a real user gets a confusing panic |

**Atomicity contract (audit A4):** **no partial state.** `commit(provider, key, model)` runs:

```text
keychain::save(provider, key)          // 1. credential — the critical artifact
config::write_temp_then_rename(provider)  // 2. temp file + atomic rename in same dir
on step-2 failure → keychain::delete(provider)  // 3. rollback
```

Either both keychain + config get written, or neither. The §2 happy-path order (keychain first,
config second) matches this contract.

---

## 9. Dependencies (additions only)

| Crate | Version | Why | Risk |
|-------|---------|-----|------|
| `ratatui` | `0.30` | TUI framework — `ui.rs` / `app.rs` / `event.rs` (default `crossterm` backend, per <https://ratatui.rs/installation/>) | low — mainstream, frequent releases, MSRV 1.74 |
| `crossterm` | (transitive via `ratatui`) | Cross-platform terminal backend; pulls in `enable_raw_mode`, `EnterAlternateScreen`, `event::read/poll`, panic hook | low — already used by ratatui, no extra surface |
| `keyring` | `3.x` | Cross-platform credential store (macOS keychain / Linux Secret Service / Windows wincred) | **medium (audit A12)** — on Linux it pulls `zbus` (a real dep-tree growth); verify `cargo tree` + MSRV in the dependency-bearing slice (1.5) before committing |

**Optional / under review** (not in v0.2 unless a real need surfaces):

| Crate | When | Why |
|-------|------|-----|
| `color-eyre` | if a real user sees a confusing panic | per <https://ratatui.rs/recipes/apps/color-eyre/> — nicer panic pages, but adds a non-trivial dep |
| `better-panic` | same | lighter-weight alternative to `color-eyre` |
| `insta` | snapshot tests for `tui::ui` | ratatui's `TestBackend` produces `Buffer`; `insta` snapshots are the idiomatic way (see <https://ratatui.rs/recipes/testing/>) |

**Removed from earlier draft**: `termion` (no longer needed; ratatui + crossterm
cover everything termion would have). No tokio additions are needed inside the wizard (the probe
runs on a std thread, §6.2).

---

## 10. Testing strategy

Unit (no network, no real TTY):

- `setup::needs_onboarding()` — **port the §3 truth table to code and test every row** (highest-value
  testable artifact; includes `--fake-model`, `aura setup`, provider-env-var, bench rows)
- `setup::providers::lookup(id)` — id → Provider struct
- `setup::providers::default_for_id(id)` — id → (base_url, model, env_var)
- `setup::providers::chat_url(id)` / `models_url(id)` — **assert no double `/v1`**:
  `chat_url("deepseek") == "https://api.deepseek.com/v1/chat/completions"` (A1 regression test)
- `setup::keychain::save/load` — using `keyring`'s mock feature (`#[cfg(test)]` with `keyring::mock`);
  plus the legacy-service fallback (`load("minimax")` finds `MINIMAX_API_KEY` entry) (A3)
- `setup::commit()` — **rollback test**: keychain write ok, config write fails → keychain entry is
  deleted (A4)
- `setup::tui::app::update(msg, app)` — pure state transitions:
  - `Msg::Key('k')` on `ConfigConflict` → keep-as-custom path (`PickCustomUrl` prefilled with
    existing endpoint/model, trailing `/v1` normalized) (Q11)
  - `Msg::Key('r')` on `ConfigConflict` → `PickProvider` (replace; endpoint/model overwritten on save) (Q11)
  - `Msg::Key('1')` on `PickProvider` → transitions to `EnterApiKey{provider: deepseek}`
  - `Msg::Char('a')` on `EnterApiKey` → `app.input.push('a')`, render shows `*`
  - `Msg::Enter` on `EnterApiKey` → transitions to `Verifying` (if input non-empty)
  - `Msg::Tick` on `Verifying` → spinner frame advances (no state change)
  - `Msg::VerifyDone(Ok(_))` on `Verifying` → transitions to `Saving`
- `setup::verify` — probe semantics with a mock HTTP server: 200 → Ok; 401 → Rejected;
  429 → Accepted (rate-limited note); 400 on chat → falls back to `/models`; both fail →
  Unverifiable (A8/Q12)
- `setup::verify` — probe request body uses `max_tokens = provider.min_probe_tokens`
  (default 1; catalog override honored) (Q12)

Snapshot tests for the renderer (the big win ratatui gives us):

- `setup::tui::ui::render(frame, &app)` against `ratatui::backend::TestBackend`
- For each `App` state, assert the rendered `Buffer` matches a stored snapshot
  (per <https://ratatui.rs/recipes/testing/testing-with-insta-snapshots/>)
- Snapshots committed under `tests/snapshots/` so any visual regression is a
  diff in code review
- One snapshot per state × one representative provider list = ~6 snapshots;
  the file count stays small

Integration (with mock backend):

- `tests/setup_wizard.rs`: non-TTY detection → wizard bails before `init()`
- `tests/setup_wizard.rs`: end-to-end event sequence (`1`, `s`, `k`, `-`, `t`, `e`, `s`, `t`, Enter)
  → drives `App` from `PickProvider` to `Saving`, asserts `keychain.save` was
  called with the right `(service, account, key)` (mock keyring)
- `tests/config_resolve.rs`: precedence still works (CLI > config > provider-env > AURA_API_KEY >
  keychain); **legacy config without `provider` keeps today's behavior** (A7)
- `tests/bench_no_wizard.rs`: `aura bench run` and `aura --fake-model "task"` with no key/env →
  `needs_onboarding()` is false (A6 regression test)

End-to-end (manual, gated on user approval):

- One real key per provider (DeepSeek/MiniMax/Kimi) — verify probe returns 200 and saved key
  round-trips through `keyring`. This is **the** smoke test for v0.2. Without it, the wizard is a UI
  demo.

End-to-end (TUI rendering, manual):

- Run `aura setup` in a real terminal (iTerm2, Terminal.app, gnome-terminal, Windows Terminal) —
  confirm the layout matches the §2 sketch, masked input is masked, paste does not fire Enter, Esc
  cleanly exits, terminal state is restored.

---

## 11. Rollout / loop plan

L1 (this doc, no code) → DONE (you are reading it; audit revision folded in)

L2 (single end-to-end slice, human gate before merge):

- **Slice 1**: `setup` module skeleton + `needs_onboarding()` (full §3 truth table incl. `--fake-model`,
  `aura setup`, provider-env rows) + `aura setup` subcommand stub (prints "not implemented")
  - 0 net behavior change; verifies module layout; **the truth table is tested before any UI exists**
- **Slice 1.5**: ratatui skeleton — add `ratatui` + `crossterm` to `Cargo.toml`, create
  `setup::tui/{mod,app,ui,event,theme}.rs` with empty stubs; one smoke test that initializes a
  `TestBackend` and renders an empty frame. 0 user-visible; this is the dependency-bearing slice
  (run `cargo build` early to catch MSRV / cross-platform issues; **check `cargo tree` growth from
  `keyring`'s zbus tree** — audit A12)
- **Slice 2**: provider catalog (`providers.toml` + `setup::providers` incl.
  `chat_url`/`models_url` no-double-`/v1` tests)
  - 0 user-visible; pure data plumbing
- **Slice 2.5**: resolution extension — `config.provider` field, `resolve()` per §4.3 (catalog
  defaults + keychain lowest layer), `keychain.rs` save/load/legacy-fallback, `commit()` with
  rollback; all unit-tested. 0 user-visible; makes the later slices testable without a TTY
- **Slice 3**: TUI provider picker + keychain write for one provider (DeepSeek first)
  - **first user-visible change**; needs real key to test
- **Slice 4**: full 3-provider picker + verify probe (chat-completion probe + `/models` fallback)
- **Slice 5**: default `aura` (no args) → wizard trigger; `--api-key` / `--fake-model` / subcommands
  skip. **Human gate: confirm the behavior change** — no-key non-TTY runs (incl. `aura bench run` in a
  fresh env) now exit 1 instead of silently faking (A6)
- **Slice 6**: docs update (README "First run" section + `config.example.toml` endpoint convention
  fix + this doc → "Implemented")

Each slice ends with: all tests green (397+ and growing), `cargo fmt --check`,
`cargo clippy --all-targets -- -D warnings`, one human E2E per slice that introduces new
user-visible behavior.

No L3 (auto-merge) without explicit human ratification after slice 6.

---

## 12. Open questions (for human review before slice 1)

1. **Kimi default model**: `moonshot-v1-8k` is the smallest context. Should default be
   `moonshot-v1-32k` (more useful, slightly pricier)? — recommend 8k to match "smallest viable default"
2. **MiniMax default model**: ~~`MiniMax-Text-01` vs `abab6.5s-chat`~~ → **RESOLVED (audit A2)**:
   `MiniMax-M2.5`, the model proven in the 2026-08-09 real-model E2E and already used in
   `src/model_http.rs` mock tests. Remaining action: re-verify the exact model id string against
   live MiniMax docs before shipping the catalog.
3. **Multi-profile later?**: this doc leaves `alias = "..."` as a v0.3+ field. Confirm not needed in
   v0.2 (a single active provider per user)
4. **Custom-provider path**: include "Other (OpenAI-compatible)" in the v0.2 picker, or defer to
   v0.3? — recommend include (the 5th option; 5 lines of code)
5. **Linux without Secret Service**: is the `keys/`-file fallback acceptable, or do we hard-require
   keyring? — **resolved (audit A9)**: fallback with warning; the `.env` option is rejected because
   `loop-constraints.md` forbids editing `.env` files and "shell sourcing" is not something a binary
   can do
6. **Backend choice**: ratatui 0.30 defaults to `crossterm`, which is the right pick for our
   5-platform binary matrix (Linux/macOS/Windows). Confirm we stick with the default; only revisit if
   a real user reports a Windows-specific bug that crossterm can't handle.
7. **`color-eyre` / `better-panic`?** v0.2 ships without; revisit only after a real user files a
   "the panic message was unreadable" issue. Default to no.
8. **Mouse support in TUI?** v0.2 ships keyboard-only (predictable over SSH). Confirm — recommend no.
9. **Spinner / progress animation during `Verifying`?** ratatui doesn't ship one; the tick-driven
   event loop (§6.2) makes a hand-rolled 4-frame spinner trivial (~15 lines, no new dep) — recommend
   the tiny built-in spinner over adding `throbber-widgets-tui`.
10. **Where does the key live when `--api-key` is NOT given AND stdin is a pipe?** Current plan
    (§6.5): exit 1 with a plain message. Some users may want `aura setup --from-stdin` to accept
    `provider\nkey\n` on stdin. Recommend defer to v0.3.
11. **Existing-config conflict (audit A10)** → **RESOLVED**: `ConfigConflict` preamble state
    (§2/§6.2). `K` keeps the existing endpoint/model as a custom provider (writes
    `provider = "custom"`, normalizes a legacy trailing `/v1` with a visible note); `R` runs the
    normal picker and overwrites endpoint/model with the picked provider's defaults. Rationale:
    never silently mix a foreign endpoint with a new provider key; `K`-path reuses `PickCustomUrl`
    so it is ~5 lines of state, no new UI.
12. **Verify probe budget** → **RESOLVED**: catalog field `min_probe_tokens` (default 1, §4.1).
    Probe uses `max_tokens = provider.min_probe_tokens`; if a provider rejects it, bump the field
    during slice-4 E2E with a comment — no per-provider code. Added the 429 → authenticated rule
    (rate limit implies a valid key), and 400-on-both → "save anyway?" default No.

## 13. What this doc does NOT do

- No OAuth / subscription providers (Pi supports ChatGPT/Claude/Copilot via OAuth; aura v0.2 does
  not — out of scope per user request, which named only 3 API-key providers)
- No "model picker" beyond the provider's `default_model` + a small list of well-known models per provider
- No telemetry / phone-home
- **The `Config` resolve order is extended (keychain + catalog defaults, §4.3), not rewritten** —
  legacy configs without `provider` behave exactly as today (audit A7)
- No code in this document (per user instruction)

With the ratatui addition (2026-08-10), the following are also out of scope for v0.2:

- No TUI for the main agent loop (still a single-shot CLI; the TUI is **only** for the onboarding
  wizard). A full TUI for the agent itself is a v0.3+ effort — bigger scope, different concurrency model.
- No mouse interaction in the wizard (keyboard only; §12 Q8).
- No `color-eyre` or `better-panic` — revisit only if needed (§12 Q7).
- No `--from-stdin` non-interactive mode — defer to v0.3 (§12 Q10).
- No `.env` files are ever written (audit A9; loop-constraints.md).
