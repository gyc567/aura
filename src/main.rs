//! Aura CLI 入口。
//!
//! v1 范围：
//! - 单任务模式：`aura <INSTRUCTION>`。
//! - 通过 `--fake-model` 启用确定性脚本（无网络），或 `--api-key` + `--endpoint` + `--model` 启用 HTTP 模型。
//! - 默认工具集：`read_file`, `write_file`, `run_command`, `list_dir`, `grep_files`, `find_files`, `todo_write`。
//! - SIGINT handler：设置 `Arc<AtomicBool>`，循环 graceful 停止。
//! - 文本输出（默认）或 JSON（`--json`）。
//!
//! v1.2 新增：`aura bench` 子命令（run / report / init / list）。
//!
//! 不在 v1 范围：TUI / 交互模式 / 配置子命令。

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use clap::Parser;

use aura::agent::run_with_session as run_agent_with_session;
use aura::agent::{RunReport, StopReasonPayload, run as run_agent};
use aura::bench::{
    BenchSuite, Summary, TaskResult, TaskSpec, format_diff_report,
    format_text_report as format_bench_report,
};
use aura::children::ChildRegistry;
use aura::cli::{BenchCommand, CliArgs, CliCommand, CliPolicyLevel};
use aura::domain::{Decision, TaskRequest, ToolArgument, ToolCall};
use aura::error::AgentError;
use aura::event::VecEventSink;
use aura::model::{ModelGateway, ModelRequest, ModelResponse};
use aura::output::{JsonReport, format_text_report as format_agent_text_report};
use aura::policy::Policy;
use aura::registry::ToolRegistry;
use aura::session::Session;
use aura::tools::agent_message::AgentMessageTool;
use aura::tools::subagent::SubagentTool;
use aura::tools::{
    find_files::FindFilesTool, grep_files::GrepFilesTool, list_dir::ListDirTool,
    read_file::ReadFileTool, run_command::RunCommandTool, scratchpad::ScratchpadTool,
    todo_write::TodoWriteTool, write_file::WriteFileTool,
};
use aura::{Budget, Config, ErrorBudget, HttpConfig, HttpModelAdapter, InMemoryRegistry};

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
    // Handle bench subcommand
    if let Some(command) = &args.command {
        return run_bench(command);
    }

    // Main agent mode (no subcommand)
    let instruction = args.instruction.as_deref().ok_or_else(|| {
        AgentError::InvalidRequest(
            "missing INSTRUCTION: provide a task or use `aura bench --help`".into(),
        )
    })?;

    run_agent_mode(args, instruction)
}

/// Handle bench subcommands.
fn run_bench(command: &CliCommand) -> Result<ExitCode, AgentError> {
    match command {
        CliCommand::Bench(bench) => match &bench.command {
            BenchCommand::Run {
                tasks,
                agent,
                parallel,
                timeout,
                output,
                sandbox: _,
            } => run_bench_run(
                tasks.as_deref(),
                agent,
                parallel.as_ref(),
                *timeout,
                output.as_deref(),
            ),
            BenchCommand::Report { dir } => run_bench_report(dir),
            BenchCommand::Init { name } => run_bench_init(name),
            BenchCommand::List {} => run_bench_list(),
            BenchCommand::Diff {
                base_dir,
                current_dir,
            } => run_bench_diff(base_dir, current_dir),
        },
    }
}

/// `aura bench run`: execute all matching task specs.
fn run_bench_run(
    tasks_glob: Option<&str>,
    agent_cmd: &str,
    parallel: Option<&usize>,
    timeout_s: u64,
    output_dir: Option<&str>,
) -> Result<ExitCode, AgentError> {
    let suite = BenchSuite::load(tasks_glob).map_err(AgentError::Context)?;
    let parallel_n = parallel
        .copied()
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, std::num::NonZero::get));
    let results: Vec<TaskResult> = suite.run_all_parallel(agent_cmd, timeout_s, parallel_n);
    // Generate summary
    let run_id = run_id_now();
    let agent_label = agent_cmd.to_string();
    let summary = Summary::from_results(&run_id, &agent_label, results);

    // Output
    let json =
        serde_json::to_string_pretty(&summary).map_err(|e| AgentError::Context(e.to_string()))?;
    println!("{json}");

    let text = format_bench_report(&summary);
    eprintln!("{text}");

    // Save results if output dir specified
    if let Some(dir) = output_dir {
        let dir_path = PathBuf::from(dir);
        std::fs::create_dir_all(&dir_path)
            .map_err(|e| AgentError::Context(format!("create output dir: {e}")))?;
        std::fs::write(dir_path.join("summary.json"), &json)
            .map_err(|e| AgentError::Context(format!("write summary: {e}")))?;
        for task in &summary.tasks {
            let task_json = serde_json::to_string_pretty(task)
                .map_err(|e| AgentError::Context(e.to_string()))?;
            std::fs::write(dir_path.join(format!("{}.json", task.task_id)), task_json)
                .map_err(|e| AgentError::Context(format!("write task result: {e}")))?;
        }
        eprintln!("Results saved to: {}", dir_path.display());
    }

    // Exit code: 0 if all passed, 1 if any failed
    let all_passed = summary.failed == 0;
    Ok(if all_passed {
        ExitCode::from(0)
    } else {
        ExitCode::from(1)
    })
}

/// `aura bench report`: read results dir and format text report.
fn run_bench_report(dir: &str) -> Result<ExitCode, AgentError> {
    let dir_path = Path::new(dir);
    let summary_path = dir_path.join("summary.json");
    let content = std::fs::read_to_string(&summary_path)
        .map_err(|e| AgentError::Context(format!("read {}: {e}", summary_path.display())))?;
    let summary: Summary = serde_json::from_str(&content)
        .map_err(|e| AgentError::Context(format!("parse summary.json: {e}")))?;

    println!("{}", format_bench_report(&summary));
    Ok(ExitCode::from(0))
}

/// `aura bench diff`: compare two result directories and show changes.
fn run_bench_diff(base_dir: &str, current_dir: &str) -> Result<ExitCode, AgentError> {
    let base_path = Path::new(base_dir);
    let cur_path = Path::new(current_dir);
    let base = std::fs::read_to_string(base_path.join("summary.json"))
        .map_err(|e| AgentError::Context(format!("read base summary: {e}")))?;
    let base: Summary = serde_json::from_str(&base)
        .map_err(|e| AgentError::Context(format!("parse base summary.json: {e}")))?;
    let cur = std::fs::read_to_string(cur_path.join("summary.json"))
        .map_err(|e| AgentError::Context(format!("read current summary: {e}")))?;
    let cur: Summary = serde_json::from_str(&cur)
        .map_err(|e| AgentError::Context(format!("parse current summary.json: {e}")))?;

    print!("{}", format_diff_report(&base, &cur));
    Ok(ExitCode::from(0))
}

/// `aura bench init`: create a new task scaffold.
fn run_bench_init(name: &str) -> Result<ExitCode, AgentError> {
    if name.is_empty() {
        return Err(AgentError::InvalidRequest(
            "task name cannot be empty".into(),
        ));
    }
    let tasks_dir = PathBuf::from("bench/tasks");
    std::fs::create_dir_all(&tasks_dir)
        .map_err(|e| AgentError::Context(format!("create bench/tasks: {e}")))?;
    let path = tasks_dir.join(format!("{name}.yaml"));
    let template = bench_task_template(name);
    std::fs::write(&path, template).map_err(|e| AgentError::Context(format!("write task: {e}")))?;
    eprintln!("Created task scaffold: {}", path.display());
    Ok(ExitCode::from(0))
}

/// `aura bench list`: list available tasks.
fn run_bench_list() -> Result<ExitCode, AgentError> {
    let tasks_dir = PathBuf::from("bench/tasks");
    if !tasks_dir.exists() {
        eprintln!("No bench/tasks directory found.");
        return Ok(ExitCode::from(0));
    }

    let mut tasks: Vec<PathBuf> = Vec::new();
    for entry in std::fs::read_dir(&tasks_dir)
        .map_err(|e| AgentError::Context(format!("read bench/tasks: {e}")))?
    {
        let entry = entry.map_err(|e| AgentError::Context(e.to_string()))?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("yaml") {
            tasks.push(path);
        }
    }
    tasks.sort();

    println!("Available tasks ({}):", tasks.len());
    for task_path in &tasks {
        match TaskSpec::from_path(task_path) {
            Ok(spec) => {
                println!("  {}  [{}] {}", spec.id, spec.difficulty, spec.name);
            }
            Err(e) => {
                let fname = task_path.file_name().unwrap().to_string_lossy();
                println!("  {fname}  [ERROR: {e}]");
            }
        }
    }
    Ok(ExitCode::from(0))
}

/// Generate a unique run ID based on current timestamp.
fn run_id_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    // Format as YYYY-MM-DDThhmmssZ (simplified)
    let days = secs / 86400;
    let rem = secs % 86400;
    let hours = rem / 3600;
    let minutes = (rem % 3600) / 60;
    let seconds = rem % 60;
    let base_days = 17532; // 2026-01-01 in days from epoch
    let date_days = days.saturating_sub(base_days);
    format!(
        "run-{:04}{:02}{:02}T{:02}{:02}{:02}Z",
        2026,
        u32::try_from(date_days / 365).unwrap_or(99) % 100, // Simplified year calc
        u32::try_from((date_days % 365) / 30 + 1).unwrap_or(12),
        hours as u32,
        minutes as u32,
        seconds as u32
    )
}

fn bench_task_template(name: &str) -> String {
    format!(
        r#"id: {name}
name: "Task: {name}"
description: |
  Brief description of what this task tests.
difficulty: easy
category: feature
skills: []
setup:
  - action: mkdir
    path: src
instruction: |
  Write a hello world program in Rust that prints "Hello, {name}!".
verify:
  type: command
  command: "cargo run --quiet"
  cwd: "${{AURA_WORKSPACE}}"
  timeout_seconds: 30
tags:
  - beginner
"#
    )
}

/// Run the main agent mode (existing logic, now with Option instruction).
fn run_agent_mode(args: &CliArgs, instruction: &str) -> Result<ExitCode, AgentError> {
    let workspace = resolve_workspace(args.workspace.as_deref())?;
    let task = TaskRequest::new(instruction, workspace.clone(), args.max_turns)?;
    let budget = Budget::new(args.max_turns, 100_000)?;
    let _policy = match args.policy {
        CliPolicyLevel::Strict => Policy::strict(workspace.clone()),
        CliPolicyLevel::Balanced => Policy::balanced(workspace.clone()),
        CliPolicyLevel::Permissive => Policy::permissive(workspace.clone()),
    };

    let child_registry: Arc<ChildRegistry> = Arc::new(ChildRegistry::new());
    let tool_registry_ref: Arc<Mutex<Option<Arc<InMemoryRegistry>>>> = Arc::new(Mutex::new(None));

    // 配置合并：CLI 参数 > ~/.config/aura/config.toml > 环境变量。
    let config = Config::load()?;
    let merged_model = Config::resolve(args.model.clone(), config.model.clone(), None);
    let model_choice = choose_model(args, &config);
    let model: Arc<dyn ModelGateway + Send + Sync> = model_choice.into_dyn();

    let registry = build_registry(
        &workspace,
        &args.tools,
        &model,
        &child_registry,
        &tool_registry_ref,
        2,
    )?;
    let registry = Arc::new(registry);

    // Wire tool_registry_ref for SubagentTool (after registry is built)
    *tool_registry_ref.lock().unwrap() = Some(registry.clone());

    let mut sink = VecEventSink::new();
    let interrupted = Arc::new(AtomicBool::new(false));

    let report = if let Some(resume_path) = &args.resume {
        let mut session =
            Session::resume(resume_path.clone(), workspace.clone(), merged_model.clone());
        futures_block_on(async {
            spawn_sigint_handler(interrupted.clone());
            run_agent_with_session(
                task.clone(),
                &*model,
                &*registry as &dyn ToolRegistry,
                budget,
                ErrorBudget::default(),
                &mut session,
                &mut sink,
                interrupted.clone(),
            )
            .await
        })?
    } else {
        futures_block_on(async {
            spawn_sigint_handler(interrupted.clone());
            run_agent(
                task.clone(),
                &*model,
                &*registry as &dyn ToolRegistry,
                budget,
                ErrorBudget::default(),
                &mut sink,
                interrupted.clone(),
                merged_model.clone(),
            )
            .await
        })?
    };

    if args.json {
        let jr = JsonReport::from_report(instruction, &workspace, &report);
        let s = jr
            .to_json()
            .map_err(|e| AgentError::Context(e.to_string()))?;
        println!("{s}");
    } else {
        print!(
            "{}",
            format_agent_text_report(instruction, &workspace, &report)
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
    fn into_dyn(self) -> Arc<dyn ModelGateway + Send + Sync> {
        match self {
            Self::Fake(m) => Arc::new(m),
            Self::Http(m) => Arc::new(m),
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

fn choose_model(args: &CliArgs, config: &Config) -> ModelChoice {
    if args.fake_model {
        return ModelChoice::Fake(build_fake_model());
    }
    let endpoint = Config::resolve(args.endpoint.clone(), config.endpoint.clone(), None);
    let model = Config::resolve(args.model.clone(), config.model.clone(), None);
    let api_key = Config::resolve(
        args.api_key.clone(),
        config.api_key.clone(),
        std::env::var("AURA_API_KEY").ok(),
    );
    if let (Some(endpoint), Some(model), Some(api_key)) = (endpoint, model, api_key) {
        let cfg = HttpConfig::new(endpoint, model, api_key);
        return ModelChoice::Http(HttpModelAdapter::new(cfg));
    }
    eprintln!(
        "warning: no API key or endpoint provided; running in fake mode (no real agentic \
        behavior). Provide --api-key and --endpoint for actual use, or pass --fake-model \
        explicitly."
    );
    ModelChoice::Fake(build_fake_model())
}

fn build_registry(
    workspace: &Path,
    tools: &[String],
    model: &Arc<dyn ModelGateway + Send + Sync>,
    child_registry: &Arc<ChildRegistry>,
    tool_registry_ref: &Arc<Mutex<Option<Arc<InMemoryRegistry>>>>,
    max_depth: u32,
) -> Result<InMemoryRegistry, AgentError> {
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
            "scratchpad" => built.push(Arc::new(ScratchpadTool::new(workspace.to_path_buf()))),
            other => {
                return Err(AgentError::Context(format!("unknown tool: `{other}`")));
            }
        }
    }
    // Always add subagent + agent_message tools (RLM subagent support)
    built.push(Arc::new(SubagentTool::new(
        model.clone(),
        child_registry.clone(),
        tool_registry_ref.clone(),
        max_depth,
    )));
    built.push(Arc::new(AgentMessageTool::new(child_registry.clone())));
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
fn futures_block_on<F: std::future::Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .expect("tokio build")
        .block_on(fut)
}

/// SIGINT handler：在 `multi_thread` runtime 内 spawn 监听 `ctrl_c`。
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
        StopReasonPayload::Completed { .. } => ExitCode::from(0),
        _ => ExitCode::from(1),
    }
}
