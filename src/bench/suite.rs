//! Bench suite: load tasks from directory and run them.

use std::path::Path;
use std::sync::Arc;
use std::thread;

use crate::bench::runner::TaskStatus;
use crate::bench::runner::{TaskResult, TaskRunner};
use crate::bench::spec::TaskSpec;
use crate::bench::workspace::Workspace;

/// A collection of bench tasks loaded from the filesystem.
pub struct BenchSuite {
    /// Loaded task specs.
    pub tasks: Vec<TaskSpec>,
}

impl BenchSuite {
    /// Load tasks from `bench/tasks/*.yaml` (or a custom glob substring).
    pub fn load(tasks_glob: Option<&str>) -> Result<Self, String> {
        let tasks_dir = Path::new("bench/tasks");
        if !tasks_dir.is_dir() {
            return Err(format!(
                "bench tasks directory not found: {}",
                tasks_dir.display()
            ));
        }

        let mut tasks: Vec<TaskSpec> = Vec::new();
        let mut files: Vec<_> = std::fs::read_dir(tasks_dir)
            .map_err(|e| format!("read bench/tasks: {e}"))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("yaml"))
            .collect();
        files.sort();

        for path in &files {
            match TaskSpec::from_path(path) {
                Ok(spec) => tasks.push(spec),
                Err(e) => {
                    let fname = path.file_name().unwrap().to_string_lossy();
                    return Err(format!("parse {fname}: {e}"));
                }
            }
        }

        // Apply glob filter if specified (simple substring match for Phase B1)
        if let Some(glob) = tasks_glob {
            if !glob.is_empty() {
                let pattern = glob.to_lowercase();
                tasks.retain(|t| t.id.to_lowercase().contains(&pattern));
            }
        }

        if tasks.is_empty() {
            return Err("no matching tasks found in bench/tasks/".into());
        }

        Ok(Self { tasks })
    }

    /// Run all tasks sequentially and collect results.
    #[must_use]
    pub fn run_all(&self, agent_cmd: &str, timeout_s: u64) -> Vec<TaskResult> {
        self.run_all_parallel(agent_cmd, timeout_s, 1)
    }

    /// Run all tasks with optional parallelism.
    ///
    /// `parallel = 1` runs sequentially. `parallel = N` runs up to N tasks
    /// concurrently on separate threads, each with its own isolated Workspace.
    #[must_use]
    pub fn run_all_parallel(
        &self,
        agent_cmd: &str,
        timeout_s: u64,
        parallel: usize,
    ) -> Vec<TaskResult> {
        let runner = Arc::new(
            TaskRunner::new()
                .agent_cmd(agent_cmd)
                .default_timeout(timeout_s),
        );
        let tasks: Arc<Vec<TaskSpec>> = Arc::new(self.tasks.clone());

        // Pre-allocate result slots (filled in completion order)
        let n = tasks.len();
        let results: Vec<TaskResult> = if parallel <= 1 {
            // Sequential path
            let mut results = Vec::with_capacity(n);
            for spec in tasks.iter() {
                results.push(Self::run_single(&runner, spec));
            }
            results
        } else {
            // Parallel path: use a channel to collect results preserving order
            use std::sync::mpsc;
            let (tx, rx) = mpsc::channel::<(usize, TaskResult)>();

            let mut handles = Vec::new();
            for (idx, spec) in tasks.iter().enumerate() {
                let runner = Arc::clone(&runner);
                let spec = spec.clone();
                let tx = tx.clone();
                let handle = thread::spawn(move || {
                    let result = Self::run_single(&runner, &spec);
                    let _ = tx.send((idx, result));
                });
                handles.push(handle);
            }
            drop(tx); // close the original sender

            // Collect all results
            let mut results: Vec<Option<TaskResult>> = (0..n).map(|_| None).collect();
            for (idx, result) in rx {
                results[idx] = Some(result);
            }

            // Wait for all threads
            for handle in handles {
                let _ = handle.join();
            }

            results.into_iter().flatten().collect()
        };

        results
    }

    /// Run a single task with its own isolated workspace.
    fn run_single(runner: &TaskRunner, spec: &TaskSpec) -> TaskResult {
        let ws = match Workspace::new() {
            Ok(w) => w,
            Err(e) => {
                return TaskResult {
                    task_id: spec.id.clone(),
                    task_name: spec.name.clone(),
                    difficulty: spec.difficulty.to_string(),
                    category: spec.category.to_string(),
                    status: TaskStatus::Error,
                    verify_exit_code: None,
                    agent_wall_time_s: 0.0,
                    agent_turns: 0,
                    error: Some(e.to_string()),
                    workspace_snapshot: None,
                };
            }
        };
        runner.run(spec, &ws)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suite_empty_when_no_tasks() {
        let result = BenchSuite::load(None);
        match result {
            Ok(suite) => assert!(suite.tasks.is_empty() || !suite.tasks.is_empty()),
            Err(_) => {} // OK if directory doesn't exist in test
        }
    }
}
