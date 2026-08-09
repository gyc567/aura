//! 文件类工具集成测试（Phase 5 覆盖率补齐）：`read_file` / `write_file` /
//! `run_command` / `list_dir` / `grep_files` / `find_files`。

use aura::tools::{
    find_files::FindFilesTool, grep_files::GrepFilesTool, list_dir::ListDirTool,
    read_file::ReadFileTool, run_command::RunCommandTool, write_file::WriteFileTool,
};
use aura::{Tool, ToolArgument, ToolContext, ToolInput};
use serde_json::json;

fn ctx(ws: &std::path::Path) -> ToolContext {
    // macOS 上 /var → /private/var 符号链接：工具内部会 canonicalize，
    // 这里先规范化 workspace 避免路径策略误判 escapes workspace。
    let ws = ws.canonicalize().unwrap_or_else(|_| ws.to_path_buf());
    ToolContext::new(ws, "c1")
}

/// 在临时 workspace 里准备一个小项目。
fn scaffold(ws: &std::path::Path) {
    std::fs::create_dir_all(ws.join("src")).unwrap();
    std::fs::write(ws.join("README.md"), "# Demo\n\nHello world.\n").unwrap();
    std::fs::write(ws.join("src/main.rs"), "fn main() { println!(\"hi\"); }\n").unwrap();
    std::fs::write(
        ws.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }\n",
    )
    .unwrap();
}

// ── read_file ──────────────────────────────────────────────────────────────

#[test]
fn read_file_existing() {
    let temp = tempfile::TempDir::new().unwrap();
    scaffold(temp.path());
    let tool = ReadFileTool::new();
    let args = ToolArgument::new(json!({ "path": "README.md" }));
    let out = tool
        .execute(ToolInput::new(args), &ctx(temp.path()))
        .unwrap();
    assert!(out.success);
    assert!(out.content.contains("Hello world"));
}

#[test]
fn read_file_missing_is_error() {
    let temp = tempfile::TempDir::new().unwrap();
    scaffold(temp.path());
    let tool = ReadFileTool::new();
    let args = ToolArgument::new(json!({ "path": "nope.txt" }));
    let out = tool.execute(ToolInput::new(args), &ctx(temp.path()));
    assert!(out.is_err(), "missing file must error");
}

#[test]
fn read_file_missing_path_argument() {
    let temp = tempfile::TempDir::new().unwrap();
    let tool = ReadFileTool::new();
    let args = ToolArgument::new(json!({}));
    assert!(
        tool.execute(ToolInput::new(args), &ctx(temp.path()))
            .is_err()
    );
}

// ── write_file ─────────────────────────────────────────────────────────────

#[test]
fn write_file_creates_and_reads_back() {
    let temp = tempfile::TempDir::new().unwrap();
    scaffold(temp.path());
    let tool = WriteFileTool::new();
    let args = ToolArgument::new(json!({ "path": "notes.txt", "content": "line one\nline two" }));
    let out = tool
        .execute(ToolInput::new(args), &ctx(temp.path()))
        .unwrap();
    assert!(out.success);
    let content = std::fs::read_to_string(temp.path().join("notes.txt")).unwrap();
    assert_eq!(content, "line one\nline two");
}

#[test]
fn write_file_creates_parent_dirs() {
    let temp = tempfile::TempDir::new().unwrap();
    let tool = WriteFileTool::new();
    let args = ToolArgument::new(json!({ "path": "a/b/c.txt", "content": "x" }));
    tool.execute(ToolInput::new(args), &ctx(temp.path()))
        .unwrap();
    assert!(temp.path().join("a/b/c.txt").exists());
}

#[test]
fn write_file_missing_content_argument() {
    let temp = tempfile::TempDir::new().unwrap();
    let tool = WriteFileTool::new();
    let args = ToolArgument::new(json!({ "path": "x.txt" }));
    assert!(
        tool.execute(ToolInput::new(args), &ctx(temp.path()))
            .is_err()
    );
}

// ── run_command ────────────────────────────────────────────────────────────

#[test]
fn run_command_success() {
    let temp = tempfile::TempDir::new().unwrap();
    let tool = RunCommandTool::new();
    let args = ToolArgument::new(json!({ "command": "echo", "args": ["hello-run"] }));
    let out = tool
        .execute(ToolInput::new(args), &ctx(temp.path()))
        .unwrap();
    assert!(out.success);
    assert!(out.content.contains("hello-run"));
}

#[test]
fn run_command_failure_reported() {
    let temp = tempfile::TempDir::new().unwrap();
    let tool = RunCommandTool::new();
    let args = ToolArgument::new(json!({ "command": "sh", "args": ["-c", "exit 3"] }));
    let out = tool
        .execute(ToolInput::new(args), &ctx(temp.path()))
        .unwrap();
    assert!(!out.success, "non-zero exit must be reported as failure");
}

// ── list_dir ───────────────────────────────────────────────────────────────

#[test]
fn list_dir_lists_entries() {
    let temp = tempfile::TempDir::new().unwrap();
    scaffold(temp.path());
    let tool = ListDirTool::new();
    let args = ToolArgument::new(json!({ "path": "." }));
    let out = tool
        .execute(ToolInput::new(args), &ctx(temp.path()))
        .unwrap();
    assert!(out.success);
    assert!(out.content.contains("README.md"));
    assert!(out.content.contains("src"));
}

#[test]
fn list_dir_missing_dir_errors() {
    let temp = tempfile::TempDir::new().unwrap();
    let tool = ListDirTool::new();
    let args = ToolArgument::new(json!({ "path": "does-not-exist" }));
    assert!(
        tool.execute(ToolInput::new(args), &ctx(temp.path()))
            .is_err()
    );
}

// ── grep_files ─────────────────────────────────────────────────────────────

#[test]
fn grep_files_substring_match() {
    let temp = tempfile::TempDir::new().unwrap();
    scaffold(temp.path());
    let tool = GrepFilesTool::new();
    let args = ToolArgument::new(json!({ "path": "src", "pattern": "add" }));
    let out = tool
        .execute(ToolInput::new(args), &ctx(temp.path()))
        .unwrap();
    assert!(out.success);
    assert!(out.content.contains("lib.rs"), "got: {}", out.content);
}

#[test]
fn grep_files_no_match_reports_empty() {
    let temp = tempfile::TempDir::new().unwrap();
    scaffold(temp.path());
    let tool = GrepFilesTool::new();
    let args = ToolArgument::new(json!({ "path": "src", "pattern": "zzz-no-such" }));
    let out = tool
        .execute(ToolInput::new(args), &ctx(temp.path()))
        .unwrap();
    assert!(out.success);
    assert!(out.content.contains("no match") || !out.content.contains("lib.rs"));
}

// ── find_files ─────────────────────────────────────────────────────────────

#[test]
fn find_files_by_name_substring() {
    let temp = tempfile::TempDir::new().unwrap();
    scaffold(temp.path());
    let tool = FindFilesTool::new();
    let args = ToolArgument::new(json!({ "path": ".", "pattern": "README" }));
    let out = tool
        .execute(ToolInput::new(args), &ctx(temp.path()))
        .unwrap();
    assert!(out.success);
    assert!(out.content.contains("README.md"));
}

#[test]
fn find_files_no_match() {
    let temp = tempfile::TempDir::new().unwrap();
    scaffold(temp.path());
    let tool = FindFilesTool::new();
    let args = ToolArgument::new(json!({ "path": ".", "pattern": "zzz" }));
    let out = tool
        .execute(ToolInput::new(args), &ctx(temp.path()))
        .unwrap();
    assert!(out.success);
}
