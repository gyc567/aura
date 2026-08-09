//! Plugin lifecycle state machine.
//!
//! Manages install/uninstall/enable/disable transitions for plugins.
//! See [`docs/plugin-spec-v2.md`](../../docs/plugin-spec-v2.md) §7.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::AgentError;
use crate::plugin::manifest::PluginManifest;

/// Plugin installation state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginState {
    /// Plugin is installed and enabled (active).
    Enabled,
    /// Plugin is installed but disabled.
    Disabled,
    /// Plugin is not installed.
    NotInstalled,
}

/// Errors that can occur during lifecycle transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LifecycleError {
    /// Plugin is already installed.
    AlreadyInstalled,
    /// Plugin is not installed.
    NotInstalled,
    /// Plugin is already in the requested state.
    AlreadyInState,
}

impl std::fmt::Display for LifecycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyInstalled => write!(f, "plugin already installed"),
            Self::NotInstalled => write!(f, "plugin not installed"),
            Self::AlreadyInState => write!(f, "plugin already in requested state"),
        }
    }
}

impl std::error::Error for LifecycleError {}

/// Plugin lifecycle manager.
///
/// Tracks installed plugins and their states. In v2, this is an in-memory store;
/// future versions may persist to disk.
#[derive(Debug, Default)]
pub struct PluginLifecycle {
    /// Map of plugin name -> (manifest, state, install path).
    plugins: HashMap<String, (PluginManifest, PluginState, PathBuf)>,
}

impl PluginLifecycle {
    /// Create a new empty lifecycle manager.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Install a plugin from a directory containing `plugin.json`.
    ///
    /// The plugin starts in [`PluginState::Enabled`].
    ///
    /// # Errors
    ///
    /// - [`LifecycleError::AlreadyInstalled`]: plugin with the same name exists.
    /// - [`AgentError::InvalidRequest`]: manifest parsing failed.
    pub fn install(&mut self, plugin_dir: PathBuf) -> Result<(), AgentError> {
        let manifest_path = plugin_dir.join("plugin.json");
        let manifest = PluginManifest::from_path(&manifest_path)?;

        if self.plugins.contains_key(&manifest.name) {
            return Err(AgentError::InvalidArguments(format!(
                "plugin `{}` is already installed",
                manifest.name
            )));
        }

        self.plugins.insert(
            manifest.name.clone(),
            (manifest, PluginState::Enabled, plugin_dir),
        );
        Ok(())
    }

    /// Uninstall a plugin by name.
    ///
    /// # Errors
    ///
    /// - [`AgentError::InvalidArguments`]: plugin not found.
    pub fn uninstall(&mut self, name: &str) -> Result<(), AgentError> {
        self.plugins.remove(name).ok_or_else(|| {
            AgentError::InvalidArguments(format!("plugin `{name}` is not installed"))
        })?;
        Ok(())
    }

    /// Enable a disabled plugin.
    ///
    /// # Errors
    ///
    /// - [`AgentError::InvalidArguments`]: plugin not found or already enabled.
    pub fn enable(&mut self, name: &str) -> Result<(), AgentError> {
        let entry = self.plugins.get_mut(name).ok_or_else(|| {
            AgentError::InvalidArguments(format!("plugin `{name}` is not installed"))
        })?;

        if entry.1 == PluginState::Enabled {
            return Err(AgentError::InvalidArguments(format!(
                "plugin `{name}` is already enabled"
            )));
        }

        entry.1 = PluginState::Enabled;
        Ok(())
    }

    /// Disable an enabled plugin.
    ///
    /// # Errors
    ///
    /// - [`AgentError::InvalidArguments`]: plugin not found or already disabled.
    pub fn disable(&mut self, name: &str) -> Result<(), AgentError> {
        let entry = self.plugins.get_mut(name).ok_or_else(|| {
            AgentError::InvalidArguments(format!("plugin `{name}` is not installed"))
        })?;

        if entry.1 == PluginState::Disabled {
            return Err(AgentError::InvalidArguments(format!(
                "plugin `{name}` is already disabled"
            )));
        }

        entry.1 = PluginState::Disabled;
        Ok(())
    }

    /// Get the state of a plugin.
    #[must_use]
    pub fn state(&self, name: &str) -> PluginState {
        self.plugins
            .get(name)
            .map_or(PluginState::NotInstalled, |(_, state, _)| *state)
    }

    /// List all installed plugins.
    #[must_use]
    pub fn list(&self) -> Vec<(&str, PluginState, &PathBuf)> {
        let mut result: Vec<(&str, PluginState, &PathBuf)> = self
            .plugins
            .iter()
            .map(|(name, (manifest, state, path))| {
                // Use manifest.name for consistency, but key should match
                let _ = manifest;
                (name.as_str(), *state, path)
            })
            .collect();
        result.sort_by(|a, b| a.0.cmp(b.0));
        result
    }

    /// Get the manifest of an installed plugin.
    #[must_use]
    pub fn manifest(&self, name: &str) -> Option<&PluginManifest> {
        self.plugins.get(name).map(|(m, _, _)| m)
    }

    /// Get the install path of an installed plugin.
    #[must_use]
    pub fn install_path(&self, name: &str) -> Option<&PathBuf> {
        self.plugins.get(name).map(|(_, _, p)| p)
    }

    /// Check if a plugin is installed (regardless of enabled/disabled state).
    #[must_use]
    pub fn is_installed(&self, name: &str) -> bool {
        self.plugins.contains_key(name)
    }

    /// Check if a plugin is enabled.
    #[must_use]
    pub fn is_enabled(&self, name: &str) -> bool {
        self.plugins
            .get(name)
            .is_some_and(|(_, state, _)| *state == PluginState::Enabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_plugin_dir(name: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        let manifest =
            format!(r#"{{"name": "{name}", "version": "1.0.0", "description": "test"}}"#);
        std::fs::write(path.join("plugin.json"), manifest).unwrap();
        (dir, path)
    }

    #[test]
    fn test_install_and_list() {
        let (_dir, path) = create_test_plugin_dir("test-plugin");
        let mut lifecycle = PluginLifecycle::new();

        lifecycle.install(path.clone()).unwrap();
        assert!(lifecycle.is_installed("test-plugin"));
        assert!(lifecycle.is_enabled("test-plugin"));

        let list = lifecycle.list();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].0, "test-plugin");
        assert_eq!(list[0].1, PluginState::Enabled);
    }

    #[test]
    fn test_install_duplicate_fails() {
        let (_dir1, path1) = create_test_plugin_dir("dup-plugin");
        // Create another dir with same plugin name
        let (_dir2, path2) = create_test_plugin_dir("dup-plugin");

        let mut lifecycle = PluginLifecycle::new();
        lifecycle.install(path1).unwrap();
        let result = lifecycle.install(path2);
        assert!(result.is_err());
    }

    #[test]
    fn test_uninstall() {
        let (_dir, path) = create_test_plugin_dir("removable");
        let mut lifecycle = PluginLifecycle::new();

        lifecycle.install(path).unwrap();
        assert!(lifecycle.is_installed("removable"));

        lifecycle.uninstall("removable").unwrap();
        assert!(!lifecycle.is_installed("removable"));
        assert_eq!(lifecycle.state("removable"), PluginState::NotInstalled);
    }

    #[test]
    fn test_uninstall_not_installed_fails() {
        let mut lifecycle = PluginLifecycle::new();
        let result = lifecycle.uninstall("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_enable_disable() {
        let (_dir, path) = create_test_plugin_dir("toggle");
        let mut lifecycle = PluginLifecycle::new();

        lifecycle.install(path).unwrap();
        assert!(lifecycle.is_enabled("toggle"));

        lifecycle.disable("toggle").unwrap();
        assert!(!lifecycle.is_enabled("toggle"));
        assert_eq!(lifecycle.state("toggle"), PluginState::Disabled);

        lifecycle.enable("toggle").unwrap();
        assert!(lifecycle.is_enabled("toggle"));
    }

    #[test]
    fn test_enable_already_enabled_fails() {
        let (_dir, path) = create_test_plugin_dir("on-plugin");
        let mut lifecycle = PluginLifecycle::new();
        lifecycle.install(path).unwrap();

        let result = lifecycle.enable("on-plugin");
        assert!(result.is_err());
    }

    #[test]
    fn test_disable_already_disabled_fails() {
        let (_dir, path) = create_test_plugin_dir("off-plugin");
        let mut lifecycle = PluginLifecycle::new();
        lifecycle.install(path).unwrap();
        lifecycle.disable("off-plugin").unwrap();

        let result = lifecycle.disable("off-plugin");
        assert!(result.is_err());
    }

    #[test]
    fn test_state_not_installed() {
        let lifecycle = PluginLifecycle::new();
        assert_eq!(lifecycle.state("ghost"), PluginState::NotInstalled);
    }

    #[test]
    fn test_manifest_retrieval() {
        let (_dir, path) = create_test_plugin_dir("manifest-test");
        let mut lifecycle = PluginLifecycle::new();
        lifecycle.install(path).unwrap();

        let m = lifecycle.manifest("manifest-test").unwrap();
        assert_eq!(m.name, "manifest-test");
        assert_eq!(m.version, "1.0.0");
    }

    #[test]
    fn test_install_invalid_manifest_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        // Write invalid manifest
        std::fs::write(path.join("plugin.json"), "not json").unwrap();

        let mut lifecycle = PluginLifecycle::new();
        let result = lifecycle.install(path);
        assert!(result.is_err());
    }

    #[test]
    fn test_install_missing_manifest_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        // No plugin.json

        let mut lifecycle = PluginLifecycle::new();
        let result = lifecycle.install(path);
        assert!(result.is_err());
    }
}
