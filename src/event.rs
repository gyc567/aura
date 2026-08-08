//! 智能体运行时事件与事件接收器。
//!
//! 事件用于 CLI 展示、日志与测试观测。事件类型本身是纯数据，
//! 接收器可以是终端、JSON 写入器或测试中的 `Vec`。

use crate::state::StopReason;

/// 一次循环中可能产生的所有事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentEvent {
    /// 任务启动。
    Started {
        /// 任务指令。
        task: String,
    },
    /// 上下文收集完成。
    ContextCollected {
        /// 收集到的文件数。
        files: usize,
        /// 总字节数。
        bytes: u64,
    },
    /// 模型被请求一次。
    ModelRequested,
    /// 工具开始执行。
    ToolStarted {
        /// 工具名。
        name: String,
    },
    /// 工具执行结束。
    ToolFinished {
        /// 工具名。
        name: String,
        /// 是否成功。
        success: bool,
    },
    /// 验证结束（仅含摘要，详细报告由 `verify` 模块产出）。
    VerificationFinished {
        /// 是否全部通过。
        success: bool,
    },
    /// 任务以正常原因结束。
    Completed {
        /// 摘要。
        summary: String,
    },
    /// 任务失败。
    Failed {
        /// 失败描述。
        error: String,
    },
    /// 智能体停止。
    Stopped {
        /// 停止原因。
        reason: StopReason,
    },
}

/// 事件接收器抽象。
///
/// 接收器应当是非阻塞、幂等且不抛错的；实现内部应自行处理 IO 错误，
/// 避免事件链污染核心循环。
pub trait EventSink {
    /// 派发一个事件。
    fn emit(&mut self, event: AgentEvent);
}

/// 测试与脚本友好的收集型接收器。
#[derive(Debug, Default, Clone)]
pub struct VecEventSink {
    events: Vec<AgentEvent>,
}

impl VecEventSink {
    /// 创建空收集器。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 已收集的事件引用。
    #[must_use]
    pub fn events(&self) -> &[AgentEvent] {
        &self.events
    }

    /// 取出所有权并返回事件向量。
    #[must_use]
    pub fn into_events(self) -> Vec<AgentEvent> {
        self.events
    }
}

impl EventSink for VecEventSink {
    fn emit(&mut self, event: AgentEvent) {
        self.events.push(event);
    }
}
