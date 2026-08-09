//! Bench framework integration tests.
//!
//! 覆盖：bench CLI 子命令、TaskSpec 解析、Workspace 隔离、
//! `TaskRunner` 执行、`Summary`/`Report` 生成、`BenchSuite` 加载。

use std::process::Command;

use assert_cmd::prelude::*;
use predicates::prelude::*;

/// 查找 `aura` 二进制。
fn aura() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_aura"));
    cmd.current_dir("/tmp");
    cmd
}

// === TaskSpec parsing (lib-level) ===

#[test]
fn task_spec_parse_hello_world() {
    let yaml = r#"
id: hello-world
name: "Hello World"
description: "Print hello world"
difficulty: easy
category: feature
skills: ["rust"]
setup:
  - action: mkdir
    path: src
instruction: "Create src/main.rs"
verify:
  type: command
  command: "cargo run --quiet"
  cwd: "${AURA_WORKSPACE}"
  timeout_seconds: 30
tags: ["beginner"]
"#;
    let spec = aura::bench::TaskSpec::from_str(yaml).unwrap();
    assert_eq!(spec.id, "hello-world");
    assert_eq!(spec.name, "Hello World");
    assert_eq!(spec.difficulty, aura::bench::Difficulty::Easy);
    assert_eq!(spec.category, aura::bench::Category::Feature);
    assert_eq!(spec.skills, vec!["rust".to_string()]);
    assert_eq!(spec.tags, vec!["beginner".to_string()]);
    assert!(matches!(
        spec.verify,
        aura::bench::VerifySpec::Command { .. }
    ));
}

#[test]
fn task_spec_parse_all_difficulties() {
    for (yaml, expected) in [
        ("difficulty: easy\n", aura::bench::Difficulty::Easy),
        ("difficulty: medium\n", aura::bench::Difficulty::Medium),
        ("difficulty: hard\n", aura::bench::Difficulty::Hard),
    ] {
        let full = format!(
            "id: t\nname: t\ndescription: t\ncategory: feature\ninstruction: t\nverify:\n  type: cargo_fmt\n{yaml}"
        );
        let spec = aura::bench::TaskSpec::from_str(&full).unwrap();
        assert_eq!(spec.difficulty, expected, "failed for: {yaml}");
    }
}

#[test]
fn task_spec_parse_all_categories() {
    for (yaml, expected) in [
        ("category: testing\n", aura::bench::Category::Testing),
        ("category: bugfix\n", aura::bench::Category::Bugfix),
        ("category: refactor\n", aura::bench::Category::Refactor),
        ("category: feature\n", aura::bench::Category::Feature),
        ("category: docs\n", aura::bench::Category::Docs),
        ("category: infra\n", aura::bench::Category::Infra),
    ] {
        let full = format!(
            "id: t\nname: t\ndifficulty: easy\ninstruction: t\nverify:\n  type: cargo_fmt\n{yaml}"
        );
        let spec = aura::bench::TaskSpec::from_str(&full).unwrap();
        assert_eq!(spec.category, expected, "failed for: {yaml}");
    }
}

#[test]
fn task_spec_verify_types() {
    let cmd_yaml = "type: command\ncommand: echo hi\ncwd: ${AURA_WORKSPACE}\ntimeout_seconds: 10";
    let v: aura::bench::VerifySpec = serde_yaml::from_str(cmd_yaml).unwrap();
    assert!(matches!(v, aura::bench::VerifySpec::Command { .. }));

    let fe_yaml = "type: file_exists\npath: src/main.rs";
    let v: aura::bench::VerifySpec = serde_yaml::from_str(fe_yaml).unwrap();
    assert!(matches!(v, aura::bench::VerifySpec::FileExists { .. }));

    let test_yaml = "type: cargo_test\ntimeout_seconds: 120";
    let v: aura::bench::VerifySpec = serde_yaml::from_str(test_yaml).unwrap();
    assert!(matches!(v, aura::bench::VerifySpec::CargoTest { .. }));

    let fmt_yaml = "type: cargo_fmt";
    let v: aura::bench::VerifySpec = serde_yaml::from_str(fmt_yaml).unwrap();
    assert!(matches!(v, aura::bench::VerifySpec::CargoFmt));
}

#[test]
fn task_spec_setup_actions() {
    let action: aura::bench::SetupAction =
        serde_yaml::from_str("action: write\npath: src/main.rs\ncontent: fn main() {}").unwrap();
    assert!(matches!(action, aura::bench::SetupAction::Write { .. }));

    let action: aura::bench::SetupAction =
        serde_yaml::from_str("action: mkdir\npath: tests").unwrap();
    assert!(matches!(action, aura::bench::SetupAction::Mkdir { .. }));

    let action: aura::bench::SetupAction =
        serde_yaml::from_str("action: copy\nfrom: /tmp/a\nto: b").unwrap();
    assert!(matches!(action, aura::bench::SetupAction::Copy { .. }));
}

// === Workspace tests ===

#[test]
fn workspace_creates_isolated_dir() {
    let ws = aura::bench::Workspace::new().unwrap();
    assert!(ws.root().exists());
    assert!(ws.root().to_string_lossy().contains("aura-bench-"));
}

#[test]
fn workspace_setup_write_and_read() {
    let ws = aura::bench::Workspace::new().unwrap();
    ws.setup(&[aura::bench::SetupAction::Write {
        path: "src/lib.rs".to_string(),
        content: "pub fn add(a: i32, b: i32) -> i32 { a + b }".to_string(),
    }])
    .unwrap();
    let content = std::fs::read_to_string(ws.root().join("src/lib.rs")).unwrap();
    assert_eq!(content, "pub fn add(a: i32, b: i32) -> i32 { a + b }");
}

#[test]
fn workspace_resolve_vars() {
    let ws = aura::bench::Workspace::new().unwrap();
    let resolved = ws.resolve_vars("${AURA_WORKSPACE}/src");
    assert!(resolved.starts_with(ws.root().to_str().unwrap()));
    assert!(resolved.ends_with("/src"));
}

// === Summary & Report tests ===

#[test]
fn summary_from_results_calculates_pass_rate() {
    let results = vec![
        make_result("t1", aura::bench::TaskStatus::Passed, 10.0, 2),
        make_result("t2", aura::bench::TaskStatus::Passed, 5.0, 1),
        make_result("t3", aura::bench::TaskStatus::Failed, 3.0, 1),
        make_result("t4", aura::bench::TaskStatus::Error, 1.0, 0),
    ];
    let summary = aura::bench::Summary::from_results("test-run", "test-agent", results);
    assert_eq!(summary.total, 4);
    assert_eq!(summary.passed, 2);
    assert_eq!(summary.failed, 2);
    assert!((summary.pass_rate - 0.5).abs() < 0.01);
    assert!((summary.total_wall_time_s - 19.0).abs() < 0.01);
}

#[test]
fn summary_by_category() {
    let results = vec![
        make_result("t1", aura::bench::TaskStatus::Passed, 10.0, 2),
        make_result("t2", aura::bench::TaskStatus::Failed, 5.0, 1),
    ];
    let summary = aura::bench::Summary::from_results("test", "agent", results);
    let cat = summary.by_category.get("testing").unwrap();
    assert_eq!(cat.total, 2);
    assert_eq!(cat.passed, 1);
    assert!((cat.rate - 0.5).abs() < 0.01);
}

#[test]
fn summary_by_difficulty() {
    let results = vec![
        make_result("t1", aura::bench::TaskStatus::Passed, 10.0, 2),
        make_result("t2", aura::bench::TaskStatus::Timeout, 5.0, 1),
    ];
    let summary = aura::bench::Summary::from_results("test", "agent", results);
    let diff = summary.by_difficulty.get("medium").unwrap();
    assert_eq!(diff.total, 2);
    assert_eq!(diff.passed, 1);
}

#[test]
fn text_report_contains_pass_rate() {
    let results = vec![make_result(
        "hello",
        aura::bench::TaskStatus::Passed,
        10.0,
        2,
    )];
    let summary = aura::bench::Summary::from_results("run1", "aura", results);
    let report = aura::bench::format_text_report(&summary);
    assert!(report.contains("Aura Bench Report"));
    assert!(report.contains("100.0%"));
    assert!(report.contains("1/1"));
}

#[test]
fn text_report_shows_failed_tasks() {
    let results = vec![
        make_result("hello", aura::bench::TaskStatus::Passed, 10.0, 2),
        make_result("bug", aura::bench::TaskStatus::Failed, 5.0, 1),
    ];
    let summary = aura::bench::Summary::from_results("run1", "aura", results);
    let report = aura::bench::format_text_report(&summary);
    assert!(report.contains("Failed Tasks"));
    assert!(report.contains("bug"));
}

#[test]
fn summary_to_json_round_trip() {
    let results = vec![make_result("t1", aura::bench::TaskStatus::Passed, 10.0, 2)];
    let summary = aura::bench::Summary::from_results("run1", "agent", results);
    let json = summary.to_json().unwrap();
    let parsed: aura::bench::Summary = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.total, 1);
    assert_eq!(parsed.passed, 1);
    assert_eq!(parsed.tasks[0].task_id, "t1");
}

// === CLI integration tests ===

#[test]
fn bench_help_shows_subcommands() {
    aura()
        .arg("bench")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("run"))
        .stdout(predicate::str::contains("report"))
        .stdout(predicate::str::contains("init"))
        .stdout(predicate::str::contains("list"));
}

#[test]
fn bench_list_shows_seed_tasks() {
    let mut cmd = aura();
    cmd.current_dir(env!("CARGO_MANIFEST_DIR"));
    cmd.arg("bench")
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("hello-world"))
        .stdout(predicate::str::contains("add-tests-to-lib"))
        .stdout(predicate::str::contains("fix-compile-error"))
        .stdout(predicate::str::contains("format-code"));
}

#[test]
fn bench_init_creates_task_scaffold() {
    let tmp = tempfile::tempdir().unwrap();
    let mut cmd = aura();
    cmd.current_dir(tmp.path());
    cmd.arg("bench")
        .arg("init")
        .arg("my-new-task")
        .assert()
        .success();
    let scaffold = tmp.path().join("bench/tasks/my-new-task.yaml");
    assert!(scaffold.exists());
    let content = std::fs::read_to_string(&scaffold).unwrap();
    assert!(content.contains("id: my-new-task"));
    assert!(content.contains("difficulty: easy"));
}

#[test]
fn bench_init_empty_name_fails() {
    let tmp = tempfile::tempdir().unwrap();
    let mut cmd = aura();
    cmd.current_dir(tmp.path());
    cmd.arg("bench").arg("init").arg("").assert().failure();
}

#[test]
fn bench_report_requires_existing_dir() {
    let mut cmd = aura();
    cmd.current_dir("/tmp");
    cmd.arg("bench")
        .arg("report")
        .arg("/nonexistent/path")
        .assert()
        .failure()
        .stderr(predicate::str::contains("aura error"));
}

// === Helpers ===

#[test]
fn bench_run_with_simple_agent_creates_file() {
    // Create a temp task that writes a file via setup, verify with file_exists
    let tmp = tempfile::tempdir().unwrap();
    let tasks_dir = tmp.path().join("bench/tasks");
    std::fs::create_dir_all(&tasks_dir).unwrap();

    let task_yaml = r"id: write-file
name: Write File
difficulty: easy
category: feature
instruction: Write output.txt
setup:
  - action: write
    path: output.txt
    content: done
verify:
  type: file_exists
  path: output.txt
tags:
  - test
";
    std::fs::write(tasks_dir.join("write-file.yaml"), task_yaml).unwrap();

    // `true` as agent: setup already created the file, so verify passes
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_aura"));
    cmd.current_dir(tmp.path());
    cmd.args(["bench", "run", "--agent", "true"]);
    let output = cmd.output().unwrap();
    assert!(
        output.status.success(),
        "bench run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!(
        "{stdout}
{stderr}"
    );
    assert!(
        combined.contains("100.0%") || combined.contains("1/1"),
        "output: {combined}"
    );
}

#[test]
fn bench_run_output_dir_saves_summary() {
    let tmp = tempfile::tempdir().unwrap();
    let tasks_dir = tmp.path().join("bench/tasks");
    std::fs::create_dir_all(&tasks_dir).unwrap();

    let task_yaml = r"id: write-file
name: Write File
difficulty: easy
category: feature
instruction: Write output.txt
setup:
  - action: write
    path: output.txt
    content: done
verify:
  type: file_exists
  path: output.txt
tags:
  - test
";
    std::fs::write(tasks_dir.join("write-file.yaml"), task_yaml).unwrap();

    let out_dir = tmp.path().join("results");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_aura"));
    cmd.current_dir(tmp.path());
    cmd.args([
        "bench",
        "run",
        "--agent",
        "true",
        "--output",
        out_dir.to_str().unwrap(),
    ]);
    let output = cmd.output().unwrap();
    assert!(output.status.success());

    // Verify summary.json was saved
    let summary_path = out_dir.join("summary.json");
    assert!(
        summary_path.exists(),
        "summary.json should be saved to output dir"
    );
    let content = std::fs::read_to_string(&summary_path).unwrap();
    assert!(content.contains("total"));
    assert!(content.contains("passed"));
}

#[test]
fn bench_run_no_tasks_fails() {
    let tmp = tempfile::tempdir().unwrap();
    // No bench/tasks directory — should fail
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_aura"));
    cmd.current_dir(tmp.path());
    cmd.args(["bench", "run"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("aura error"));
}

fn make_result(
    id: &str,
    status: aura::bench::TaskStatus,
    wall: f64,
    turns: u32,
) -> aura::bench::TaskResult {
    aura::bench::TaskResult {
        task_id: id.to_string(),
        task_name: id.to_string(),
        difficulty: aura::bench::Difficulty::Medium.to_string(),
        category: aura::bench::Category::Testing.to_string(),
        status,
        verify_exit_code: Some(0),
        agent_wall_time_s: wall,
        agent_turns: turns,
        error: None,
        workspace_snapshot: None,
    }
}
#[test]
fn diff_report_compares_summaries() {
    let base_results = vec![
        make_result("t1", aura::bench::TaskStatus::Passed, 10.0, 2),
        make_result("t2", aura::bench::TaskStatus::Failed, 5.0, 1),
    ];
    let base = aura::bench::Summary::from_results("base", "agent", base_results);

    let cur_results = vec![
        make_result("t1", aura::bench::TaskStatus::Passed, 8.0, 2),
        make_result("t2", aura::bench::TaskStatus::Passed, 5.0, 1),
    ];
    let cur = aura::bench::Summary::from_results("cur", "agent", cur_results);

    let report = aura::bench::format_diff_report(&base, &cur);
    assert!(report.contains("Aura Bench Diff Report"));
    assert!(report.contains("t2"));
    assert!(report.contains("2 task(s) changed"));
}

#[test]
fn diff_report_no_changes() {
    let results = vec![make_result("t1", aura::bench::TaskStatus::Passed, 10.0, 2)];
    let base = aura::bench::Summary::from_results("base", "agent", results.clone());
    let cur = aura::bench::Summary::from_results("cur", "agent", results);

    let report = aura::bench::format_diff_report(&base, &cur);
    assert!(report.contains("No changes detected"));
}

#[test]
fn bench_diff_compares_two_runs() {
    let exe = env!("CARGO_BIN_EXE_aura");
    let tmp = tempfile::tempdir().unwrap();

    let base_dir = tmp.path().join("base");
    let cur_dir = tmp.path().join("current");
    std::fs::create_dir_all(&base_dir).unwrap();
    std::fs::create_dir_all(&cur_dir).unwrap();

    let base_results = vec![
        make_result("t1", aura::bench::TaskStatus::Passed, 10.0, 2),
        make_result("t2", aura::bench::TaskStatus::Passed, 5.0, 1),
    ];
    let base = aura::bench::Summary::from_results("base-run", "test-agent", base_results);
    std::fs::write(
        base_dir.join("summary.json"),
        serde_json::to_string_pretty(&base).unwrap(),
    )
    .unwrap();

    let cur_results = vec![
        make_result("t1", aura::bench::TaskStatus::Passed, 8.0, 2),
        make_result("t2", aura::bench::TaskStatus::Failed, 5.0, 1),
    ];
    let cur = aura::bench::Summary::from_results("cur-run", "test-agent", cur_results);
    std::fs::write(
        cur_dir.join("summary.json"),
        serde_json::to_string_pretty(&cur).unwrap(),
    )
    .unwrap();

    let out = Command::new(exe)
        .args([
            "bench",
            "diff",
            base_dir.to_str().unwrap(),
            cur_dir.to_str().unwrap(),
        ])
        .output()
        .expect("run bench diff");
    assert!(out.status.success(), "bench diff should succeed");
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.contains("Aura Bench Diff Report"));
    assert!(stdout.contains("t2"));
}

#[test]
fn bench_diff_requires_two_dirs() {
    let exe = env!("CARGO_BIN_EXE_aura");
    let tmp = tempfile::tempdir().unwrap();
    let missing = tmp.path().join("nonexistent");

    let out = Command::new(exe)
        .args(["bench", "diff", missing.to_str().unwrap(), "/tmp"])
        .output()
        .expect("run bench diff");
    assert!(
        !out.status.success(),
        "bench diff should fail on missing dir"
    );
}
