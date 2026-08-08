//! CLI 集成测试（`assert_cmd`）。
//!
//! 覆盖：--help、--version、参数校验、成功运行、JSON 输出、失败退出码。

use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;

/// 查找 `aura-cli` 二进制（cargo build 后位于 `target/debug/`）。
fn aura() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_aura-cli"));
    cmd.current_dir("/tmp"); // 任意存在目录
    cmd
}

#[test]
fn help_flag_returns_success() {
    aura()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("aura"));
}

#[test]
fn version_flag_returns_success() {
    aura()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("aura"));
}

#[test]
fn missing_instruction_fails() {
    aura()
        .arg("--workspace")
        .arg("/tmp")
        .assert()
        .failure()
        .stderr(predicate::str::contains("INSTRUCTION"));
}

#[test]
fn nonexistent_workspace_fails() {
    aura()
        .arg("--workspace")
        .arg("/this/path/does/not/exist")
        .arg("do thing")
        .assert()
        .failure();
}

#[test]
fn fake_model_run_returns_success() {
    aura()
        .arg("--workspace")
        .arg("/tmp")
        .arg("--fake-model")
        .arg("--max-turns")
        .arg("5")
        .arg("plan the work")
        .assert()
        .success()
        .stdout(predicate::str::contains("status:      OK"));
}

#[test]
fn json_output_contains_schema_field() {
    aura()
        .arg("--workspace")
        .arg("/tmp")
        .arg("--fake-model")
        .arg("--json")
        .arg("plan the work")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"schema\": \"aura.report.v1\""));
}

#[test]
fn json_output_contains_status_field() {
    aura()
        .arg("--workspace")
        .arg("/tmp")
        .arg("--fake-model")
        .arg("--json")
        .arg("plan")
        .assert()
        .success()
        .stdout(predicate::str::contains("\"status\": \"ok\""));
}

#[test]
fn unknown_tool_fails() {
    aura()
        .arg("--workspace")
        .arg("/tmp")
        .arg("--fake-model")
        .arg("--tools")
        .arg("read_file,todo_write")
        .arg("plan")
        .assert()
        .failure()
        .stderr(predicate::str::contains("read_file"));
}

#[test]
fn default_policy_is_balanced() {
    // 默认 balanced policy 应允许执行；fake 模型返回 done；exit 0
    aura()
        .arg("--workspace")
        .arg("/tmp")
        .arg("--fake-model")
        .arg("--policy")
        .arg("balanced")
        .arg("plan")
        .assert()
        .success();
}

#[test]
fn strict_policy_flag_accepted() {
    // strict policy 应被解析并执行（v1 fake 脚本不走命令，不被策略拦）
    aura()
        .arg("--workspace")
        .arg("/tmp")
        .arg("--fake-model")
        .arg("--policy")
        .arg("strict")
        .arg("plan")
        .assert()
        .success();
}

#[test]
fn invalid_policy_fails() {
    aura()
        .arg("--workspace")
        .arg("/tmp")
        .arg("--policy")
        .arg("bogus")
        .arg("plan")
        .assert()
        .failure();
}

#[test]
fn max_turns_zero_fails() {
    aura()
        .arg("--workspace")
        .arg("/tmp")
        .arg("--max-turns")
        .arg("0")
        .arg("plan")
        .assert()
        .failure();
}

#[test]
fn text_report_includes_summary() {
    aura()
        .arg("--workspace")
        .arg("/tmp")
        .arg("--fake-model")
        .arg("plan")
        .assert()
        .success()
        .stdout(predicate::str::contains("summary:"));
}
