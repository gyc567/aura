//! RLM 式子代理注册表（v1.2 新增，Architecture §4.2）。
//!
//! `ChildRegistry` 是父作用域的共享状态，管理所有背景子代理的生命周期：
//! - `register`: 子代理通过 `subagent` 工具注册
//! - `status` / `list`: 父代理查询子代理状态
//! - `fetch_result`: 获取已完成子代理的结果
//! - `delete`: 清理已完成子代理
//! - `send_message`: 向子代理的邮箱投递消息

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::domain::{AgentMessage, ChildHandle, ChildId, ChildStatus};

/// 共享的子代理注册表。
///
/// 使用 `Arc<Mutex<...>>` 包装，以便在父代理循环与后台 tokio 任务之间共享。
pub struct ChildRegistry {
    children: Mutex<HashMap<ChildId, ChildHandle>>,
}

impl ChildRegistry {
    /// 创建空注册表。
    #[must_use]
    pub fn new() -> Self {
        Self {
            children: Mutex::new(HashMap::new()),
        }
    }

    /// 注册一个新子代理，返回其 ID。
    pub fn register(
        &self,
        name: Option<String>,
        session_dir: PathBuf,
        status: ChildStatus,
    ) -> ChildId {
        let child_id = ChildId::generate();
        let handle = ChildHandle {
            child_id: child_id.clone(),
            name,
            session_dir,
            status,
            result: None,
            inbox: Vec::new(),
        };
        self.children
            .lock()
            .unwrap()
            .insert(child_id.clone(), handle);
        child_id
    }

    /// 获取子代理句柄（克隆）。
    #[must_use]
    pub fn get(&self, child_id: &ChildId) -> Option<ChildHandle> {
        let guard = self.children.lock().unwrap();
        let handle = guard.get(child_id)?;
        Some(clone_handle(handle))
    }

    /// 更新子代理状态。
    pub fn set_status(&self, child_id: &ChildId, status: ChildStatus) {
        if let Some(handle) = self.children.lock().unwrap().get_mut(child_id) {
            handle.status = status;
        }
    }

    /// 记录子代理完成结果。
    pub fn set_result(&self, child_id: &ChildId, result: String) {
        if let Some(handle) = self.children.lock().unwrap().get_mut(child_id) {
            handle.status = ChildStatus::Completed;
            handle.result = Some(result);
        }
    }

    /// 向子代理的邮箱投递消息。
    pub fn send_message(&self, child_id: &ChildId, msg: AgentMessage) -> bool {
        let mut guard = self.children.lock().unwrap();
        match guard.get_mut(child_id) {
            Some(handle) => {
                handle.inbox.push(msg);
                true
            }
            None => false,
        }
    }

    /// 取出子代理邮箱中的全部消息并清空（子代理每轮拉取一次）。
    #[must_use]
    pub fn drain_inbox(&self, child_id: &ChildId) -> Vec<AgentMessage> {
        let mut guard = self.children.lock().unwrap();
        guard
            .get_mut(child_id)
            .map(|h| std::mem::take(&mut h.inbox))
            .unwrap_or_default()
    }

    /// 移除子代理。
    pub fn delete(&self, child_id: &ChildId) -> bool {
        self.children.lock().unwrap().remove(child_id).is_some()
    }

    /// 列出所有子代理 ID。
    #[must_use]
    pub fn list(&self) -> Vec<(ChildId, ChildStatus, Option<String>)> {
        self.children
            .lock()
            .unwrap()
            .values()
            .map(|h| (h.child_id.clone(), h.status, h.name.clone()))
            .collect()
    }

    /// 获取已完成子代理的结果。
    #[must_use]
    pub fn fetch_result(&self, child_id: &ChildId) -> Option<String> {
        self.children.lock().unwrap().get(child_id)?.result.clone()
    }

    /// 注册表中的子代理数量。
    #[must_use]
    pub fn len(&self) -> usize {
        self.children.lock().unwrap().len()
    }

    /// 注册表是否为空。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for ChildRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// 子代理收件箱句柄：`run_with_session` 每轮模型请求前拉取待处理消息。
///
/// parent 通过 `agent_message` 投递的消息在此被取出并注入子会话
/// （架构 §4.2：`delivered to the child's inbox and will be processed
/// in its next turn`）。
#[derive(Clone)]
pub struct ChildInbox {
    registry: Arc<ChildRegistry>,
    child_id: ChildId,
}

impl ChildInbox {
    /// 创建收件箱句柄。
    #[must_use]
    pub fn new(registry: Arc<ChildRegistry>, child_id: ChildId) -> Self {
        Self { registry, child_id }
    }

    /// 取出待处理消息并清空邮箱（空 Vec 表示无新消息）。
    #[must_use]
    pub fn drain(&self) -> Vec<AgentMessage> {
        self.registry.drain_inbox(&self.child_id)
    }
}

/// 克隆 ChildHandle（跳过 inbox，避免深拷贝已消费的消息）。
fn clone_handle(handle: &ChildHandle) -> ChildHandle {
    ChildHandle {
        child_id: handle.child_id.clone(),
        name: handle.name.clone(),
        session_dir: handle.session_dir.clone(),
        status: handle.status,
        result: handle.result.clone(),
        inbox: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_register_and_get() {
        let reg = ChildRegistry::new();
        let id = reg.register(
            Some("worker-1".into()),
            PathBuf::from("/tmp/child1"),
            ChildStatus::Running,
        );
        let handle = reg.get(&id).expect("child should exist");
        assert_eq!(handle.child_id, id);
        assert_eq!(handle.name, Some("worker-1".into()));
        assert_eq!(handle.status, ChildStatus::Running);
        assert!(handle.result.is_none());
    }

    #[test]
    fn registry_set_result() {
        let reg = ChildRegistry::new();
        let id = reg.register(None, PathBuf::from("/tmp/test"), ChildStatus::Running);
        reg.set_result(&id, "Task completed successfully".into());
        let handle = reg.get(&id).unwrap();
        assert_eq!(handle.status, ChildStatus::Completed);
        assert_eq!(handle.result, Some("Task completed successfully".into()));
    }

    #[test]
    fn registry_send_and_fetch() {
        let reg = ChildRegistry::new();
        let id = reg.register(None, PathBuf::from("/tmp/test"), ChildStatus::Running);
        let msg = AgentMessage {
            to: id.clone(),
            from: "parent".into(),
            content: "keep going".into(),
        };
        assert!(reg.send_message(&id, msg));
        assert!(reg.delete(&id));
        assert!(!reg.delete(&id));
    }

    #[test]
    fn registry_list_and_len() {
        let reg = ChildRegistry::new();
        let id1 = reg.register(
            Some("a".into()),
            PathBuf::from("/tmp/a"),
            ChildStatus::Running,
        );
        let id2 = reg.register(
            Some("b".into()),
            PathBuf::from("/tmp/b"),
            ChildStatus::Running,
        );
        assert_eq!(reg.len(), 2);
        let list = reg.list();
        let ids: Vec<&ChildId> = list.iter().map(|(id, _, _)| id).collect();
        assert!(ids.contains(&&id1));
        assert!(ids.contains(&&id2));
    }

    #[test]
    fn registry_send_message_to_unknown_fails() {
        let reg = ChildRegistry::new();
        let fake_id = ChildId("nonexistent".into());
        let msg = AgentMessage {
            to: fake_id.clone(),
            from: "parent".into(),
            content: "hello".into(),
        };
        assert!(!reg.send_message(&fake_id, msg));
    }

    #[test]
    fn registry_drain_inbox_returns_and_clears() {
        let reg = ChildRegistry::new();
        let id = reg.register(
            Some("child".into()),
            PathBuf::from("/tmp/c"),
            ChildStatus::Running,
        );
        reg.send_message(
            &id,
            AgentMessage {
                to: id.clone(),
                from: "parent".into(),
                content: "m1".into(),
            },
        );
        reg.send_message(
            &id,
            AgentMessage {
                to: id.clone(),
                from: "parent".into(),
                content: "m2".into(),
            },
        );
        let drained = reg.drain_inbox(&id);
        assert_eq!(drained.len(), 2);
        assert_eq!(drained[0].content, "m1");
        assert!(reg.get(&id).unwrap().inbox.is_empty(), "inbox cleared");
        // 再次 drain 返回空
        assert!(reg.drain_inbox(&id).is_empty());
        // 未知 child 返回空
        assert!(reg.drain_inbox(&ChildId("nope".into())).is_empty());
    }

    #[test]
    fn child_inbox_drain_delegates_to_registry() {
        let reg = Arc::new(ChildRegistry::new());
        let id = reg.register(
            Some("child".into()),
            PathBuf::from("/tmp/c"),
            ChildStatus::Running,
        );
        let inbox = ChildInbox::new(reg.clone(), id.clone());
        reg.send_message(
            &id,
            AgentMessage {
                to: id.clone(),
                from: "parent".into(),
                content: "ping".into(),
            },
        );
        let msgs = inbox.drain();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].content, "ping");
        assert!(inbox.drain().is_empty());
    }
}
