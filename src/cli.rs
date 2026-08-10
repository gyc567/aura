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
//! v1.2 新增：
//! - `aura bench` 子命令：run / report / init / list。
//!
//! 不在 v1 范围：
//! - TUI / 交互模式。
//! - 配置子命令（init / status 等）；CLI 只暴露 `aura` 主入口。

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::policy::PolicyLevel;

/// Aura 编码智能体 CLI 参数。
#[derive(Debug, Clone, Parser)]
#[command(name = "aura", version, about = "KISS Rust coding agent")]
pub struct CliArgs {
    /// 自然语言任务指令（当不使用子命令时必填）。
    #[arg(value_name = "INSTRUCTION")]
    pub instruction: Option<String>,

    /// 工作区绝对路径；必须存在。
    #[arg(short, long, value_name = "PATH")]
    pub workspace: Option<PathBuf>,

    /// 最大循环轮次。
    #[arg(long, default_value_t = 12, value_name = "N")]
    pub max_turns: u32,

    /// 策略等级：strict / balanced / permissive。
    #[arg(long, value_enum, default_value_t = CliPolicyLevel::Balanced)]
    pub policy: CliPolicyLevel,

    /// 工具白名单（逗号分隔）。默认包含所有工具。
    #[arg(
        long,
        value_delimiter = ',',
        default_value = "read_file,write_file,run_command,list_dir,grep_files,find_files,todo_write",
        value_name = "LIST"
    )]
    pub tools: Vec<String>,

    /// 跳过确认提示（v1 始终不阻塞，留作 forward-compat 占位）。
    #[arg(long)]
    pub yes: bool,

    /// JSON 结构化输出（覆盖默认文本输出）。
    #[arg(long)]
    pub json: bool,

    /// 恢复上次会话：从指定 JSONL transcript 文件读取消息历史。
    #[arg(long, value_name = "FILE")]
    pub resume: Option<PathBuf>,

    /// 使用 fake model（确定性脚本，无网络）。
    #[arg(long)]
    pub fake_model: bool,

    /// OpenAI-compatible endpoint（HTTP model 用）。
    #[arg(long, value_name = "URL")]
    pub endpoint: Option<String>,

    /// 模型名。
    #[arg(long)]
    pub model: Option<String>,

    /// API key（优先级：`--api-key` > 配置文件 > `AURA_API_KEY` 环境变量）。
    #[arg(long, hide_env_values = true)]
    pub api_key: Option<String>,

    /// 子命令（bench 等）。
    #[command(subcommand)]
    pub command: Option<CliCommand>,
}

/// 顶层子命令。
#[derive(Debug, Clone, Subcommand)]
pub enum CliCommand {
    /// 基准测试框架
    Bench(BenchCli),
    /// 首次运行配置向导：交互式设置大模型 provider 与 API key
    Setup(SetupCli),
}

/// `aura setup` 子命令参数。
///
/// slice 1：仅占位，无参数。后续 slice 会加 `--non-interactive` / `--provider` 等。
#[derive(Debug, Clone, Parser)]
#[command(name = "aura-setup", about = "Aura first-run onboarding wizard")]
pub struct SetupCli {
    /// 仅占位，未来可承载 `--non-interactive` / `--provider <id>` 等
    #[command(subcommand)]
    pub command: SetupCommand,
}

/// `aura setup` 预留子命令枚举（slice 1 为空）。
#[derive(Debug, Clone, Subcommand)]
pub enum SetupCommand {
    /// 默认行为：启动交互式 TUI 向导
    Wizard,
}

/// `aura bench` 子命令参数。
#[derive(Debug, Clone, Parser)]
#[command(name = "aura-bench", about = "Aura bench framework")]
pub struct BenchCli {
    #[command(subcommand)]
    /// bench 子命令。
    pub command: BenchCommand,
}

/// bench 子命令枚举。
#[derive(Debug, Clone, Subcommand)]
pub enum BenchCommand {
    /// 运行基准测试任务
    Run {
        /// 任务匹配模式（glob），默认 bench/tasks/*.yaml
        #[arg(long, short = 'g', value_name = "GLOB")]
        tasks: Option<String>,

        /// agent 命令（默认 cargo run --bin aura）
        #[arg(long, value_name = "CMD", default_value = "cargo run --bin aura")]
        agent: String,

        /// 并行任务数（默认 CPU 核数）
        #[arg(long, short, value_name = "N")]
        parallel: Option<usize>,

        /// 单任务超时秒数
        #[arg(long, default_value_t = 300, value_name = "SECS")]
        timeout: u64,

        /// 输出目录（默认 bench/results/<timestamp>/）
        #[arg(long, short, value_name = "DIR")]
        output: Option<String>,

        /// 沙箱模式：none | docker | nix
        #[arg(long, value_name = "MODE")]
        sandbox: Option<String>,
    },

    /// 生成报告
    Report {
        /// 结果目录
        #[arg(value_name = "DIR")]
        dir: String,
    },

    /// 初始化新任务脚手架
    Init {
        /// 任务名称（kebab-case）
        #[arg(value_name = "NAME")]
        name: String,
    },

    /// 列出可用任务
    List {},

    /// 比较两个运行结果的差异
    Diff {
        /// 基准结果目录
        #[arg(value_name = "BASE_DIR")]
        base_dir: String,
        /// 当前结果目录
        #[arg(value_name = "CURRENT_DIR")]
        current_dir: String,
    },
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
