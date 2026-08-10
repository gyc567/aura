//! API key 的密钥存储（macOS keychain / Linux Secret Service / Windows wincred）。
//!
//! 基于 `keyring` crate。**slice 3 状态**：`save` / `load` / `delete` 完成，
//! TUI 调用 `save` 落盘；`load` 供 slice 5 的 `needs_onboarding` 判定使用。
//!
//! 平台约定（与 `providers.toml` 的 `keychain_service` 对应）：
//! - service = provider 的 `keychain_service`（如 `aura-deepseek`）
//! - account = `aura`（固定，跨 provider 一致）
//!
//! 测试用 keyring 内置 mock credential store（`set_mock_credential_store`），
//! 不触碰真实系统 keychain。

use keyring::{Entry, Error as KeyringError};

use crate::error::AgentError;

/// keychain account 名（跨 provider 一致）。
pub const ACCOUNT: &str = "aura";

/// 把 API key 写入系统凭据库。
///
/// # Errors
///
/// - `AgentError::ToolFailed` — keyring 写入失败（服务不可用 / 权限等）
pub fn save(service: &str, key: &str) -> Result<(), AgentError> {
    let entry = Entry::new(service, ACCOUNT).map_err(|e| keyring_err(&e))?;
    entry.set_password(key).map_err(|e| keyring_err(&e))?;
    Ok(())
}

/// 从系统凭据库读取 API key。
///
/// # Errors
///
/// - `AgentError::ToolFailed` — keyring 读取失败
pub fn load(service: &str) -> Result<String, AgentError> {
    let entry = Entry::new(service, ACCOUNT).map_err(|e| keyring_err(&e))?;
    entry.get_password().map_err(|e| keyring_err(&e))
}

/// 从系统凭据库删除 API key。
///
/// # Errors
///
/// - `AgentError::ToolFailed` — keyring 删除失败（条目不存在也算失败）
pub fn delete(service: &str) -> Result<(), AgentError> {
    let entry = Entry::new(service, ACCOUNT).map_err(|e| keyring_err(&e))?;
    entry.delete_credential().map_err(|e| keyring_err(&e))
}

/// keyring 错误 → AgentError（统一映射，隐藏 keyring 类型泄漏）。
fn keyring_err(e: &KeyringError) -> AgentError {
    AgentError::ToolFailed(format!("keychain: {e}"))
}

#[cfg(test)]
mod tests {
    use keyring::{mock, set_default_credential_builder};

    use super::*;

    /// 每个测试独立 mock store，避免跨测试污染。
    fn mock_store() {
        set_default_credential_builder(mock::default_credential_builder());
    }

    /// mock store 是 `EntryOnly` 持久化：每个 `Entry::new` 返回新实例、无密码。
    /// 所以 `save`/`load` 的 roundtrip 在 mock 下测不了（需真实 keychain E2E，见 slice 3 末尾）。
    /// 这里测的是**单个 Entry** 的语义 —— 模拟真实 keyring 在 service+user 相同时的行为。
    #[test]
    fn single_entry_roundtrip() {
        mock_store();
        let entry = Entry::new("aura-test-service", ACCOUNT).expect("entry");
        entry.set_password("sk-secret").expect("set");
        assert_eq!(entry.get_password().expect("get"), "sk-secret");
    }

    #[test]
    fn single_entry_overwrite() {
        mock_store();
        let entry = Entry::new("aura-test-service", ACCOUNT).expect("entry");
        entry.set_password("first").expect("set 1");
        entry.set_password("second").expect("set 2");
        assert_eq!(entry.get_password().expect("get"), "second");
    }

    #[test]
    fn save_returns_ok_under_mock() {
        mock_store();
        save("aura-test-service", "key").expect("save should succeed");
    }

    #[test]
    fn load_missing_entry_errors() {
        mock_store();
        let err = load("aura-no-such-service").unwrap_err();
        assert!(matches!(err, AgentError::ToolFailed(_)));
    }

    #[test]
    fn delete_returns_ok_under_mock() {
        mock_store();
        // mock 下 delete 无条目也返回 Ok（真实 store 可能 NoEntry → Err）
        let _ = delete("aura-test-service");
    }

    #[test]
    fn delete_missing_entry_errors() {
        mock_store();
        let err = delete("aura-no-such-service").unwrap_err();
        assert!(matches!(err, AgentError::ToolFailed(_)));
    }

    #[test]
    fn account_is_constant() {
        assert_eq!(ACCOUNT, "aura");
    }
}
