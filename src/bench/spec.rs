//! 任务定义（TaskSpec）：YAML 解析 + 类型安全的任务描述。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 单个评测任务的完整定义。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    /// 任务唯一 ID（全小写，kebab-case）。
    pub id: String,
    /// 人类可读名称。
    pub name: String,
    /// 详细描述。
    pub description: Option<String>,
    /// 难度级别。
    pub difficulty: Difficulty,
    /// 任务分类（用于分组统计）。
    pub category: Category,
    /// 所需技能（用于过滤）。
    #[serde(default)]
    pub skills: Vec<String>,
    /// 前置准备步骤。
    #[serde(default)]
    pub setup: Vec<SetupAction>,
    /// 任务指令（发给 agent 的自然语言）。
    pub instruction: String,
    /// 验证方式。
    pub verify: VerifySpec,
    /// 参考答案路径（可选）。
    #[serde(default)]
    pub reference: Option<ReferenceSpec>,
    /// 标签（用于过滤）。
    #[serde(default)]
    pub tags: Vec<String>,
}

impl TaskSpec {
    /// 从 YAML 文件解析任务。
    pub fn from_path(path: impl Into<PathBuf>) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path.into())?;
        let spec: TaskSpec = serde_yaml::from_str(&content)?;
        Ok(spec)
    }

    /// 从 YAML 字符串解析任务。
    #[allow(clippy::self_named_constructors)]
    pub fn from_str(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }
}

/// 难度级别。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}

impl std::fmt::Display for Difficulty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Difficulty::Easy => write!(f, "easy"),
            Difficulty::Medium => write!(f, "medium"),
            Difficulty::Hard => write!(f, "hard"),
        }
    }
}

/// 任务分类。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    Testing,
    Bugfix,
    Refactor,
    Feature,
    Docs,
    Infra,
    Unknown,
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Category::Testing => write!(f, "testing"),
            Category::Bugfix => write!(f, "bugfix"),
            Category::Refactor => write!(f, "refactor"),
            Category::Feature => write!(f, "feature"),
            Category::Docs => write!(f, "docs"),
            Category::Infra => write!(f, "infra"),
            Category::Unknown => write!(f, "unknown"),
        }
    }
}

/// setup 操作类型。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum SetupAction {
    Write {
        path: String,
        content: String,
    },
    Mkdir {
        path: String,
    },
    Copy {
        from: String,
        to: String,
    },
    Clone {
        repo: String,
        #[serde(default = "default_clone_depth")]
        depth: u32,
    },
}

fn default_clone_depth() -> u32 {
    1
}

/// 验证方式。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VerifySpec {
    Command {
        command: String,
        #[serde(default = "default_cwd")]
        cwd: String,
        #[serde(default = "default_timeout")]
        timeout_seconds: u64,
    },
    FileExists {
        path: String,
    },
    CargoTest {
        #[serde(default = "default_timeout")]
        timeout_seconds: u64,
    },
    CargoFmt,
    GitDiff {
        #[serde(default)]
        pattern: Option<String>,
    },
}

fn default_cwd() -> String {
    "${AURA_WORKSPACE}".to_string()
}

fn default_timeout() -> u64 {
    60
}

/// 参考答案定义。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceSpec {
    pub file: Option<String>,
    pub coverage_target: Option<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE_TASK: &str = r#"
id: hello-world
name: Hello World
description: Print hello world
difficulty: easy
category: feature
skills: []
setup:
  - action: mkdir
    path: src
instruction: Create src/main.rs that prints "Hello, World!"
verify:
  type: command
  command: cargo run --quiet
  cwd: "${AURA_WORKSPACE}"
  timeout_seconds: 30
tags:
  - beginner
"#;

    #[test]
    fn parse_task_spec() {
        let spec = TaskSpec::from_str(EXAMPLE_TASK).unwrap();
        assert_eq!(spec.id, "hello-world");
        assert_eq!(spec.difficulty, Difficulty::Easy);
        assert!(matches!(spec.verify, VerifySpec::Command { .. }));
    }

    #[test]
    fn difficulty_serde() {
        let yaml = serde_yaml::to_string(&Difficulty::Medium).unwrap();
        assert_eq!(yaml.trim(), "medium");
    }

    #[test]
    fn setup_action_write() {
        let yaml = r#"
action: write
path: src/main.rs
content: |
  fn main() { println!("hi"); }
"#;
        let action: SetupAction = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(action, SetupAction::Write { .. }));
    }

    #[test]
    fn verify_command() {
        let yaml = r#"
type: command
command: cargo test
cwd: "${AURA_WORKSPACE}"
timeout_seconds: 60
"#;
        let spec: VerifySpec = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(spec, VerifySpec::Command { .. }));
    }

    #[test]
    fn verify_cargo_test() {
        let yaml = "type: cargo_test\ntimeout_seconds: 120";
        let spec: VerifySpec = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(spec, VerifySpec::CargoTest { .. }));
    }

    #[test]
    fn verify_cargo_fmt() {
        let yaml = "type: cargo_fmt";
        let spec: VerifySpec = serde_yaml::from_str(yaml).unwrap();
        assert!(matches!(spec, VerifySpec::CargoFmt));
    }
}
