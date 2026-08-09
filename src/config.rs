//! 用户配置文件：`~/.config/aura/config.toml`（或 `AURA_CONFIG` 显式指定路径）。
//!
//! 优先级：CLI 参数 > 配置文件 > 环境变量（`AURA_API_KEY`）。
//!
//! 示例（见仓库根目录 `config.example.toml`）：
//! ```toml
//! endpoint = "https://api.openai.com/v1"
//! model = "gpt-4o"
//! # api_key = "sk-..."  # 推荐用 AURA_API_KEY 环境变量或 CLI --api-key，不要写进配置文件
//! ```
//!
//! 行为：文件不存在 → 空配置（默认值）；文件存在但解析失败 → 报错退出（fail fast）。

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::AgentError;

/// Aura 用户配置。
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Config {
    /// OpenAI-compatible endpoint URL。
    pub endpoint: Option<String>,
    /// 模型名。
    pub model: Option<String>,
    /// API key（建议用 `AURA_API_KEY` 环境变量或 `--api-key` 传入，而非写入配置文件）。
    pub api_key: Option<String>,
}

impl Config {
    /// 加载配置。路径：`AURA_CONFIG` 显式路径 > `$XDG_CONFIG_HOME/aura/config.toml`
    /// > `$HOME/.config/aura/config.toml`（Windows 用 `%USERPROFILE%`）。
    pub fn load() -> Result<Self, AgentError> {
        Self::load_from(&Self::config_path())
    }

    /// 从指定路径加载（测试与高级用法用）。文件不存在视为空配置；
    /// 文件存在但解析失败则报错（fail fast）。
    pub fn load_from(path: &Path) -> Result<Self, AgentError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)
            .map_err(|e| AgentError::Context(format!("read config {}: {e}", path.display())))?;
        toml::from_str(&content)
            .map_err(|e| AgentError::Context(format!("parse config {}: {e}", path.display())))
    }

    /// 配置文件的候选路径（不检查存在性）。
    pub fn config_path() -> PathBuf {
        if let Some(p) = std::env::var_os("AURA_CONFIG") {
            return PathBuf::from(p);
        }
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            return PathBuf::from(xdg).join("aura").join("config.toml");
        }
        let home = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from);
        match home {
            Some(h) => h.join(".config").join("aura").join("config.toml"),
            None => PathBuf::from("config.toml"),
        }
    }

    /// 合并优先级：CLI 显式值 > 配置文件 > 环境变量回退值。
    ///
    /// `cli` 为命令行参数（`Option`，未给为 `None`），`env_fallback` 为环境变量回退
    /// （如 `AURA_API_KEY`）。返回最终生效的值。
    #[must_use]
    pub fn resolve(
        cli: Option<String>,
        config: Option<String>,
        env_fallback: Option<String>,
    ) -> Option<String> {
        cli.or(config).or(env_fallback)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn write_config(dir: &std::path::Path, content: &str) -> PathBuf {
        let path = dir.join("config.toml");
        fs::write(&path, content).expect("write test config");
        path
    }

    #[test]
    fn parse_full_config() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_config(
            dir.path(),
            "endpoint = \"https://api.example.com/v1\"\nmodel = \"gpt-4o\"\napi_key = \"sk-x\"\n",
        );
        let cfg = Config::load_from(&path).expect("load ok");
        assert_eq!(
            cfg,
            Config {
                endpoint: Some("https://api.example.com/v1".into()),
                model: Some("gpt-4o".into()),
                api_key: Some("sk-x".into()),
            }
        );
    }

    #[test]
    fn missing_fields_default_to_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_config(dir.path(), "endpoint = \"https://api.example.com/v1\"\n");
        let cfg = Config::load_from(&path).expect("load ok");
        assert_eq!(cfg.endpoint.as_deref(), Some("https://api.example.com/v1"));
        assert_eq!(cfg.model, None);
        assert_eq!(cfg.api_key, None);
    }

    #[test]
    fn empty_file_is_default() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_config(dir.path(), "");
        assert_eq!(
            Config::load_from(&path).expect("load ok"),
            Config::default()
        );
    }

    #[test]
    fn missing_file_is_default() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert_eq!(
            Config::load_from(&dir.path().join("nope.toml")).expect("load ok"),
            Config::default()
        );
    }

    #[test]
    fn invalid_config_fails_fast() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = write_config(dir.path(), "not = [valid toml");
        assert!(Config::load_from(&path).is_err());
    }

    #[test]
    #[allow(unsafe_code)]
    fn config_path_prefers_aura_config_env() {
        unsafe {
            std::env::set_var("AURA_CONFIG", "/tmp/custom/aura.toml");
        }
        assert_eq!(
            Config::config_path(),
            PathBuf::from("/tmp/custom/aura.toml")
        );
    }

    #[test]
    fn resolve_precedence_cli_over_config_over_env() {
        // CLI 优先
        assert_eq!(
            Config::resolve(Some("cli".into()), Some("cfg".into()), Some("env".into())),
            Some("cli".into())
        );
        // config 次之
        assert_eq!(
            Config::resolve(None, Some("cfg".into()), Some("env".into())),
            Some("cfg".into())
        );
        // env 兜底
        assert_eq!(
            Config::resolve(None, None, Some("env".into())),
            Some("env".into())
        );
        // 全空
        assert_eq!(Config::resolve(None, None, None), None);
    }
}
