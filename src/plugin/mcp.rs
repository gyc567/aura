//! MCP server configuration parsing.
//!
//! Parses the `mcpServers` block from `plugin.json` and validates transport-specific constraints.
//! See [`docs/plugin-spec-v2.md`](../../docs/plugin-spec-v2.md) §4.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::error::AgentError;

/// MCP server configuration block (from plugin.json).
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct McpConfig {
    /// Named MCP server entries (key = server name).
    #[serde(default)]
    #[serde(rename = "mcpServers")]
    pub mcp_servers: HashMap<String, McpServerEntry>,
}

/// A single MCP server entry.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct McpServerEntry {
    /// Transport type.
    #[serde(rename = "type")]
    pub transport: McpTransport,
    /// Command to execute (stdio only).
    pub command: Option<String>,
    /// Command arguments (stdio only).
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables (stdio only). Map of variable name -> value.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Working directory (stdio only). Must be `./` or within the plugin root.
    pub cwd: Option<String>,
    /// Server URL (streamable-http / sse only).
    pub url: Option<String>,
    /// HTTP headers (streamable-http / sse only).
    #[serde(default)]
    pub headers: Vec<String>,
}

/// MCP transport types.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum McpTransport {
    /// stdio transport (local process).
    #[serde(alias = "stdio")]
    Stdio,
    /// streamable-http transport.
    #[serde(alias = "streamable-http")]
    StreamableHttp,
    /// SSE transport.
    Sse,
}

impl McpConfig {
    /// Parse MCP config from a JSON string (the `mcp` block of plugin.json).
    ///
    /// # Errors
    ///
    /// - [`AgentError::InvalidRequest`]: invalid JSON.
    pub fn from_json(json: &str) -> Result<Self, AgentError> {
        let config: Self = serde_json::from_str(json)
            .map_err(|e| AgentError::InvalidRequest(format!("parse mcp config: {e}")))?;
        Ok(config)
    }

    /// Parse MCP config from a file.
    pub fn from_path(path: &Path) -> Result<Self, AgentError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| AgentError::InvalidRequest(format!("read mcp config: {e}")))?;
        Self::from_json(&content)
    }

    /// Validate all server entries.
    ///
    /// # Errors
    ///
    /// - [`AgentError::InvalidArguments`]: transport-specific validation failed.
    pub fn validate(&self, plugin_root: &Path) -> Result<(), AgentError> {
        for (name, server) in &self.mcp_servers {
            server.validate(name, plugin_root)?;
        }
        Ok(())
    }

    /// Get all server names.
    #[must_use]
    pub fn server_names(&self) -> Vec<&str> {
        self.mcp_servers.keys().map(String::as_str).collect()
    }
}

impl McpServerEntry {
    /// Validate transport-specific constraints.
    ///
    /// # Errors
    ///
    /// - [`AgentError::InvalidArguments`]: validation failed.
    pub fn validate(&self, name: &str, plugin_root: &Path) -> Result<(), AgentError> {
        match self.transport {
            McpTransport::Stdio => {
                // stdio requires command
                if self.command.is_none() {
                    return Err(AgentError::InvalidArguments(format!(
                        "MCP server `{name}` (stdio) requires a `command`"
                    )));
                }
                // stdio must not have url
                if self.url.is_some() {
                    return Err(AgentError::InvalidArguments(format!(
                        "MCP server `{name}` (stdio) must not have a `url`"
                    )));
                }
                // cwd must be within plugin root
                if let Some(ref cwd) = self.cwd {
                    Self::validate_cwd(cwd, plugin_root, name)?;
                }
                // env must not contain PLUGIN_ROOT or PLUGIN_DATA
                for key in self.env.keys() {
                    if key == "PLUGIN_ROOT" || key == "PLUGIN_DATA" {
                        return Err(AgentError::InvalidArguments(format!(
                            "MCP server `{name}` env cannot override `PLUGIN_ROOT` or `PLUGIN_DATA`"
                        )));
                    }
                }
            }
            McpTransport::StreamableHttp | McpTransport::Sse => {
                // HTTP-based transports require url
                if self.url.is_none() {
                    return Err(AgentError::InvalidArguments(format!(
                        "MCP server `{name}` ({:?}) requires a `url`",
                        self.transport
                    )));
                }
                // HTTP-based transports must not have command/args/cwd/env
                if self.command.is_some() {
                    return Err(AgentError::InvalidArguments(format!(
                        "MCP server `{name}` ({:?}) must not have a `command`",
                        self.transport
                    )));
                }
                if !self.args.is_empty() {
                    return Err(AgentError::InvalidArguments(format!(
                        "MCP server `{name}` ({:?}) must not have `args`",
                        self.transport
                    )));
                }
                if self.cwd.is_some() {
                    return Err(AgentError::InvalidArguments(format!(
                        "MCP server `{name}` ({:?}) must not have `cwd`",
                        self.transport
                    )));
                }
                if !self.env.is_empty() {
                    return Err(AgentError::InvalidArguments(format!(
                        "MCP server `{name}` ({:?}) must not have `env`",
                        self.transport
                    )));
                }
            }
        }
        Ok(())
    }

    /// Validate that cwd is within the plugin root.
    fn validate_cwd(cwd: &str, plugin_root: &Path, name: &str) -> Result<(), AgentError> {
        // Accept "./" as a special case
        if cwd == "./" {
            return Ok(());
        }
        // Must start with "./" or be relative
        let cwd_path = Path::new(cwd);
        let resolved = if cwd_path.is_absolute() {
            cwd_path.to_path_buf()
        } else {
            plugin_root.join(cwd_path)
        };
        // Canonicalize if possible (resolves symlinks, normalizes ..)
        // If canonicalize fails (path doesn't exist), fall back to manual normalization
        let normalized = resolved
            .canonicalize()
            .unwrap_or_else(|_| normalize_path(&resolved));
        let canonical_root = plugin_root
            .canonicalize()
            .unwrap_or_else(|_| plugin_root.to_path_buf());
        if !normalized.starts_with(&canonical_root) {
            return Err(AgentError::InvalidArguments(format!(
                "MCP server `{name}` cwd `{cwd}` escapes plugin root"
            )));
        }
        Ok(())
    }
}

/// Normalize a path by resolving `.` and `..` components without requiring the path to exist.
fn normalize_path(path: &Path) -> std::path::PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {
                // Skip `.`
            }
            std::path::Component::ParentDir => {
                // `..` pops the last component if it's not `..` or a root
                if let Some(last) = components.last() {
                    if matches!(last, std::path::Component::Normal(_)) {
                        components.pop();
                    } else {
                        components.push(component);
                    }
                } else {
                    components.push(component);
                }
            }
            std::path::Component::Prefix(_)
            | std::path::Component::RootDir
            | std::path::Component::Normal(_) => {
                components.push(component);
            }
        }
    }
    components.iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_stdio_config() {
        let json = r#"{
            "mcpServers": {
                "filesystem": {
                    "type": "stdio",
                    "command": "npx",
                    "args": ["-y", "@modelcontextprotocol/server-filesystem", "/workspace"],
                    "env": {},
                    "cwd": "./"
                }
            }
        }"#;
        let config = McpConfig::from_json(json).unwrap();
        assert_eq!(config.mcp_servers.len(), 1);
        let fs = config.mcp_servers.get("filesystem").unwrap();
        assert_eq!(fs.command.as_deref(), Some("npx"));
        assert_eq!(fs.transport, McpTransport::Stdio);
    }

    #[test]
    fn test_parse_http_config() {
        let json = r#"{
            "mcpServers": {
                "http-api": {
                    "type": "streamable-http",
                    "url": "http://localhost:8080/mcp",
                    "headers": ["Authorization: Bearer token"]
                }
            }
        }"#;
        let config = McpConfig::from_json(json).unwrap();
        assert_eq!(config.mcp_servers.len(), 1);
        let api = config.mcp_servers.get("http-api").unwrap();
        assert_eq!(api.url.as_deref(), Some("http://localhost:8080/mcp"));
        assert_eq!(api.transport, McpTransport::StreamableHttp);
    }

    #[test]
    fn test_parse_sse_config() {
        let json = r#"{
            "mcpServers": {
                "legacy": {
                    "type": "sse",
                    "url": "http://localhost:9000/sse"
                }
            }
        }"#;
        let config = McpConfig::from_json(json).unwrap();
        assert_eq!(config.mcp_servers.len(), 1);
        let legacy = config.mcp_servers.get("legacy").unwrap();
        assert_eq!(legacy.transport, McpTransport::Sse);
    }

    #[test]
    fn test_stdio_requires_command() {
        let json = r#"{
            "mcpServers": {"bad": {"type": "stdio"}}
        }"#;
        let config = McpConfig::from_json(json).unwrap();
        let result = config.validate(Path::new("/tmp/plugin"));
        assert!(result.is_err());
    }

    #[test]
    fn test_stdio_rejects_url() {
        let json = r#"{
            "mcpServers": {"bad": {"type": "stdio", "command": "foo", "url": "http://x"}}
        }"#;
        let config = McpConfig::from_json(json).unwrap();
        let result = config.validate(Path::new("/tmp/plugin"));
        assert!(result.is_err());
    }

    #[test]
    fn test_stdio_rejects_plugin_root_env() {
        let json = r#"{
            "mcpServers": {"evil": {"type": "stdio", "command": "foo", "env": {"PLUGIN_ROOT": "/escaped"}}}
        }"#;
        let config = McpConfig::from_json(json).unwrap();
        let result = config.validate(Path::new("/tmp/plugin"));
        assert!(result.is_err());
    }

    #[test]
    fn test_stdio_rejects_plugin_data_env() {
        let json = r#"{
            "mcpServers": {"evil": {"type": "stdio", "command": "foo", "env": {"PLUGIN_DATA": "/escaped"}}}
        }"#;
        let config = McpConfig::from_json(json).unwrap();
        let result = config.validate(Path::new("/tmp/plugin"));
        assert!(result.is_err());
    }

    #[test]
    fn test_stdio_cwd_within_root() {
        let json = r#"{
            "mcpServers": {"good": {"type": "stdio", "command": "foo", "cwd": "./subdir"}}
        }"#;
        let config = McpConfig::from_json(json).unwrap();
        // Create a temp dir structure
        let dir = tempfile::tempdir().unwrap();
        let plugin_root = dir.path().to_path_buf();
        std::fs::create_dir_all(plugin_root.join("subdir")).unwrap();
        let result = config.validate(&plugin_root);
        assert!(result.is_ok());
    }

    #[test]
    fn test_stdio_cwd_escapes_root() {
        let json = r#"{
            "mcpServers": {"bad": {"type": "stdio", "command": "foo", "cwd": "../../etc"}}
        }"#;
        let config = McpConfig::from_json(json).unwrap();
        let result = config.validate(Path::new("/tmp/plugin"));
        assert!(result.is_err());
    }

    #[test]
    fn test_http_requires_url() {
        let json = r#"{
            "mcpServers": {"bad": {"type": "streamable-http"}}
        }"#;
        let config = McpConfig::from_json(json).unwrap();
        let result = config.validate(Path::new("/tmp/plugin"));
        assert!(result.is_err());
    }

    #[test]
    fn test_http_rejects_command() {
        let json = r#"{
            "mcpServers": {"bad": {"type": "streamable-http", "url": "http://x", "command": "foo"}}
        }"#;
        let config = McpConfig::from_json(json).unwrap();
        let result = config.validate(Path::new("/tmp/plugin"));
        assert!(result.is_err());
    }

    #[test]
    fn test_http_rejects_cwd() {
        let json = r#"{
            "mcpServers": {"bad": {"type": "sse", "url": "http://x", "cwd": "./"}}
        }"#;
        let config = McpConfig::from_json(json).unwrap();
        let result = config.validate(Path::new("/tmp/plugin"));
        assert!(result.is_err());
    }

    #[test]
    fn test_http_rejects_env() {
        let json = r#"{
            "mcpServers": {"bad": {"type": "sse", "url": "http://x", "env": {"FOO": "bar"}}}
        }"#;
        let config = McpConfig::from_json(json).unwrap();
        let result = config.validate(Path::new("/tmp/plugin"));
        assert!(result.is_err());
    }

    #[test]
    fn test_valid_stdio_config() {
        let json = r#"{
            "mcpServers": {"good": {"type": "stdio", "command": "npx", "args": ["-y", "server"], "env": {"PATH": "/usr/bin"}, "cwd": "./"}}
        }"#;
        let config = McpConfig::from_json(json).unwrap();
        let result = config.validate(Path::new("/tmp/plugin"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_valid_http_config() {
        let json = r#"{
            "mcpServers": {"good": {"type": "streamable-http", "url": "http://localhost:8080/mcp", "headers": ["Auth: Bearer xxx"]}}
        }"#;
        let config = McpConfig::from_json(json).unwrap();
        let result = config.validate(Path::new("/tmp/plugin"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_server_names() {
        let json = r#"{
            "mcpServers": {
                "alpha": {"type": "stdio", "command": "a"},
                "beta": {"type": "sse", "url": "http://b"}
            }
        }"#;
        let config = McpConfig::from_json(json).unwrap();
        let mut names = config.server_names();
        names.sort_unstable();
        assert_eq!(names, vec!["alpha", "beta"]);
    }

    #[test]
    fn test_empty_mcp_servers() {
        let json = r#"{"mcpServers": {}}"#;
        let config = McpConfig::from_json(json).unwrap();
        assert!(config.mcp_servers.is_empty());
        let result = config.validate(Path::new("/tmp/plugin"));
        assert!(result.is_ok());
    }
}
