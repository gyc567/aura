# Loop State — Aura Coding Agent

Last run: 2026-08-09T17:00Z (Full audit of uncommitted changes; 3H+4M+3S findings, all high/medium fixed & re-verified; 356 tests GREEN)

## High Priority

- **全面审计（2026-08-09）** — ✅ COMPLETE，详见 [`docs/audit-2026-08-09.md`](docs/audit-2026-08-09.md)
  - 高危 H1-H3 已修复：release.yml artifact 路径不一致、compaction 多字节切片 panic（+边界测试）、Windows zip 用 PowerShell
  - 安全 S1-S3 已修复：install.sh curl `--` 分隔 + AURA_SHA256 校验、publish tag 格式校验、zip 单条目提取
  - 中危 M4 已修复：Session::push 先写 transcript 后推内存；M1（压缩写回 session）/M2（scratchpad 并发）/M3（resume model 元数据）列入建议
  - 审计修复过程自发现并修复 H4-H6：curl `--` 位置、`ARTIFACT#*.` 扩展名、macOS sha256sum 无 `-c`
  - 复验：356 tests / fmt / clippy 全绿；install.sh SHA256 正误分支、端到端安装均实测通过
  - `.github/workflows/release.yml`：5 平台原生构建矩阵（linux x64/arm64、macos x64/arm64、windows x64）+ tar.gz/zip 打包（含 README/LICENSE/config.example.toml）+ artifact upload + tag 触发 draft release + install.sh 上传
  - `release/install.sh`：平台/架构检测 → 下载对应资产 → 解压 → 装到 `~/.local/bin` → PATH 提示 → `--version` 自验；支持 AURA_REPO/AURA_VERSION/AURA_INSTALL_DIR/AURA_RELEASE_URL 覆盖
  - 本地验证：macOS 双架构（x86_64+arm64）release 构建并运行通过；打包 tar.gz/zip 结构正确；install.sh 用本地 HTTP 模拟 release 全链路通过
  - 注意：真实 CI 跑通需推送后观察 Actions（本沙箱无法触发 GitHub）
- **配置文件支持** — ✅ COMPLETE
  - `~/.config/aura/config.toml`（AURA_CONFIG / XDG_CONFIG_HOME 可覆盖）：endpoint/model/api_key
  - 优先级：CLI 参数 > 配置文件 > AURA_API_KEY 环境变量；坏配置 fail fast、缺配置无副作用
  - 10 个新测试（7 单测 + 3 集成）；示例 `config.example.toml`
- **v1.2 Bench Framework** — ✅ COMPLETE (Phase B1+B2)
  - Design doc: `docs/bench-framework.md`
  - Phase B1: TaskSpec/Workspace/Runner/Report ✅, `aura bench run/report/init/list` ✅, 8 seed tasks ✅, 22 bench tests ✅
  - Phase B2: `--parallel` execution ✅, `format_diff_report` ✅, `bench diff` CLI ✅, `bench init` scaffold ✅, `bench report` from results dir ✅
  - Remaining: Docker sandbox (optional), result diff CLI command, `bench submit` (future)
- **Phase 5 accepted at 91%**: remaining 9% (`cli.rs` Clap derive, `model_http::complete()` no mock HTTP, `main.rs` binary helpers) — requires architectural decision; defer to Phase 5 revisit
- **Phase 6 ✅**: scratchpad CLI wiring + max_wall_time + Budget extension
- **RLM subagent** (Phase 6): ✅ complete — ChildRegistry + subagent tool + agent_message tool + multi-thread runtime + max_depth recursion

## Quality Gates

| Gate | Status |
|------|--------|
| `cargo fmt --check` | ✅ |
| `cargo clippy -D warnings` | ✅ 0 warnings (all targets) |
| `cargo test --workspace` | ✅ 356 tests |

## Phase Status

| Phase | Status | Notes |
|------|--------|-------|
| v1 (L0) | ✅ done | 215 tests, 91% cov |
| Phase 1–4 | ✅ done | |
| Phase 5 | ⚠️ 91% | accepted |
| **Phase 6** (RLM) | ✅ done | ChildRegistry + subagent/agent_message tools + multi-thread runtime + max_depth |
| **v1.2 Bench** | ✅ complete | CLI subcommands ✅, 8/8 seed tasks ✅, 26 tests ✅, parallel ✅, diff ✅ |
| **Phase 7** (v2) | ✅ complete | compaction ✅ (13 tests); Session resume ✅; Session↔scratchpad ✅ (2 new tests); plugin v2 ✅ (clippy fixed) |

## Watch List

- Phase 5 revisit: mock HTTP server for `complete()` coverage
- cargo audit (network unavailable)

---
---
