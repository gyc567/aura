# Bench Task Authoring Guide

## Task Spec Format (YAML)

Each task is a YAML file in `bench/tasks/`. Key fields:

| Field | Required | Description |
|-------|----------|-------------|
| `id` | ✅ | Unique kebab-case identifier |
| `name` | ✅ | Human-readable name |
| `description` | ❌ | Detailed description |
| `difficulty` | ✅ | `easy` | `medium` | `hard` |
| `category` | ✅ | `testing` | `bugfix` | `refactor` | `feature` | `docs` | `infra` |
| `skills` | ❌ | Required skills (e.g. `["rust", "cargo"]`) |
| `setup` | ❌ | List of setup actions |
| `instruction` | ✅ | Natural language instruction for the agent |
| `verify` | ✅ | Verification spec |
| `reference` | ❌ | Reference solution |
| `tags` | ❌ | List of tags |

## Verify Types

| `type` | Fields | Success condition |
|--------|--------|-------------------|
| `command` | `command`, `cwd`, `timeout_seconds` | Exit code 0 |
| `file_exists` | `path` | File exists and is non-empty |
| `cargo_test` | `timeout_seconds` | `cargo test --quiet` exits 0 |
| `cargo_fmt` | — | `cargo fmt --check` passes |
| `git_diff` | `pattern` | Git diff matches pattern |

## Setup Actions

| `action` | Fields |
|----------|--------|
| `write` | `path`, `content` |
| `mkdir` | `path` |
| `copy` | `from`, `to` |
| `clone` | `repo`, `depth` (default: 1) |

## Template

```yaml
id: my-task
name: "My Task"
description: |
  What this task tests.
difficulty: easy
category: feature
skills: ["rust"]
setup:
  - action: write
    path: src/main.rs
    content: |
      fn main() {}
instruction: |
  Your task here.
verify:
  type: command
  command: "cargo build --quiet"
  cwd: "${AURA_WORKSPACE}"
  timeout_seconds: 30
tags:
  - beginner
```
