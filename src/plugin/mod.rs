//! Plugin system (v2 candidate).
//!
//! Implements directory-based plugins with MCP server integration.
//! See [`docs/plugin-spec-v2.md`](../../docs/plugin-spec-v2.md) for the full specification.
//!
//! v2 modules:
//! - [`manifest`]: plugin.json parsing + validation
//! - [`lifecycle`]: install/uninstall/enable/disable state machine
//! - [`mcp`]: MCP server configuration parsing
//! - [`resolver`]: plugin directory scanning + skill discovery
//! - [`secret`]: ${SECRET} template replacement

pub mod lifecycle;
pub mod manifest;
pub mod mcp;
pub mod resolver;
pub mod secret;

pub use lifecycle::{PluginLifecycle, PluginState};
pub use manifest::{Author, PluginManifest};
pub use mcp::{McpConfig, McpServerEntry, McpTransport};
pub use resolver::{DiscoveredSkill, PluginResolver};
pub use secret::SecretStore;
