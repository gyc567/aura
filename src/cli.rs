//! CLI 参数解析（clap derive）。
//!
//! 设计参考 [`docs/coding-agent-design.md`](../../docs/coding-agent-design.md) §11 Phase 4。
//!
//! v1 范围：
//! - 单子命令 `aura <INSTRUCTION>`（无子命令树）。
//! - 文本（默认）/ JSON 输出。
//! - 路径、轮次、策略等级、白名单工具。
//! - 模型选择：HTTP provider 或 `--fake-model`（确定性脚本，测试用）。
//!
//! 不在 v1 范围：
//! - TUI / 交互模式。
//! - 配置子命令（init / status 等）；CLI 只暴露 `aura` 主入口。

use std::path::PathBuf;

use clap::Parser;

use crate::policy::PolicyLevel;

/// Aura 编码智能体 CLI 参数。
#[derive(Debug, Clone, Parser)]
#[command(name = "aura", version, about = "KISS Rust coding agent")]
pub struct CliArgs {
    /// 自然语言任务指令。
    #[arg(value_name = "INSTRUCTION")]
    pub instruction: String,

    /// 工作区绝对路径；必须存在。
    #[arg(short, long, value_name = "PATH")]
    pub workspace: Option<PathBuf>,

    /// 最大循环轮次。
    #[arg(long, default_value_t = 12, value_name = "N")]
    pub max_turns: u32,

    /// 策略等级：strict / balanced / permissive。
    #[arg(long, value_enum, default_value_t = CliPolicyLevel::Balanced)]
    pub policy: CliPolicyLevel,

    /// 工具白名单（逗号分隔）。默认 `todo_write`。
    #[arg(
        long,
        value_delimiter = ',',
        default_value = "todo_write",
        value_name = "LIST"
    )]
    pub tools: Vec<String>,

    /// 跳过确认提示（v1 始终不阻塞，留作 forward-compat 占位）。
    #[arg(long)]
    pub yes: bool,

    /// JSON 结构化输出（覆盖默认文本输出）。
    #[arg(long)]
    pub json: bool,

    /// 使用 fake model（确定性脚本，无网络）。
    #[arg(long)]
    pub fake_model: bool,

    /// OpenAI-compatible endpoint（HTTP model 用）。
    #[arg(long, value_name = "URL")]
    pub endpoint: Option<String>,

    /// 模型名。
    #[arg(long)]
    pub model: Option<String>,

    /// API key（也可通过 `AURA_API_KEY` 环境变量）。
    #[arg(long, env = "AURA_API_KEY", hide_env_values = true)]
    pub api_key: Option<String>,
}

/// CLI 友好的 `PolicyLevel` 镜像。`clap` `value_enum` 要求单独类型。
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum CliPolicyLevel {
    /// 严格：所有副作用需要确认。
    Strict,
    /// 平衡（默认）。
    Balanced,
    /// 宽松：仅阻断 device write 与 system dir 修改。
    Permissive,
}

impl From<CliPolicyLevel> for PolicyLevel {
    fn from(level: CliPolicyLevel) -> Self {
        match level {
            CliPolicyLevel::Strict => Self::Strict,
            CliPolicyLevel::Balanced => Self::Balanced,
            CliPolicyLevel::Permissive => Self::Permissive,
        }
    }
}
