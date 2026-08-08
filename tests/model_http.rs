//! `model_http` 模块集成测试。
//!
//! 覆盖：`HttpConfig` 构造、`url()` 逻辑、`HttpModelAdapter` 构造。
//! Wire 格式转换测试见 `src/model_http.rs` 的 `#[cfg(test)]` 模块。

use aura::model_http::{HttpConfig, HttpModelAdapter};

#[test]
fn http_config_new_sets_fields() {
    let cfg = HttpConfig::new("https://api.openai.com", "gpt-4o".into(), "sk-key");
    assert_eq!(cfg.endpoint, "https://api.openai.com");
    assert_eq!(cfg.model, "gpt-4o");
    assert_eq!(cfg.api_key, "sk-key");
    assert_eq!(cfg.path, "/v1/chat/completions");
}

#[test]
fn http_config_with_path_overrides_path() {
    let cfg = HttpConfig::new("https://api.openai.com", "gpt-4o".into(), "sk-key")
        .with_path("/v1/completions");
    assert_eq!(cfg.path, "/v1/completions");
}

#[test]
fn http_config_url_trims_trailing_slash_from_endpoint() {
    let cfg = HttpConfig::new("https://api.openai.com/", "gpt-4o".into(), "sk-key");
    assert_eq!(cfg.url(), "https://api.openai.com/v1/chat/completions");
}

#[test]
fn http_config_url_adds_leading_slash_to_path() {
    let cfg = HttpConfig::new("https://api.openai.com", "gpt-4o".into(), "sk-key")
        .with_path("/v1/chat/completions");
    assert_eq!(cfg.url(), "https://api.openai.com/v1/chat/completions");
}

#[test]
fn http_config_url_without_leading_slash_in_path() {
    let cfg = HttpConfig::new("https://api.openai.com", "gpt-4o".into(), "sk-key")
        .with_path("v1/chat/completions");
    assert_eq!(cfg.url(), "https://api.openai.com/v1/chat/completions");
}

#[test]
fn http_config_clone_is_independent() {
    let cfg1 = HttpConfig::new("https://api.openai.com", "gpt-4o".into(), "sk-key1");
    let cfg2 = cfg1.clone();
    assert_eq!(cfg1.url(), cfg2.url());
    let mut cfg3 = cfg2.clone();
    cfg3.endpoint = "https://other.example.com".into();
    assert_ne!(cfg1.url(), cfg3.url());
}

#[test]
fn http_model_adapter_with_client() {
    use std::net::TcpListener;
    let _listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let cfg = HttpConfig::new("https://api.openai.com", "gpt-4o".into(), "sk-key");
    let client = reqwest::Client::new();
    let _adapter = HttpModelAdapter::with_client(cfg, client);
}

#[test]
fn http_model_adapter_default_client() {
    let cfg = HttpConfig::new("https://api.openai.com", "gpt-4o".into(), "sk-key");
    let _adapter = HttpModelAdapter::new(cfg);
}
