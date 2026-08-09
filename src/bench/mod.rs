//! Aura Bench Framework — 评测任务执行与报告。

#![allow(unused)]
#![allow(missing_docs)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::redundant_closure_for_method_calls)]
#![allow(clippy::format_push_string)]
#![allow(clippy::missing_fields_in_debug)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::bool_comparison)]
#![allow(clippy::unused_self)]

#[allow(clippy::all)]
mod report;
#[allow(clippy::all)]
mod runner;
#[allow(clippy::all)]
mod spec;
#[allow(clippy::all)]
mod suite;
#[allow(clippy::all)]
mod workspace;

pub use report::{CategoryStats, DifficultyStats, Summary, format_diff_report, format_text_report};
pub use runner::{TaskResult, TaskRunner, TaskStatus};
pub use spec::{Category, Difficulty, SetupAction, TaskSpec, VerifySpec};
pub use suite::BenchSuite;
pub use workspace::Workspace;
