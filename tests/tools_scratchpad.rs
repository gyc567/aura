//! `scratchpad` 工具集成测试。

use aura::tools::scratchpad::ScratchpadTool;
use aura::{Tool, ToolArgument, ToolContext, ToolInput};
use serde_json::json;

#[test]
fn test_scratchpad_set_and_get() {
    let temp = tempfile::TempDir::new().unwrap();
    let tool = ScratchpadTool::new(temp.path().to_path_buf());
    let ctx = ToolContext::new("/tmp".into(), "c1");

    let args = ToolArgument::new(json!({
        "action": "set",
        "name": "foo",
        "value": "hello world"
    }));
    let out = tool.execute(ToolInput::new(args), &ctx).unwrap();
    assert!(out.success);

    let args = ToolArgument::new(json!({
        "action": "get",
        "name": "foo"
    }));
    let out = tool.execute(ToolInput::new(args), &ctx).unwrap();
    assert_eq!(out.content, "hello world");
}

#[test]
fn test_scratchpad_append() {
    let temp = tempfile::TempDir::new().unwrap();
    let tool = ScratchpadTool::new(temp.path().to_path_buf());
    let ctx = ToolContext::new("/tmp".into(), "c1");

    // set initial
    let args = ToolArgument::new(json!({
        "action": "set",
        "name": "notes",
        "value": "first line"
    }));
    tool.execute(ToolInput::new(args), &ctx).unwrap();

    // append second
    let args = ToolArgument::new(json!({
        "action": "append",
        "name": "notes",
        "value": "second line"
    }));
    let out = tool.execute(ToolInput::new(args), &ctx).unwrap();
    assert!(out.success);

    // verify combined
    let args = ToolArgument::new(json!({
        "action": "get",
        "name": "notes"
    }));
    let out = tool.execute(ToolInput::new(args), &ctx).unwrap();
    assert_eq!(out.content, "first line\nsecond line");
}

#[test]
fn test_scratchpad_list() {
    let temp = tempfile::TempDir::new().unwrap();
    let tool = ScratchpadTool::new(temp.path().to_path_buf());
    let ctx = ToolContext::new("/tmp".into(), "c1");

    for (n, v) in [("key_a", "val1"), ("key_b", "val2")] {
        let args = ToolArgument::new(json!({
            "action": "set", "name": n, "value": v
        }));
        tool.execute(ToolInput::new(args), &ctx).unwrap();
    }

    let args = ToolArgument::new(json!({"action": "list"}));
    let out = tool.execute(ToolInput::new(args), &ctx).unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&out.content).unwrap();
    assert_eq!(parsed.len(), 2);
    // each entry has name, bytes, updated_at
    for entry in &parsed {
        assert!(entry.get("name").is_some());
        assert!(entry.get("bytes").is_some());
        assert!(entry.get("updated_at").is_some());
    }
}

#[test]
fn test_scratchpad_clear() {
    let temp = tempfile::TempDir::new().unwrap();
    let tool = ScratchpadTool::new(temp.path().to_path_buf());
    let ctx = ToolContext::new("/tmp".into(), "c1");

    for (n, v) in [("x", "1"), ("y", "2"), ("z", "3")] {
        let args = ToolArgument::new(json!({
            "action": "set", "name": n, "value": v
        }));
        tool.execute(ToolInput::new(args), &ctx).unwrap();
    }

    let args = ToolArgument::new(json!({"action": "clear"}));
    let out = tool.execute(ToolInput::new(args), &ctx).unwrap();
    assert!(out.content.contains("3 entries cleared"));

    let args = ToolArgument::new(json!({"action": "list"}));
    let out = tool.execute(ToolInput::new(args), &ctx).unwrap();
    let parsed: Vec<serde_json::Value> = serde_json::from_str(&out.content).unwrap();
    assert!(parsed.is_empty());
}

#[test]
fn test_scratchpad_get_nonexistent() {
    let temp = tempfile::TempDir::new().unwrap();
    let tool = ScratchpadTool::new(temp.path().to_path_buf());
    let ctx = ToolContext::new("/tmp".into(), "c1");

    let args = ToolArgument::new(json!({
        "action": "get",
        "name": "this_key_does_not_exist"
    }));
    let err = tool.execute(ToolInput::new(args), &ctx).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("key not found") || msg.contains("this_key_does_not_exist"));
}

#[test]
fn test_scratchpad_append_nonexistent() {
    let temp = tempfile::TempDir::new().unwrap();
    let tool = ScratchpadTool::new(temp.path().to_path_buf());
    let ctx = ToolContext::new("/tmp".into(), "c1");

    let args = ToolArgument::new(json!({
        "action": "append",
        "name": "ghost_key",
        "value": "nope"
    }));
    let err = tool.execute(ToolInput::new(args), &ctx).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("key not found") || msg.contains("ghost_key"));
}

#[test]
fn test_scratchpad_idempotent_unchanged() {
    let temp = tempfile::TempDir::new().unwrap();
    let tool = ScratchpadTool::new(temp.path().to_path_buf());
    let ctx = ToolContext::new("/tmp".into(), "c1");

    // set once
    let args = ToolArgument::new(json!({
        "action": "set", "name": "steady", "value": "same"
    }));
    tool.execute(ToolInput::new(args), &ctx).unwrap();

    // same value with idempotent=true → "unchanged"
    let args = ToolArgument::new(json!({
        "action": "set", "name": "steady", "value": "same", "idempotent": true
    }));
    let out = tool.execute(ToolInput::new(args), &ctx).unwrap();
    assert_eq!(out.content, "unchanged");

    // different value → overwrites
    let args = ToolArgument::new(json!({
        "action": "set", "name": "steady", "value": "changed", "idempotent": true
    }));
    let out = tool.execute(ToolInput::new(args), &ctx).unwrap();
    assert_eq!(out.content, "set `steady`");

    let args = ToolArgument::new(json!({"action": "get", "name": "steady"}));
    let out = tool.execute(ToolInput::new(args), &ctx).unwrap();
    assert_eq!(out.content, "changed");
}

#[test]
fn test_scratchpad_works_via_dyn_tool() {
    // verify it works when called through `&dyn Tool`
    let temp = tempfile::TempDir::new().unwrap();
    let tool: Box<dyn Tool> = Box::new(ScratchpadTool::new(temp.path().to_path_buf()));
    let ctx = ToolContext::new("/tmp".into(), "c1");

    let args = ToolArgument::new(json!({
        "action": "set", "name": "dyn_test", "value": "via_trait"
    }));
    let out = tool.execute(ToolInput::new(args), &ctx).unwrap();
    assert!(out.success);
    assert_eq!(tool.name(), "scratchpad");
}

#[test]
fn test_scratchpad_summary_no_file() {
    let temp = tempfile::TempDir::new().unwrap();
    // No scratchpad.json exists yet
    let summary = ScratchpadTool::summary(temp.path());
    assert!(summary.is_none());
}

#[test]
fn test_scratchpad_summary_empty_after_clear() {
    let temp = tempfile::TempDir::new().unwrap();
    let workspace = temp.path().to_path_buf();
    let tool = ScratchpadTool::new(workspace.clone());
    let ctx = ToolContext::new("/tmp".into(), "c1");
    // Clear to get empty state
    let args = ToolArgument::new(json!({"action": "clear"}));
    tool.execute(ToolInput::new(args), &ctx).unwrap();

    let summary = ScratchpadTool::summary(&workspace);
    assert!(summary.is_none());
}

#[test]
fn test_scratchpad_summary_with_entries() {
    let temp = tempfile::TempDir::new().unwrap();
    let workspace = temp.path().to_path_buf();
    let tool = ScratchpadTool::new(workspace.clone());
    let ctx = ToolContext::new("/tmp".into(), "c1");

    let args = ToolArgument::new(json!({
        "action": "set",
        "name": "file_a",
        "value": "a".repeat(200)
    }));
    tool.execute(ToolInput::new(args), &ctx).unwrap();

    let args = ToolArgument::new(json!({
        "action": "set",
        "name": "file_b",
        "value": "b".repeat(150)
    }));
    tool.execute(ToolInput::new(args), &ctx).unwrap();

    let summary = ScratchpadTool::summary(&workspace);
    assert!(summary.is_some());
    let s = summary.unwrap();
    assert!(s.contains("file_a: 200B"));
    assert!(s.contains("file_b: 150B"));
}

#[test]
fn test_scratchpad_summary_reads_from_disk() {
    // Verify summary reads from disk, not just memory
    let temp = tempfile::TempDir::new().unwrap();
    let workspace = temp.path().to_path_buf();

    // Create a scratchpad tool, write data, then drop it
    {
        let tool = ScratchpadTool::new(workspace.clone());
        let ctx = ToolContext::new("/tmp".into(), "c1");
        let args = ToolArgument::new(json!({
            "action": "set",
            "name": "persistent",
            "value": "hello world"
        }));
        tool.execute(ToolInput::new(args), &ctx).unwrap();
    }

    // Create a NEW tool instance (forces re-read from disk)
    let tool2 = ScratchpadTool::new(workspace.clone());
    let summary = ScratchpadTool::summary(&workspace);
    assert!(summary.is_some());
    assert!(summary.unwrap().contains("persistent: 11B"));

    // Also verify the new tool sees the data
    let ctx = ToolContext::new("/tmp".into(), "c2");
    let args = ToolArgument::new(json!({"action": "get", "name": "persistent"}));
    let out = tool2.execute(ToolInput::new(args), &ctx).unwrap();
    assert_eq!(out.content, "hello world");
}

/// M2：两个独立 store 实例（主 agent / subagent）并发写不同 key，
/// persist 合并后两个 key 都应保留。
#[test]
fn test_scratchpad_concurrent_stores_merge_on_persist() {
    let temp = tempfile::TempDir::new().unwrap();
    let path = temp.path().to_path_buf();
    let tool_a = ScratchpadTool::new(path.clone());
    let tool_b = ScratchpadTool::new(path.clone());
    let ctx = ToolContext::new("/tmp".into(), "c1");

    let set = |tool: &ScratchpadTool, name: &str, value: &str| {
        let args = ToolArgument::new(json!({ "action": "set", "name": name, "value": value }));
        tool.execute(ToolInput::new(args), &ctx).unwrap();
    };
    set(&tool_a, "k1", "v1");
    set(&tool_b, "k2", "v2"); // B 的内存副本不含 k1；persist 应合并保留

    let tool_c = ScratchpadTool::new(path);
    let args = ToolArgument::new(json!({ "action": "list" }));
    let out = tool_c.execute(ToolInput::new(args), &ctx).unwrap();
    assert!(
        out.content.contains("k1") && out.content.contains("k2"),
        "both keys must survive, got: {}",
        out.content
    );
}

/// 低危项：损坏的 scratchpad.json 应改名保留（.corrupt），不静默覆盖丢失。
#[test]
fn test_scratchpad_corrupt_file_is_backed_up() {
    let temp = tempfile::TempDir::new().unwrap();
    let dir = temp.path().join("artifacts");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("scratchpad.json");
    std::fs::write(&path, "not [ valid json").unwrap();

    let tool = ScratchpadTool::new(temp.path().to_path_buf());
    let ctx = ToolContext::new("/tmp".into(), "c1");
    let args = ToolArgument::new(json!({ "action": "set", "name": "ok", "value": "1" }));
    tool.execute(ToolInput::new(args), &ctx).unwrap();

    assert!(
        path.with_extension("json.corrupt").exists(),
        "corrupt file backed up"
    );
    let args = ToolArgument::new(json!({ "action": "get", "name": "ok" }));
    let out = tool.execute(ToolInput::new(args), &ctx).unwrap();
    assert_eq!(out.content, "1");
}
