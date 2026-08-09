//! Secret injection for plugin configurations.
//!
//! Replaces `${SECRET}` templates in MCP headers and env vars with actual values
//! from a secret store. Secrets are never stored plaintext in manifests.
//! See [`docs/plugin-spec-v2.md`](../../docs/plugin-spec-v2.md) §6.

use std::collections::HashMap;

use crate::error::AgentError;

/// Secret store for plugin configuration.
///
/// In v2, this is an in-memory store. Future versions may integrate with
/// system keychains or environment variables.
#[derive(Debug, Default)]
pub struct SecretStore {
    secrets: HashMap<String, String>,
}

impl SecretStore {
    /// Create an empty secret store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a secret store from environment variables.
    ///
    /// Reads all env vars and makes them available for substitution.
    #[must_use]
    pub fn from_env() -> Self {
        let mut secrets = HashMap::new();
        for (key, value) in std::env::vars() {
            secrets.insert(key, value);
        }
        Self { secrets }
    }

    /// Insert a secret into the store.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.secrets.insert(key.into(), value.into());
    }

    /// Get a secret value.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.secrets.get(key).map(String::as_str)
    }

    /// Replace `${KEY}` templates in a string with secret values.
    ///
    /// If a referenced secret is not found, returns an error.
    ///
    /// # Errors
    ///
    /// - [`AgentError::InvalidArguments`]: referenced secret not found.
    pub fn resolve(&self, template: &str) -> Result<String, AgentError> {
        let mut result = String::with_capacity(template.len());
        let mut chars = template.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch == '$' && chars.peek() == Some(&'{') {
                chars.next(); // consume '{'
                let mut key = String::new();
                let mut closed = false;
                for c in chars.by_ref() {
                    if c == '}' {
                        closed = true;
                        break;
                    }
                    key.push(c);
                }
                if !closed {
                    return Err(AgentError::InvalidArguments(format!(
                        "unclosed secret template: ${{{key}"
                    )));
                }
                match self.secrets.get(&key) {
                    Some(value) => result.push_str(value),
                    None => {
                        return Err(AgentError::InvalidArguments(format!(
                            "secret `{key}` not found"
                        )));
                    }
                }
            } else {
                result.push(ch);
            }
        }

        Ok(result)
    }

    /// Resolve all templates in a list of strings.
    ///
    /// # Errors
    ///
    /// - [`AgentError::InvalidArguments`]: any referenced secret not found.
    pub fn resolve_all(&self, templates: &[String]) -> Result<Vec<String>, AgentError> {
        templates.iter().map(|t| self.resolve(t)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_simple() {
        let mut store = SecretStore::new();
        store.insert("API_KEY", "secret123");
        let result = store.resolve("Bearer ${API_KEY}").unwrap();
        assert_eq!(result, "Bearer secret123");
    }

    #[test]
    fn test_resolve_multiple() {
        let mut store = SecretStore::new();
        store.insert("USER", "admin");
        store.insert("PASS", "hunter2");
        let result = store.resolve("${USER}:${PASS}").unwrap();
        assert_eq!(result, "admin:hunter2");
    }

    #[test]
    fn test_resolve_no_template() {
        let store = SecretStore::new();
        let result = store.resolve("plain text").unwrap();
        assert_eq!(result, "plain text");
    }

    #[test]
    fn test_resolve_missing_secret() {
        let store = SecretStore::new();
        let result = store.resolve("Bearer ${MISSING}");
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_unclosed_template() {
        let mut store = SecretStore::new();
        store.insert("KEY", "val");
        let result = store.resolve("Bearer ${KEY");
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_empty_secret() {
        let mut store = SecretStore::new();
        store.insert("EMPTY", "");
        let result = store.resolve("prefix${EMPTY}suffix").unwrap();
        assert_eq!(result, "prefixsuffix");
    }

    #[test]
    fn test_resolve_all() {
        let mut store = SecretStore::new();
        store.insert("A", "alpha");
        store.insert("B", "beta");
        let templates = vec!["key: ${A}".to_string(), "key: ${B}".to_string()];
        let results = store.resolve_all(&templates).unwrap();
        assert_eq!(results, vec!["key: alpha", "key: beta"]);
    }

    #[test]
    fn test_resolve_all_with_missing() {
        let store = SecretStore::new();
        let templates = vec!["key: ${MISSING}".to_string()];
        assert!(store.resolve_all(&templates).is_err());
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_from_env() {
        // Set a known env var for testing
        unsafe {
            std::env::set_var("AURA_TEST_SECRET", "test_value");
        }
        let store = SecretStore::from_env();
        assert_eq!(store.get("AURA_TEST_SECRET"), Some("test_value"));
        unsafe {
            std::env::remove_var("AURA_TEST_SECRET");
        }
    }

    #[test]
    fn test_get_nonexistent() {
        let store = SecretStore::new();
        assert_eq!(store.get("NONEXISTENT"), None);
    }

    #[test]
    fn test_resolve_literal_dollar() {
        let store = SecretStore::new();
        // A standalone $ (not followed by {) should pass through
        let result = store.resolve("price is $5").unwrap();
        assert_eq!(result, "price is $5");
    }

    #[test]
    fn test_resolve_adjacent_templates() {
        let mut store = SecretStore::new();
        store.insert("A", "a");
        store.insert("B", "b");
        let result = store.resolve("${A}${B}").unwrap();
        assert_eq!(result, "ab");
    }
}
