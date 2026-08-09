//! Plugin manifest parsing and validation.
//!
//! Implements the [agent-plugins.org](https://github.com/agentplugins/agent-plugins-spec) v1.0.0 schema.
//! See [`docs/plugin-spec-v2.md`](../../docs/plugin-spec-v2.md) for the full specification.

use std::path::Path;

use serde::Deserialize;

use crate::error::AgentError;

/// Plugin manifest (`plugin.json`).
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct PluginManifest {
    /// Plugin name. Must match `^(?!.*(?:--|\.\.))[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?$`.
    pub name: String,
    /// Semantic version.
    pub version: String,
    /// Human-readable description.
    pub description: String,
    /// Author information.
    pub author: Option<Author>,
    /// Homepage URL.
    pub homepage: Option<String>,
    /// Repository URL.
    pub repository: Option<String>,
    /// SPDX license identifier.
    pub license: Option<String>,
    /// Search keywords.
    #[serde(default)]
    pub keywords: Vec<String>,
}

/// Author information in a plugin manifest.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct Author {
    /// Author name.
    pub name: String,
    /// Author email.
    pub email: Option<String>,
    /// Author URL.
    pub url: Option<String>,
}

/// Name validation: must match `^[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?$` and not contain `--` or `..`.
fn is_valid_plugin_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    // Must not contain -- or ..
    if name.contains("--") || name.contains("..") {
        return false;
    }
    // Must start and end with [a-z0-9]
    let bytes = name.as_bytes();
    let first = bytes[0];
    let last = bytes[bytes.len() - 1];
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return false;
    }
    if name.len() > 1 {
        if !last.is_ascii_lowercase() && !last.is_ascii_digit() {
            return false;
        }
        // Middle chars must be [a-z0-9.-]
        for &b in &bytes[1..bytes.len() - 1] {
            if !b.is_ascii_lowercase() && !b.is_ascii_digit() && b != b'.' && b != b'-' {
                return false;
            }
        }
    }
    true
}

impl PluginManifest {
    /// Parse and validate a `plugin.json` file.
    ///
    /// # Errors
    ///
    /// - [`AgentError::InvalidRequest`]: file not found or invalid JSON.
    /// - [`AgentError::InvalidArguments`]: name fails validation.
    pub fn from_path(path: &Path) -> Result<Self, AgentError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| AgentError::InvalidRequest(format!("read plugin.json: {e}")))?;
        Self::from_json(&content)
    }

    /// Parse and validate manifest from a JSON string.
    ///
    /// # Errors
    ///
    /// - [`AgentError::InvalidRequest`]: invalid JSON.
    /// - [`AgentError::InvalidArguments`]: name fails validation.
    pub fn from_json(json: &str) -> Result<Self, AgentError> {
        let manifest: Self = serde_json::from_str(json)
            .map_err(|e| AgentError::InvalidRequest(format!("parse plugin.json: {e}")))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Validate the manifest's invariants.
    ///
    /// # Errors
    ///
    /// - [`AgentError::InvalidArguments`]: name fails validation.
    pub fn validate(&self) -> Result<(), AgentError> {
        if !is_valid_plugin_name(&self.name) {
            return Err(AgentError::InvalidArguments(format!(
                "plugin name `{}` does not match required pattern:                  must be lowercase alphanumeric with optional . or - separators,                  cannot contain -- or ..",
                self.name
            )));
        }
        if self.version.is_empty() {
            return Err(AgentError::InvalidArguments(
                "plugin version must not be empty".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_manifest_json() -> String {
        r#"{
            "name": "my-plugin",
            "version": "1.0.0",
            "description": "A test plugin",
            "author": { "name": "Test", "email": "test@example.com" },
            "license": "MIT",
            "keywords": ["coding", "rust"]
        }"#
        .to_string()
    }

    #[test]
    fn test_parse_valid_manifest() {
        let m = PluginManifest::from_json(&valid_manifest_json()).unwrap();
        assert_eq!(m.name, "my-plugin");
        assert_eq!(m.version, "1.0.0");
        assert_eq!(m.description, "A test plugin");
        assert_eq!(m.license.as_deref(), Some("MIT"));
        assert_eq!(m.keywords, vec!["coding", "rust"]);
    }

    #[test]
    fn test_parse_invalid_json() {
        let result = PluginManifest::from_json("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_name_validation_valid() {
        for name in &["my-plugin", "foo", "a", "a1", "my.plugin", "a-b-c"] {
            let json = format!(r#"{{"name": "{name}", "version": "1.0.0", "description": "d"}}"#);
            assert!(
                PluginManifest::from_json(&json).is_ok(),
                "name `{name}` should be valid"
            );
        }
    }

    #[test]
    fn test_name_validation_invalid() {
        for name in &[
            "--foo", "foo--bar", "..foo", "foo..bar", "-foo", "foo-", "Foo", "UPPER", "",
        ] {
            let json = format!(r#"{{"name": "{name}", "version": "1.0.0", "description": "d"}}"#);
            let result = PluginManifest::from_json(&json);
            assert!(
                result.is_err(),
                "name `{name}` should be invalid but got: {result:?}"
            );
        }
    }

    #[test]
    fn test_version_empty_rejected() {
        let json = r#"{"name": "valid", "version": "", "description": "d"}"#;
        assert!(PluginManifest::from_json(json).is_err());
    }

    #[test]
    fn test_from_path_missing_file() {
        let result = PluginManifest::from_path(Path::new("/nonexistent/plugin.json"));
        assert!(result.is_err());
    }

    #[test]
    fn test_from_path_reads_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("plugin.json");
        std::fs::write(&path, valid_manifest_json()).unwrap();
        let m = PluginManifest::from_path(&path).unwrap();
        assert_eq!(m.name, "my-plugin");
    }
}
