//! 报告生成：Summary 结构 + 文本报告输出。

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::bench::runner::{TaskResult, TaskStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    pub run_id: String,
    pub timestamp: String,
    pub agent: String,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub pass_rate: f64,
    pub total_wall_time_s: f64,
    pub tasks: Vec<TaskResult>,
    pub by_category: HashMap<String, CategoryStats>,
    pub by_difficulty: HashMap<String, DifficultyStats>,
}

impl Summary {
    pub fn from_results(run_id: &str, agent: &str, tasks: Vec<TaskResult>) -> Self {
        let total = tasks.len();
        let passed = tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Passed)
            .count();
        let failed = total - passed;
        let pass_rate = if total > 0 {
            passed as f64 / total as f64
        } else {
            0.0
        };
        let total_wall_time_s: f64 = tasks.iter().map(|t| t.agent_wall_time_s).sum();

        let mut by_category: HashMap<String, CategoryStats> = HashMap::new();
        let mut by_difficulty: HashMap<String, DifficultyStats> = HashMap::new();

        for task in &tasks {
            let cat = by_category
                .entry(task.category.clone())
                .or_insert_with(|| CategoryStats {
                    total: 0,
                    passed: 0,
                    rate: 0.0,
                });
            cat.total += 1;
            if task.status == TaskStatus::Passed {
                cat.passed += 1;
            }

            let diff = by_difficulty
                .entry(task.difficulty.clone())
                .or_insert_with(|| DifficultyStats {
                    total: 0,
                    passed: 0,
                    rate: 0.0,
                });
            diff.total += 1;
            if task.status == TaskStatus::Passed {
                diff.passed += 1;
            }
        }

        for stats in by_category.values_mut() {
            stats.rate = if stats.total > 0 {
                stats.passed as f64 / stats.total as f64
            } else {
                0.0
            };
        }
        for stats in by_difficulty.values_mut() {
            stats.rate = if stats.total > 0 {
                stats.passed as f64 / stats.total as f64
            } else {
                0.0
            };
        }

        Self {
            run_id: run_id.to_string(),
            timestamp: iso_timestamp(),
            agent: agent.to_string(),
            total,
            passed,
            failed,
            pass_rate,
            total_wall_time_s,
            tasks,
            by_category,
            by_difficulty,
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryStats {
    pub total: usize,
    pub passed: usize,
    pub rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DifficultyStats {
    pub total: usize,
    pub passed: usize,
    pub rate: f64,
}

pub fn format_text_report(summary: &Summary) -> String {
    let mut out = String::new();
    out.push_str("Aura Bench Report\n================\n");
    out.push_str(&format!("Run:       {}\n", summary.run_id));
    out.push_str(&format!("Agent:     {}\n", summary.agent));
    out.push_str(&format!(
        "Tasks:     {} total, {} passed, {} failed\n",
        summary.total, summary.passed, summary.failed
    ));
    out.push_str(&format!("Pass Rate: {:.1}%\n", summary.pass_rate * 100.0));
    out.push_str(&format!(
        "Wall Time: {:.1}s total, {:.1}s avg\n\n",
        summary.total_wall_time_s,
        if summary.total > 0 {
            summary.total_wall_time_s / summary.total as f64
        } else {
            0.0
        }
    ));

    out.push_str("By Category\n");
    for (cat, stats) in sorted(&summary.by_category) {
        let bar = make_bar(stats.rate, 10);
        out.push_str(&format!(
            "  {:<10} {}/{} {}  {:>5.1}%\n",
            cat,
            stats.passed,
            stats.total,
            bar,
            stats.rate * 100.0
        ));
    }
    out.push('\n');

    out.push_str("By Difficulty\n");
    for (diff, stats) in sorted(&summary.by_difficulty) {
        let bar = make_bar(stats.rate, 10);
        out.push_str(&format!(
            "  {:<10} {}/{} {}  {:>5.1}%\n",
            diff,
            stats.passed,
            stats.total,
            bar,
            stats.rate * 100.0
        ));
    }
    out.push('\n');

    let failed_tasks: Vec<_> = summary
        .tasks
        .iter()
        .filter(|t| t.status != TaskStatus::Passed)
        .collect();

    if !failed_tasks.is_empty() {
        out.push_str("Failed Tasks\n");
        for task in failed_tasks {
            let reason = task.error.as_deref().unwrap_or("unknown");
            out.push_str(&format!(
                "  [X] {} ({})\n      {}\n",
                task.task_id,
                status_label(&task.status),
                reason.chars().take(60).collect::<String>()
            ));
        }
    }

    out
}

/// Compare two summaries and produce a diff report.
#[must_use]
pub fn format_diff_report(base: &Summary, current: &Summary) -> String {
    let mut out = String::new();
    out.push_str("Aura Bench Diff Report\n");
    out.push_str("---------------------\n");
    out.push_str(&format!(
        "Base:   {} ({}/{} passed)\n",
        base.run_id, base.passed, base.total
    ));
    out.push_str(&format!(
        "Current: {} ({}/{} passed)\n\n",
        current.run_id, current.passed, current.total
    ));

    // Build lookup for base results
    let base_map: std::collections::HashMap<&str, &TaskResult> =
        base.tasks.iter().map(|t| (t.task_id.as_str(), t)).collect();

    out.push_str("Task Changes\n");
    let mut change_count = 0u32;
    for task in &current.tasks {
        let prev_opt = base_map.get(task.task_id.as_str());
        let is_changed = match prev_opt {
            None => true,
            Some(p) => {
                p.status != task.status
                    || (p.agent_wall_time_s - task.agent_wall_time_s).abs() > 0.1
                    || p.agent_turns != task.agent_turns
            }
        };

        if is_changed {
            change_count += 1;
            let prev_str = prev_opt.map_or("N/A", |p| status_label(&p.status));
            let cur_str = status_label(&task.status);
            let prev_wall = prev_opt.map_or(0.0, |p| p.agent_wall_time_s);
            let prev_turns = prev_opt.map_or(0u32, |p| p.agent_turns);
            out.push_str(&format!(
                "  {:<28} {} -> {}  {:>7.1} -> {:<7.1}  {:>3} -> {:>3}\n",
                task.task_id,
                prev_str,
                cur_str,
                prev_wall,
                task.agent_wall_time_s,
                prev_turns,
                task.agent_turns
            ));
        }
    }

    if change_count == 0 {
        out.push_str("  No changes detected.\n");
    } else {
        out.push_str(&format!(
            "\n{change_count} task(s) changed status or metrics.\n"
        ));
    }

    out
}

fn status_label(status: &TaskStatus) -> &'static str {
    match status {
        TaskStatus::Passed => "passed",
        TaskStatus::Failed => "failed (verify)",
        TaskStatus::Timeout => "timeout",
        TaskStatus::Error => "error",
    }
}

fn make_bar(rate: f64, width: usize) -> String {
    let filled = usize::try_from((rate * (width as f64)).round() as i64).unwrap_or(width);
    let empty = width.saturating_sub(filled);
    "█".repeat(filled) + &"░".repeat(empty)
}

fn sorted<K: std::cmp::Ord + Clone, V>(map: &HashMap<K, V>) -> Vec<(K, &V)> {
    let mut entries: Vec<_> = map.iter().collect();
    entries.sort_by_key(|e| e.0);
    entries.into_iter().map(|(k, v)| (k.clone(), v)).collect()
}

fn iso_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    let secs = now.as_secs();
    let base = 1_767_225_600i64; // 2026-01-01 00:00:00 UTC
    let elapsed = i64::try_from(secs).unwrap_or(0) - base;
    let days = elapsed / 86400;
    let rem = elapsed % 86400;
    let hours = rem / 3600;
    let minutes = (rem % 3600) / 60;
    let seconds = rem % 60;
    format!(
        "2026-01-{:02}T{:02}:{:02}:{:02}Z",
        u32::try_from(days + 1).unwrap_or(1),
        u32::try_from(hours).unwrap_or(0),
        u32::try_from(minutes).unwrap_or(0),
        u32::try_from(seconds).unwrap_or(0)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bench::runner::TaskResult;
    use crate::bench::spec::{Category, Difficulty};

    fn make_result(id: &str, status: TaskStatus) -> TaskResult {
        TaskResult {
            task_id: id.to_string(),
            task_name: id.to_string(),
            difficulty: Difficulty::Medium.to_string(),
            category: Category::Testing.to_string(),
            status,
            verify_exit_code: Some(0),
            agent_wall_time_s: 10.0,
            agent_turns: 2,
            error: None,
            workspace_snapshot: None,
        }
    }

    #[test]
    fn summary_from_results() {
        let results = vec![
            make_result("t1", TaskStatus::Passed),
            make_result("t2", TaskStatus::Passed),
            make_result("t3", TaskStatus::Failed),
        ];
        let summary = Summary::from_results("test-run", "aura", results);
        assert_eq!(summary.total, 3);
        assert_eq!(summary.passed, 2);
        assert!((summary.pass_rate - 0.667).abs() < 0.01);
    }

    #[test]
    fn category_stats() {
        let results = vec![
            make_result("t1", TaskStatus::Passed),
            make_result("t2", TaskStatus::Passed),
            make_result("t3", TaskStatus::Failed),
        ];
        let summary = Summary::from_results("test", "agent", results);
        let cat_stats = summary.by_category.get("testing").unwrap();
        assert_eq!(cat_stats.total, 3);
        assert_eq!(cat_stats.passed, 2);
    }

    #[test]
    fn text_report_format() {
        let results = vec![make_result("hello", TaskStatus::Passed)];
        let summary = Summary::from_results("run1", "aura", results);
        let report = format_text_report(&summary);
        assert!(report.contains("Aura Bench Report"));
        assert!(report.contains("1/1"));
        assert!(report.contains("100.0%"));
    }

    #[test]
    fn status_label_text() {
        assert_eq!(super::status_label(&TaskStatus::Passed), "passed");
        assert_eq!(super::status_label(&TaskStatus::Failed), "failed (verify)");
        assert_eq!(super::status_label(&TaskStatus::Timeout), "timeout");
        assert_eq!(super::status_label(&TaskStatus::Error), "error");
    }

    #[test]
    fn make_bar_renders() {
        assert_eq!(super::make_bar(0.0, 5), "░░░░░");
        assert_eq!(super::make_bar(1.0, 5), "█████");
        assert_eq!(super::make_bar(0.5, 5), "███░░");
    }
}
