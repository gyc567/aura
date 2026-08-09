//! Plugin resolver: directory scanning and skill discovery.
//!
//! Scans plugin directories for `skills/*/SKILL.md` and extracts metadata.
//! See [`docs/plugin-spec-v2.md`](../../docs/plugin-spec-v2.md) §5.

use std::path::{Path, PathBuf};

use crate::error::AgentError;

/// A discovered skill from a plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredSkill {
    /// Skill name (from SKILL.md frontmatter).
    pub name: String,
    /// Skill description (from SKILL.md frontmatter).
    pub description: String,
    /// Path to the SKILL.md file.
    pub path: PathBuf,
}

/// Plugin resolver: scans directories for plugins and their skills.
#[derive(Debug, Default)]
pub struct PluginResolver {}

impl PluginResolver {
    /// Create a new resolver.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Scan a plugin directory and discover all skills.
    ///
    /// Looks for `skills/*/SKILL.md` files and extracts `name` and `description`
    /// from YAML frontmatter.
    ///
    /// # Errors
    ///
    /// - [`AgentError::InvalidRequest`]: plugin directory not found.
    pub fn discover_skills(&self, plugin_dir: &Path) -> Result<Vec<DiscoveredSkill>, AgentError> {
        let skills_dir = plugin_dir.join("skills");
        if !skills_dir.exists() {
            return Ok(Vec::new());
        }

        let mut skills = Vec::new();
        for entry in std::fs::read_dir(&skills_dir)
            .map_err(|e| AgentError::InvalidRequest(format!("read skills dir: {e}")))?
        {
            let entry =
                entry.map_err(|e| AgentError::InvalidRequest(format!("read skill entry: {e}")))?;
            let entry_dir = entry.path();
            if !entry_dir.is_dir() {
                continue;
            }

            let skill_md = entry_dir.join("SKILL.md");
            if !skill_md.exists() {
                continue;
            }

            let content = std::fs::read_to_string(&skill_md)
                .map_err(|e| AgentError::InvalidRequest(format!("read SKILL.md: {e}")))?;

            let (name, description) = parse_skill_frontmatter(&content);
            skills.push(DiscoveredSkill {
                name,
                description,
                path: skill_md,
            });
        }

        skills.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(skills)
    }

    /// Scan a base directory and discover all installed plugins.
    ///
    /// Looks for `*/plugin.json` files in the base directory.
    pub fn discover_plugins(&self, base_dir: &Path) -> Result<Vec<PathBuf>, AgentError> {
        if !base_dir.exists() {
            return Ok(Vec::new());
        }

        let mut plugins = Vec::new();
        for entry in std::fs::read_dir(base_dir)
            .map_err(|e| AgentError::InvalidRequest(format!("read plugins dir: {e}")))?
        {
            let entry =
                entry.map_err(|e| AgentError::InvalidRequest(format!("read dir entry: {e}")))?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let plugin_json = path.join("plugin.json");
            if plugin_json.exists() {
                plugins.push(path);
            }
        }

        plugins.sort();
        Ok(plugins)
    }
}

/// Parse YAML frontmatter from SKILL.md content.
///
/// Expects:
/// ```markdown
/// ---
/// name: my-skill
/// description: What this skill does
/// ---
/// ```
///
/// Returns `(name, description)`. If frontmatter is missing or malformed,
/// falls back to default values.
fn parse_skill_frontmatter(content: &str) -> (String, String) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return ("unnamed-skill".to_string(), String::new());
    }

    // Find the closing ---
    let after_open = &trimmed[3..];
    let Some(close_idx) = after_open.find("---") else {
        return ("unnamed-skill".to_string(), String::new());
    };

    let frontmatter = &after_open[..close_idx];
    let mut name = String::new();
    let mut description = String::new();

    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "name" => name = value.to_string(),
                "description" => description = value.to_string(),
                _ => {}
            }
        }
    }

    if name.is_empty() {
        name = "unnamed-skill".to_string();
    }

    (name, description)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_skills_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let resolver = PluginResolver::new();
        let skills = resolver.discover_skills(dir.path()).unwrap();
        assert!(skills.is_empty());
    }

    #[test]
    fn test_discover_skills_with_valid_skill() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("skills").join("my-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        let skill_content = "---\nname: my-skill\ndescription: A test skill\n---\n\n# My Skill\n";
        std::fs::write(skill_dir.join("SKILL.md"), skill_content).unwrap();

        let resolver = PluginResolver::new();
        let skills = resolver.discover_skills(dir.path()).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "my-skill");
        assert_eq!(skills[0].description, "A test skill");
    }

    #[test]
    fn test_discover_skills_multiple() {
        let dir = tempfile::tempdir().unwrap();

        for (name, desc) in [("alpha", "first"), ("beta", "second")] {
            let skill_dir = dir.path().join("skills").join(name);
            std::fs::create_dir_all(&skill_dir).unwrap();
            let content = format!("---\nname: {name}\ndescription: {desc}\n---\n");
            std::fs::write(skill_dir.join("SKILL.md"), content).unwrap();
        }

        let resolver = PluginResolver::new();
        let skills = resolver.discover_skills(dir.path()).unwrap();
        assert_eq!(skills.len(), 2);
        // Sorted alphabetically
        assert_eq!(skills[0].name, "alpha");
        assert_eq!(skills[1].name, "beta");
    }

    #[test]
    fn test_discover_skills_no_skills_dir() {
        let dir = tempfile::tempdir().unwrap();
        // No skills/ directory
        let resolver = PluginResolver::new();
        let skills = resolver.discover_skills(dir.path()).unwrap();
        assert!(skills.is_empty());
    }

    #[test]
    fn test_discover_skills_missing_frontmatter() {
        let dir = tempfile::tempdir().unwrap();
        let skill_dir = dir.path().join("skills").join("broken");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(skill_dir.join("SKILL.md"), "# No frontmatter\n").unwrap();

        let resolver = PluginResolver::new();
        let skills = resolver.discover_skills(dir.path()).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "unnamed-skill");
    }

    #[test]
    fn test_discover_plugins() {
        let dir = tempfile::tempdir().unwrap();

        for name in &["plugin-a", "plugin-b"] {
            let plugin_dir = dir.path().join(name);
            std::fs::create_dir_all(&plugin_dir).unwrap();
            let manifest =
                format!(r#"{{"name": "{name}", "version": "1.0.0", "description": "test"}}"#);
            std::fs::write(plugin_dir.join("plugin.json"), manifest).unwrap();
        }

        let resolver = PluginResolver::new();
        let plugins = resolver.discover_plugins(dir.path()).unwrap();
        assert_eq!(plugins.len(), 2);
    }

    #[test]
    fn test_discover_plugins_empty() {
        let dir = tempfile::tempdir().unwrap();
        let resolver = PluginResolver::new();
        let plugins = resolver.discover_plugins(dir.path()).unwrap();
        assert!(plugins.is_empty());
    }

    #[test]
    fn test_discover_plugins_nonexistent_dir() {
        let resolver = PluginResolver::new();
        let plugins = resolver.discover_skills(Path::new("/nonexistent")).unwrap();
        assert!(plugins.is_empty());
    }

    #[test]
    fn test_parse_skill_frontmatter_valid() {
        let content = "---\nname: test-skill\ndescription: Does testing\n---\n\nBody here";
        let (name, desc) = parse_skill_frontmatter(content);
        assert_eq!(name, "test-skill");
        assert_eq!(desc, "Does testing");
    }

    #[test]
    fn test_parse_skill_frontmatter_no_frontmatter() {
        let content = "# Just a heading\nNo frontmatter here.";
        let (name, desc) = parse_skill_frontmatter(content);
        assert_eq!(name, "unnamed-skill");
        assert!(desc.is_empty());
    }

    #[test]
    fn test_parse_skill_frontmatter_missing_name() {
        let content = "---\ndescription: No name here\n---\n";
        let (name, desc) = parse_skill_frontmatter(content);
        assert_eq!(name, "unnamed-skill");
        assert_eq!(desc, "No name here");
    }
}
