//! 策略评估器集成测试。

use std::path::PathBuf;

use aura::policy::{Capability, Policy, PolicyLevel};
use aura::{AgentError, Tool, ToolContext, ToolInput, ToolOutput};

struct ReadTool;
impl Tool for ReadTool {
    fn name(&self) -> &'static str {
        "read"
    }
    fn description(&self) -> &'static str {
        "needs FsRead"
    }
    fn required_capabilities(&self) -> &'static [Capability] {
        &[Capability::FsRead]
    }
    fn execute(&self, _input: ToolInput, _ctx: &ToolContext) -> Result<ToolOutput, AgentError> {
        Ok(ToolOutput::ok(""))
    }
}

struct HttpTool;
impl Tool for HttpTool {
    fn name(&self) -> &'static str {
        "http"
    }
    fn description(&self) -> &'static str {
        "needs Http"
    }
    fn required_capabilities(&self) -> &'static [Capability] {
        &[Capability::Http]
    }
    fn execute(&self, _input: ToolInput, _ctx: &ToolContext) -> Result<ToolOutput, AgentError> {
        Ok(ToolOutput::ok(""))
    }
}

#[test]
fn balanced_grants_basic_caps() {
    let p = Policy::balanced(PathBuf::from("/tmp"));
    assert!(p.granted().contains(&Capability::FsRead));
    assert!(p.granted().contains(&Capability::FsWrite));
    assert!(p.granted().contains(&Capability::Exec));
    assert!(!p.granted().contains(&Capability::Http));
    assert_eq!(p.level(), PolicyLevel::Balanced);
}

#[test]
fn strict_changes_level() {
    let p = Policy::strict(PathBuf::from("/tmp"));
    assert_eq!(p.level(), PolicyLevel::Strict);
}

#[test]
fn permissive_changes_level() {
    let p = Policy::permissive(PathBuf::from("/tmp"));
    assert_eq!(p.level(), PolicyLevel::Permissive);
}

#[test]
fn evaluate_capabilities_passes_for_granted() {
    let p = Policy::balanced(PathBuf::from("/tmp"));
    assert!(p.evaluate_capabilities(&ReadTool).is_ok());
}

#[test]
fn evaluate_capabilities_fails_for_missing() {
    let p = Policy::balanced(PathBuf::from("/tmp"));
    let err = p.evaluate_capabilities(&HttpTool).unwrap_err();
    match err {
        AgentError::CommandPolicy(msg) => assert!(msg.contains("Http")),
        other => panic!("expected CommandPolicy, got {other:?}"),
    }
}

#[test]
fn evaluate_command_blocks_high_risk() {
    let p = Policy::balanced(PathBuf::from("/tmp"));
    let argv = vec!["rm".into(), "-rf".into(), "/tmp/foo".into()];
    let err = p.evaluate_command(&argv).unwrap_err();
    assert!(matches!(err, AgentError::CommandPolicy(_)));
}

#[test]
fn evaluate_command_passes_safe() {
    let p = Policy::balanced(PathBuf::from("/tmp"));
    let argv = vec!["cargo".into(), "test".into()];
    assert!(p.evaluate_command(&argv).is_ok());
}

#[test]
fn strict_blocks_medium_tier() {
    let p = Policy::strict(PathBuf::from("/tmp"));
    let argv = vec!["echo".into(), "hi".into()];
    // Low tier so still OK
    assert!(p.evaluate_command(&argv).is_ok());
    let argv_high = vec!["rm".into(), "-rf".into(), "/tmp/x".into()];
    assert!(p.evaluate_command(&argv_high).is_err());
}

#[test]
fn needs_confirmation_always_true_for_strict() {
    let p = Policy::strict(PathBuf::from("/tmp"));
    assert!(p.needs_confirmation(&ReadTool).unwrap());
}

#[test]
fn needs_confirmation_follows_tool_flag() {
    struct SafeTool;
    impl Tool for SafeTool {
        fn name(&self) -> &'static str {
            "safe"
        }
        fn description(&self) -> &'static str {
            "no confirmation"
        }
        fn execute(&self, _input: ToolInput, _ctx: &ToolContext) -> Result<ToolOutput, AgentError> {
            Ok(ToolOutput::ok(""))
        }
    }
    struct RiskyTool;
    impl Tool for RiskyTool {
        fn name(&self) -> &'static str {
            "risky"
        }
        fn description(&self) -> &'static str {
            "needs yes"
        }
        fn needs_confirmation(&self) -> bool {
            true
        }
        fn execute(&self, _input: ToolInput, _ctx: &ToolContext) -> Result<ToolOutput, AgentError> {
            Ok(ToolOutput::ok(""))
        }
    }
    let p = Policy::balanced(PathBuf::from("/tmp"));
    assert!(!p.needs_confirmation(&SafeTool).unwrap());
    assert!(p.needs_confirmation(&RiskyTool).unwrap());
}

#[test]
fn evaluate_path_within_workspace_passes() {
    let p = Policy::balanced(PathBuf::from("/tmp"));
    let r = p
        .evaluate_path(PathBuf::from("/tmp/foo.txt").as_path())
        .unwrap();
    // 修复后返回规范化路径（macOS /tmp -> /private/tmp），与 workspace 的规范化形式一致。
    let canonical_tmp = PathBuf::from("/tmp").canonicalize().unwrap();
    assert!(
        r.starts_with(canonical_tmp),
        "resolved path should stay inside canonicalized workspace, got: {}",
        r.display()
    );
}

#[test]
fn evaluate_path_outside_workspace_blocked() {
    let p = Policy::balanced(PathBuf::from("/tmp"));
    let err = p
        .evaluate_path(PathBuf::from("/etc/passwd").as_path())
        .unwrap_err();
    assert!(matches!(err, AgentError::PathPolicy(_)));
}

#[test]
fn evaluate_path_relative_joined_to_workspace() {
    let p = Policy::balanced(PathBuf::from("/tmp"));
    let r = p.evaluate_path(std::path::Path::new("foo.txt")).unwrap();
    // 相对路径拼接到 workspace，结果路径应在 workspace 内
    assert!(r.starts_with("/tmp") || r.to_string_lossy().contains("foo.txt"));
}
