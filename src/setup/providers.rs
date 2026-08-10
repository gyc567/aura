//! Provider catalog — 内嵌 `providers.toml` 的运行时访问。
//!
//! **slice 2 状态**：catalog 数据 + 查找 API 完成。slice 3+ 由 TUI 消费。

use std::sync::LazyLock;

use serde::Deserialize;

use crate::error::AgentError;

/// 单个 provider 定义（对应 `providers.toml` 的一节）。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Provider {
    /// 唯一标识（`config.toml` 的 `provider` 字段值）。
    pub id: String,
    /// TUI 显示名。
    pub display_name: String,
    /// OpenAI-compatible base URL（**不含** `/v1` —— `HttpConfig::url()` 会拼 `/v1/chat/completions`）。
    pub endpoint: String,
    /// 默认模型名。
    pub default_model: String,
    /// 该 provider 的 API key 环境变量名。
    pub env_var: String,
    /// keychain service 名（macOS / Linux secret-service）。
    pub keychain_service: String,
    /// TUI picker 展示的额外模型（可选，可为空）。
    #[serde(default)]
    pub extra_models: Vec<String>,
}

/// `providers.toml` 的顶层结构。
#[derive(Debug, Deserialize)]
struct Catalog {
    #[serde(rename = "providers")]
    providers: Vec<Provider>,
}

/// 解析一次、缓存全进程（`LazyLock`，初始器编译期已知）。
static CATALOG: LazyLock<Catalog> = LazyLock::new(|| {
    // include_str! 在编译期嵌入；解析失败 = 编译时 bug（fail fast 最合适）。
    let raw = include_str!("providers.toml");
    toml::from_str(raw).expect("embedded providers.toml must be valid toml")
});

/// 全部 provider（按 toml 顺序）。
#[must_use]
pub fn all() -> &'static [Provider] {
    &CATALOG.providers
}

/// 按 id 查找 provider。
///
/// # Errors
///
/// `AgentError::UnknownTool` — 找不到时（与现有 tool registry 的 unknown 语义一致）。
pub fn lookup(id: &str) -> Result<&'static Provider, AgentError> {
    all()
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| AgentError::UnknownTool(format!("unknown provider: {id}")))
}

/// 某个 provider 的默认 (`endpoint`, `model`, `env_var`) 三元组。
#[must_use]
pub fn default_for_id(id: &str) -> Option<(&'static str, &'static str, &'static str)> {
    all().iter().find(|p| p.id == id).map(|p| {
        (
            p.endpoint.as_str(),
            p.default_model.as_str(),
            p.env_var.as_str(),
        )
    })
}

/// 验证 catalog 数据一致性（slice 2 单测 + 未来编译期 lint 用）。
///
/// - 每个 provider 的 `id` / `env_var` / `keychain_service` 必须唯一
/// - `custom` 是保留 id（走用户输入，endpoint/model 可为空）
#[must_use]
pub fn validate_invariants() -> Vec<String> {
    let mut problems = Vec::new();
    let mut ids: Vec<&str> = Vec::new();
    let mut envs: Vec<&str> = Vec::new();
    let mut svcs: Vec<&str> = Vec::new();

    for p in all() {
        if ids.contains(&p.id.as_str()) {
            problems.push(format!("duplicate id: {}", p.id));
        }
        ids.push(&p.id);

        if envs.contains(&p.env_var.as_str()) {
            problems.push(format!(
                "duplicate env_var: {} (provider {})",
                p.env_var, p.id
            ));
        }
        envs.push(&p.env_var);

        if svcs.contains(&p.keychain_service.as_str()) {
            problems.push(format!(
                "duplicate keychain_service: {} (provider {})",
                p.keychain_service, p.id
            ));
        }
        svcs.push(&p.keychain_service);

        if p.id != "custom" && (p.endpoint.is_empty() || p.default_model.is_empty()) {
            problems.push(format!(
                "non-custom provider {} must have endpoint + default_model",
                p.id
            ));
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_parses_and_has_4_providers() {
        assert_eq!(all().len(), 4);
    }

    #[test]
    fn all_required_providers_present() {
        let ids: Vec<&str> = all().iter().map(|p| p.id.as_str()).collect();
        for want in ["deepseek", "minimax", "kimi", "custom"] {
            assert!(ids.contains(&want), "missing provider {want}");
        }
    }

    #[test]
    fn deepseek_lookup() {
        let p = lookup("deepseek").expect("deepseek must exist");
        assert_eq!(p.display_name, "DeepSeek");
        assert_eq!(p.endpoint, "https://api.deepseek.com");
        assert_eq!(p.default_model, "deepseek-chat");
        assert_eq!(p.env_var, "DEEPSEEK_API_KEY");
        assert_eq!(p.keychain_service, "aura-deepseek");
    }

    #[test]
    fn minimax_lookup() {
        let p = lookup("minimax").expect("minimax must exist");
        assert_eq!(p.endpoint, "https://api.minimaxi.com");
        assert_eq!(p.default_model, "MiniMax-Text-01");
    }

    #[test]
    fn kimi_lookup() {
        let p = lookup("kimi").expect("kimi must exist");
        assert_eq!(p.endpoint, "https://api.moonshot.cn");
        assert_eq!(p.default_model, "moonshot-v1-8k");
    }

    #[test]
    fn custom_is_retained() {
        let p = lookup("custom").expect("custom must exist");
        assert!(p.endpoint.is_empty());
        assert!(p.default_model.is_empty());
    }

    #[test]
    fn unknown_lookup_errors() {
        let err = lookup("nope").unwrap_err();
        assert!(matches!(err, AgentError::UnknownTool(_)));
    }

    #[test]
    fn default_for_id_returns_triple() {
        let (ep, model, env) = default_for_id("deepseek").expect("deepseek");
        assert_eq!(ep, "https://api.deepseek.com");
        assert_eq!(model, "deepseek-chat");
        assert_eq!(env, "DEEPSEEK_API_KEY");
    }

    #[test]
    fn default_for_id_unknown_is_none() {
        assert!(default_for_id("nope").is_none());
    }

    #[test]
    fn catalog_invariants_hold() {
        let problems = validate_invariants();
        assert!(
            problems.is_empty(),
            "catalog invariant violations: {problems:?}"
        );
    }

    #[test]
    fn endpoints_are_base_urls_not_paths() {
        // slice 2 契约：providers.toml 的 endpoint 必须是 base URL（无 /v1），
        // 因为 HttpConfig::url() 会拼 /v1/chat/completions。
        // 若未来某 provider 的 base 含路径（如 Azure），需改 HttpConfig 或加字段。
        for p in all() {
            if p.id != "custom" {
                assert!(
                    !p.endpoint.contains("/v1"),
                    "endpoint for {} must be base URL, got {}",
                    p.id,
                    p.endpoint
                );
            }
        }
    }
}
