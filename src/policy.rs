//! 能力声明与策略评估器。
//!
//! 设计参考 [`docs/coding-agent-design.md`](../../docs/coding-agent-design.md) §5.3.1 与 §5.3.2。
//!
//! - `Capability` 是工具静态声明的最小权限粒度。
//! - `Policy` 评估任务是否被授予 capability，并校验路径/命令是否越界。
//! - 高危命令由 [`crate::precheck`] regex 先行阻断；`Policy` 仅做 capability gate 与路径白名单。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::error::AgentError;
use crate::precheck::{self, RiskTier};
use crate::tool::Tool;

/// 能力粒度。v1 仅枚举最少需要的 7 类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    /// 读文件 / 列表目录。
    FsRead,
    /// 写文件。
    FsWrite,
    /// 执行命令。
    Exec,
    /// 发起 HTTP 请求（v1 不用）。
    Http,
    /// 创建/管理 session（v1 不用）。
    Session,
    /// 发送事件到 sink（v1 不用）。
    Events,
    /// 渲染 UI（v1 不用）。
    Ui,
}

/// 策略等级。`Strict` 额外要求每次写操作 confirmation；`Permissive` 放宽。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyLevel {
    /// 严格：默认拒绝所有副作用。
    Strict,
    /// 平衡（默认）：仅拒绝明确高危与 workspace 外路径。
    Balanced,
    /// 宽松：仅拒绝 device write 与 system dir 修改。
    Permissive,
}

/// 策略评估器。
#[derive(Debug, Clone)]
pub struct Policy {
    level: PolicyLevel,
    granted: HashSet<Capability>,
    workspace: PathBuf,
}

impl Policy {
    /// 用默认 balanced 配置构造：`FsRead` / `FsWrite` / `Exec` / `Session` 已授予。
    #[must_use]
    pub fn balanced(workspace: PathBuf) -> Self {
        let mut granted = HashSet::new();
        granted.insert(Capability::FsRead);
        granted.insert(Capability::FsWrite);
        granted.insert(Capability::Exec);
        granted.insert(Capability::Session);
        Self {
            level: PolicyLevel::Balanced,
            granted,
            workspace,
        }
    }

    /// 严格模式：除读外所有操作都需要 confirmation。
    #[must_use]
    pub fn strict(workspace: PathBuf) -> Self {
        let mut p = Self::balanced(workspace);
        p.level = PolicyLevel::Strict;
        p
    }

    /// 宽松模式：仅保留基础 capability，命令级阻断放宽。
    #[must_use]
    pub fn permissive(workspace: PathBuf) -> Self {
        let mut p = Self::balanced(workspace);
        p.level = PolicyLevel::Permissive;
        p
    }

    /// 当前策略等级。
    #[must_use]
    pub fn level(&self) -> PolicyLevel {
        self.level
    }

    /// 当前已授予的 capability 集合（用于测试）。
    #[must_use]
    pub fn granted(&self) -> &HashSet<Capability> {
        &self.granted
    }

    /// 工作区路径。
    #[must_use]
    pub fn workspace(&self) -> &Path {
        &self.workspace
    }

    /// 评估工具调用所需的 capability 是否全部被授予。
    ///
    /// # Errors
    ///
    /// - [`AgentError::CommandPolicy`]：缺失 capability。
    pub fn evaluate_capabilities(&self, tool: &dyn Tool) -> Result<(), AgentError> {
        for cap in tool.required_capabilities() {
            if !self.granted.contains(cap) {
                return Err(AgentError::CommandPolicy(format!(
                    "tool {} requires {:?} which is not granted",
                    tool.name(),
                    cap
                )));
            }
        }
        Ok(())
    }

    /// 评估路径是否在 workspace 内（路径越界检查）。
    ///
    /// # Errors
    ///
    /// - [`AgentError::PathPolicy`]：路径不在 workspace 内，或规范化失败。
    pub fn evaluate_path(&self, path: &Path) -> Result<PathBuf, AgentError> {
        // 委托 paths::resolve_in_workspace：统一符号链接与不存在文件的规范化，
        // 避免 macOS /tmp -> /private/tmp 等场景的误报。
        crate::paths::resolve_in_workspace(path, &self.workspace)
    }

    /// 评估命令 argv 是否可执行。
    ///
    /// # Errors
    ///
    /// - [`AgentError::CommandPolicy`]：高危命令被阻断。
    pub fn evaluate_command(&self, argv: &[String]) -> Result<(), AgentError> {
        let result = precheck::analyze(argv)?;
        if self.level == PolicyLevel::Strict && result.tier != RiskTier::Low {
            return Err(AgentError::CommandPolicy(format!(
                "strict policy blocks tier={:?} command",
                result.tier
            )));
        }
        if result.tier == RiskTier::High {
            return Err(AgentError::CommandPolicy(format!(
                "high-risk command blocked: categories={:?}",
                result.categories
            )));
        }
        Ok(())
    }

    /// 评估是否需要 confirmation（v1 简化为：写文件类工具 + strict level）。
    ///
    /// # Errors
    ///
    /// 当前实现不返回错误；调用方按 `Ok(true)` 决定是否阻塞等待用户确认。
    pub fn needs_confirmation(&self, tool: &dyn Tool) -> Result<bool, AgentError> {
        if self.level == PolicyLevel::Strict {
            return Ok(true);
        }
        Ok(tool.needs_confirmation())
    }
}
