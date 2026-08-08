# Aura Code Audit Report

**Date**: 2026-08-08
**Reviewer**: loop-audit
**Scope**: All source files modified Aug 7 (~154 tests passing, 0 clippy warnings)

---

## Summary

| Dimension | Rating | Notes |
|-----------|--------|-------|
| Correctness | P0: 2 runtime panic risks | FakeModel empty queue, silent fallback |
| Safety | Good | Precheck regex, policy gates, sensitive-path deny-list |
| Design | P1: 3 issues | Silent model fallback, runtime-per-call, unwired Policy |
| Coverage | Strong | 154 tests across 17 test suites |
| Style | Clean | `#![deny(rust_2018_idioms]`, pedantic clippy |

---

## P0 — Must Fix

### 1. FakeModel::complete panics on empty queue

**File**: `src/main.rs:182`
**Severity**: Runtime panic (user-facing crash)

```rust
fn complete(&self, ...) -> ... {
    let next = self.queue.lock().unwrap().remove(0); // panics if empty
}
```

**Impact**: If `FakeModel` is used with a queue that gets exhausted (e.g., misconfigured test or `--fake-model` with wrong script), the CLI panics with `index out of bounds`.

**Fix**: Replace `remove(0)` with `.pop(0)` + `expect("fake model queue exhausted")` or return a proper error.

---

### 2. Silent fallback to FakeModel without user notification

**File**: `src/main.rs:142-143`
**Severity**: Silent misconfiguration mask

```rust
// 缺 endpoint/model/api_key → fallback 到 fake，避免启动失败
ModelChoice::Fake(build_fake_model())
```

**Impact**: User forgets `--endpoint` or `--api-key`, or has a typo — the agent runs silently in fake mode with no output, producing confusing "success" with no actual agentic behavior.

**Fix**: Emit a warning to `stderr`:

```rust
eprintln!("warning: no API key/endpoint provided, running in fake mode");
```

---

## P1 — Should Fix

### 3. futures_block_on creates a new runtime per call

**File**: `src/main.rs:195-200`

```rust
fn futures_block_on<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio build")
        .block_on(fut) // new runtime spawned every call
}
```

**Impact**: Each call allocates a new runtime (heap + threads). Called from `spawn_sigint_handler` and possibly other places. Wasteful but not incorrect.

**Fix**: Create a single `current_thread` runtime at startup and reuse it.

---

### 4. TodoWriteTool::current clones entire state

**File**: `src/tools/todo_write.rs:35-36`

```rust
pub fn current(&self) -> Vec<TodoItem> {
    self.state.lock().unwrap().clone() // clones full vec on every call
}
```

**Impact**: O(n) clone every inspection. In v1 this is likely called infrequently, but `Clone` on `Vec<TodoItem>` is not cheap if lists grow.

**Fix**: Return `&[TodoItem]` with a guard pattern, or accept the clone for now since v1 lists are small.

---

### 5. Policy::evaluate appears unwired

**File**: `src/policy.rs:52` (struct) + `src/policy.rs:54-178` (impl)

The `Policy` struct and its `evaluate` method are defined but no callsites were found in the reviewed files. If this is the evaluation method meant to gate tool execution, it's not yet wired into `Agent::run`.

**Action**: Confirm callchain — either wire it in or document as deferred in design doc.

---

### 6. ModelChoice::into_dyn is dead code

**File**: `src/main.rs:100-106`

```rust
fn into_dyn(self) -> Box<dyn aura::ModelGateway> {
    match self { ... }
}
```

**Impact**: Method is defined but never called. Dead code.

**Fix**: Remove it, or wire it into the call path if `run` expects `Box<dyn ModelGateway>`.

---

## P2 — Observations (No Action Required)

### 7. WireChoice / WireResponseMessage / WireResponseToolCall have #[allow(dead_code)]

**File**: `src/model_http.rs:144,153,163`

These struct fields are only read via `serde` deserialization (not accessed structurally). `#[allow(dead_code)]` is correct — they're part of the wire protocol even if not read structurally.

---

### 8. exit_code_from_report collapses all non-Completed to exit code 1

**File**: `src/main.rs:235-239`

Intentional per design (§7), but worth noting for CI/scripting differentiation.

---

### 9. Precheck regex rm -rf coverage

**File**: `src/precheck.rs:58`

```rust
r"\brm\s+(-\s*[rR]\s*[fF]\b|-[rRfF]{2,}\b|--recursive\b)"
```

Covers `rm -rf`, `rm -fr`, `rm -R -f`, `rm --recursive`. `rm -r /some/path` (without `-f`) is NOT caught — correct per spec.

---

## Verified Strengths

- **154 tests passing**, 0 clippy warnings, clean `cargo build`
- **Clean separation**: domain types are pure, no IO in core, trait boundaries clear
- **SSE parser is stateless**: single-pass, no allocation, cross-packet buffering correct
- **`is_sensitive` deny-list** covers `.env`, `.pem/.key/.pfx`, `.ssh/`, `secrets/`, `credentials/`
- **`LazyLock` for regex** (Rust 1.80+): avoids once_cell dep, correctly `expect("valid regex")`
- **`Send + Sync` on all shared types**: `ToolRegistry`, `ModelGateway`, `InMemoryRegistry`
- **Graceful SIGINT**: `Arc<AtomicBool>` shared with handler, `Ordering::Relaxed` sufficient

---

## Recommended Fixes (Priority Order)

| Priority | Issue | File:Line | Estimated Fix |
|----------|-------|-----------|---------------|
| P0 | FakeModel panic on empty queue | `main.rs:182` | `remove(0)` → `pop(0).expect(...)` |
| P0 | Silent fake-model fallback | `main.rs:142-143` | Add `eprintln!("warning: ...")` |
| P1 | Runtime-per-call | `main.rs:195` | Reuse single runtime |
| P1 | Policy::evaluate unwired | `policy.rs` | Confirm callchain or defer |
| P1 | into_dyn dead code | `main.rs:100-106` | Remove or use |
| P2 | TodoWriteTool::current clone | `todo_write.rs:35` | Accept for v1 scope |
