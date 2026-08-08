//! 工具结果回执与系统提醒集成测试。

use std::path::Path;

use aura::reminders::{TodoItem, TodoPriority, TodoStatus};
use aura::{
    GLOBAL_REMINDERS, READ_ONLY_REMINDERS, RUN_COMMAND_REMINDERS, RemindedOutput, SystemReminders,
    TODO_WRITE_REMINDERS, ToolOutput, WRITE_FILE_REMINDERS,
};

#[test]
fn global_reminders_non_empty() {
    assert!(!GLOBAL_REMINDERS.is_empty());
    let joined = GLOBAL_REMINDERS.join("\n");
    assert!(joined.contains("important-instruction-reminders"));
}

#[test]
fn todo_write_reminders_specific() {
    let joined = TODO_WRITE_REMINDERS.join("\n");
    assert!(joined.to_lowercase().contains("todo"));
}

#[test]
fn write_file_reminders_specific() {
    let joined = WRITE_FILE_REMINDERS.join("\n");
    assert!(joined.to_lowercase().contains("diff"));
}

#[test]
fn run_command_reminders_specific() {
    let joined = RUN_COMMAND_REMINDERS.join("\n");
    assert!(joined.to_lowercase().contains("exit"));
}

#[test]
fn read_only_reminders_specific() {
    let joined = READ_ONLY_REMINDERS.join("\n");
    assert!(joined.to_lowercase().contains("context"));
}

#[test]
fn tool_reminders_for_known_tools() {
    assert_eq!(
        aura::reminders::tool_reminders_for("todo_write"),
        TODO_WRITE_REMINDERS
    );
    assert_eq!(
        aura::reminders::tool_reminders_for("write_file"),
        WRITE_FILE_REMINDERS
    );
    assert_eq!(
        aura::reminders::tool_reminders_for("run_command"),
        RUN_COMMAND_REMINDERS
    );
}

#[test]
fn tool_reminders_for_unknown_falls_back_to_readonly() {
    assert_eq!(
        aura::reminders::tool_reminders_for("unknown_tool"),
        READ_ONLY_REMINDERS
    );
    assert_eq!(aura::reminders::tool_reminders_for(""), READ_ONLY_REMINDERS);
}

#[test]
fn reminded_output_text_includes_all_layers() {
    let out = ToolOutput::ok("PAYLOAD");
    let reminded = RemindedOutput::wrap("c1", "todo_write", out);
    let text = reminded.to_text();
    assert!(text.contains("important-instruction-reminders"));
    assert!(text.to_lowercase().contains("todo"));
    assert!(text.ends_with("PAYLOAD"));
}

#[test]
fn reminded_output_for_read_only_tool() {
    let out = ToolOutput::ok("READ");
    let reminded = RemindedOutput::wrap("c2", "list_dir", out);
    let text = reminded.to_text();
    assert!(text.contains("important-instruction-reminders"));
    assert!(text.contains("context"));
    assert!(text.ends_with("READ"));
}

#[test]
fn reminded_output_preserves_failure_flag() {
    let out = ToolOutput::err("FAIL");
    let reminded = RemindedOutput::wrap("c3", "run_command", out);
    assert!(!reminded.output.success);
    assert!(reminded.to_text().ends_with("FAIL"));
}

#[test]
fn system_reminders_baseline_non_empty() {
    let r = SystemReminders::baseline();
    assert!(r.len() >= 2);
    assert!(r[0].contains("<system-reminder>"));
    assert!(r.last().unwrap().contains("</system-reminder>"));
}

#[test]
fn system_reminders_todo_changed_counts_pending() {
    let todos = vec![
        TodoItem {
            id: "1".into(),
            content: "a".into(),
            status: TodoStatus::Completed,
            priority: TodoPriority::High,
        },
        TodoItem {
            id: "2".into(),
            content: "b".into(),
            status: TodoStatus::InProgress,
            priority: TodoPriority::Medium,
        },
        TodoItem {
            id: "3".into(),
            content: "c".into(),
            status: TodoStatus::Pending,
            priority: TodoPriority::Low,
        },
    ];
    let r = SystemReminders::todo_changed(&todos);
    let joined = r.join("\n");
    assert!(joined.contains("2 pending"));
}

#[test]
fn system_reminders_todo_changed_handles_empty() {
    let r = SystemReminders::todo_changed(&[]);
    let joined = r.join("\n");
    assert!(joined.contains("0 pending"));
}

#[test]
fn system_reminders_todo_empty_suggest_mentions_create() {
    let r = SystemReminders::todo_empty_suggest();
    let joined = r.join("\n");
    assert!(joined.to_lowercase().contains("todo"));
}

#[test]
fn system_reminders_secret_warning_contains_path() {
    let r = SystemReminders::secret_warning(Path::new("/workspace/.env"));
    let joined = r.join("\n");
    assert!(joined.contains(".env"));
    assert!(joined.to_lowercase().contains("secret"));
}
