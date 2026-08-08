//! Aura CLI 入口。
//!
//! v1 范围：
//! - 单任务模式：`aura <INSTRUCTION>`。
//! - 通过 `--fake-model` 启用确定性脚本（无网络），或 `--api-key` + `--endpoint` + `--model` 启用 HTTP 模型。
//! - 默认工具集：`read_file`, `write_file`, `run_command`, `list_dir`, `grep_files`, `find_files`, `todo_write`。
//! - SIGINT handler：设置 `Arc<AtomicBool>`，循环 graceful 停止。
//! - 文本输出（默认）或 JSON（`--json`）。
//!
//! 不在 v1 范围：TUI / 交互模式 / 配置子命令。

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::Parser;

use aura::agent::{RunReport, run as run_agent};
use aura::cli::{CliArgs, CliPolicyLevel};
use aura::domain::{Decision, TaskRequest, ToolArgument, ToolCall};
use aura::error::AgentError;
use aura::event::VecEventSink;
use aura::model::{ModelGateway, ModelRequest, ModelResponse};
use aura::output::{JsonReport, format_text_report};
use aura::policy::Policy;
use aura::registry::ToolRegistry;
use aura::tools::{
    find_files::FindFilesTool, grep_files::GrepFilesTool, list_dir::ListDirTool,
    read_file::ReadFileTool, run_command::RunCommandTool, todo_write::TodoWriteTool,
    write_file::WriteFileTool,
};
use aura::{Budget, HttpConfig, HttpModelAdapter, InMemoryRegistry};

fn main() -> ExitCode {
    let args = CliArgs::parse();
    match run(&args) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("aura error: {e}");
            ExitCode::from(e.exit_code())
        }
    }
}

fn run(args: &CliArgs) -> Result<ExitCode, AgentError> {
    let workspace = resolve_workspace(args.workspace.as_deref())?;
    let task = TaskRequest::new(&args.instruction, workspace.clone(), args.max_turns)?;
    let budget = Budget::new(args.max_turns, 100_000)?;
    // Policy is constructed here to validate CLI input, but NOT yet wired into Agent::run.
    // v1 design defers tool-call policy gating to a future phase.
    // Wire Policy::evaluate_* methods into the agent loop when file/run_command tools land.
    let _policy = match args.policy {
        CliPolicyLevel::Strict => Policy::strict(workspace.clone()),
        CliPolicyLevel::Balanced => Policy::balanced(workspace.clone()),
        CliPolicyLevel::Permissive => Policy::permissive(workspace.clone()),
    };

    let registry = build_registry(&args.tools)?;
    let mut sink = VecEventSink::new();
    let interrupted = Arc::new(AtomicBool::new(false));

    // 模型选择：`fake-model` 优先；；则检测 API key 启用 HTTP；两者皆无则 fallback 到 fake。
    let model_choice = choose_model(args);
    let model = model_choice.into_dyn();

    let report = futures_block_on(async {
        // SIGINT handler 在 runtime 内 spawn
        spawn_sigint_handler(interrupted.clone());
        run_agent(
            task.clone(),
            &*model,
            &registry as &dyn ToolRegistry,
            budget,
            &mut sink,
            interrupted.clone(),
        )
        .await
    })?;

    // 输出
    if args.json {
        let jr = JsonReport::from_report(&args.instruction, &workspace, &report);
        let s = jr
            .to_json()
            .map_err(|e| AgentError::Context(e.to_string()))?;
        println!("{s}");
    } else {
        print!(
            "{}",
            format_text_report(&args.instruction, &workspace, &report)
        );
    }

    Ok(exit_code_from_report(&report))
}

/// 模型选择。
enum ModelChoice {
    /// 确定性 fake 脚本（无网络）。
    Fake(FakeModel),
    /// HTTP OpenAI-compatible provider。
    Http(HttpModelAdapter),
}

impl ModelChoice {
    fn into_dyn(self) -> Box<dyn aura::ModelGateway> {
        match self {
            Self::Fake(m) => Box::new(m),
            Self::Http(m) => Box::new(m),
        }
    }
}

/// 构造 fake model：默认脚本 = `todo_write` 创建一条 TODO → `Done`。
fn build_fake_model() -> FakeModel {
    let todos = serde_json::json!({
        "todos": [
            {
                "id": "1",
                "content": "Plan the work via todo_write",
                "status": "in_progress",
                "priority": "high"
            }
        ]
    });
    let call =
        ToolCall::new("c1", "todo_write", ToolArgument::new(todos)).expect("valid TodoWrite call");
    let decisions = vec![
        Decision::Call(call),
        Decision::Done {
            summary: "TODO list created; user can review and re-run with file tools.".into(),
        },
    ];
    FakeModel::new(decisions)
}

fn choose_model(args: &CliArgs) -> ModelChoice {
    if args.fake_model {
        return ModelChoice::Fake(build_fake_model());
    }
    if let (Some(endpoint), Some(model), Some(api_key)) =
        (&args.endpoint, &args.model, &args.api_key)
    {
        let cfg = HttpConfig::new(endpoint.clone(), model.clone(), api_key.clone());
        return ModelChoice::Http(HttpModelAdapter::new(cfg));
    }
    // 缺 endpoint/model/api_key → fallback 到 fake，避免启动失败
    eprintln!(
        "warning: no API key or endpoint provided; running in fake mode (no real agentic \
        behavior). Provide --api-key and --endpoint for actual use, or pass --fake-model \
        explicitly."
    );
    ModelChoice::Fake(build_fake_model())
}

fn build_registry(tools: &[String]) -> Result<InMemoryRegistry, AgentError> {
    let mut built: Vec<Arc<dyn aura::Tool>> = Vec::new();
    for name in tools {
        match name.as_str() {
            "todo_write" => built.push(Arc::new(TodoWriteTool::new())),
            "read_file" => built.push(Arc::new(ReadFileTool::new())),
            "write_file" => built.push(Arc::new(WriteFileTool::new())),
            "run_command" => built.push(Arc::new(RunCommandTool::new())),
            "list_dir" => built.push(Arc::new(ListDirTool::new())),
            "grep_files" => built.push(Arc::new(GrepFilesTool::new())),
            "find_files" => built.push(Arc::new(FindFilesTool::new())),
            other => {
                return Err(AgentError::Context(format!("unknown tool: `{other}`")));
            }
        }
    }
    Ok(InMemoryRegistry::new(built))
}

/// `FakeModel`：按 FIFO 队列返回 `Decision`。
struct FakeModel {
    queue: Mutex<Vec<Decision>>,
}

impl FakeModel {
    fn new(decisions: Vec<Decision>) -> Self {
        Self {
            queue: Mutex::new(decisions),
        }
    }
}

impl ModelGateway for FakeModel {
    fn complete(
        &self,
        _req: ModelRequest,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<ModelResponse, AgentError>> + Send + '_>,
    > {
        let next = {
            let mut queue = self.queue.lock().unwrap();
            assert!(
                !queue.is_empty(),
                "FakeModel queue exhausted — provide enough decisions for all turns"
            );
            queue.remove(0)
        };
        Box::pin(async move {
            Ok(ModelResponse {
                raw: format!("{next:?}"),
                decision: next,
            })
        })
    }
}

/// 在 `current_thread` runtime 中同步等待 future。
/// 由于 `Agent::run` 已经是 `async fn`，但 `ModelGateway::complete` 返回 `impl Future + Send`，
/// 不能在非 async 上下文中直接 `.await`。此 helper 用 `tokio` runtime 包装。
fn futures_block_on<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio build")
        .block_on(fut)
}

/// SIGINT handler：在 `current_thread` runtime 内 spawn 监听 `ctrl_c`。
fn spawn_sigint_handler(interrupted: Arc<AtomicBool>) {
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            interrupted.store(true, Ordering::Relaxed);
        }
    });
}

/// 解析 workspace：缺省取当前目录；不存在报错。
fn resolve_workspace(given: Option<&Path>) -> Result<PathBuf, AgentError> {
    let path = match given {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir()
            .map_err(|e| AgentError::InvalidRequest(format!("current_dir: {e}")))?,
    };
    if !path.exists() {
        return Err(AgentError::InvalidRequest(format!(
            "workspace does not exist: {}",
            path.display()
        )));
    }
    if !path.is_absolute() {
        return Err(AgentError::InvalidRequest(format!(
            "workspace must be absolute: {}",
            path.display()
        )));
    }
    Ok(path)
}

/// 退出码由 `StopReason` 决定。
pub(crate) fn exit_code_from_report(report: &RunReport) -> ExitCode {
    match &report.stop_reason {
        aura::agent::StopReasonPayload::Completed { .. } => ExitCode::from(0),
        _ => ExitCode::from(1),
    }
}
