# Aura Plugin System Design Specification (v2 Candidate)

> 🌐 **Language / 语言**: [English](plugin-spec-v2.en.md) · [中文](plugin-spec-v2.md)

- **Version**: v0.1 candidate
- **Date**: 2026-08-07
- **Status**: v2 candidate spec, **not yet implemented**
- **Source**: Extracted from [`docs/coding-agent-design.md`](coding-agent-design.md) §11 Phase 7; original location now links here
- **Reference**: [`agent-plugins/agent-plugins-spec` v1.0.0](https://github.com/agentplugins/agent-plugins-spec)

---

## 1. Goals and Scope

v2 introduces **directory-style plugins** and **MCP server** integration on top of v1. Reuses v1's capability gates and command mediation as the security foundation, avoiding new trust models.

**Out of scope for v2** (deferred to v3+): plugin signatures, source verification, enterprise controls, secret management services, dependency resolution, audit log standardization.

---

## 2. Directory Structure

```
my-plugin/
├── plugin.json          # Plugin manifest (conforms to agent-plugins.org schema v1.0.0)
└── skills/
    └── my-skill/
        └── SKILL.md     # Skill definition (isomorphic to Aura's in-project SKILL.md)
```

---

## 3. Plugin Manifest (plugin.json)

```json
{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
  "name": "my-plugin",
  "version": "1.0.0",
  "description": "My aura plugin",
  "author": { "name": "...", "email": "...", "url": "..." },
  "homepage": "...",
  "repository": "...",
  "license": "MIT",
  "keywords": ["coding", "rust"],
  "extensions": {}
}
```

**name validation regex**: `^(?!.*(?:--|\.\.))[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?$`

---

## 4. MCP Server Configuration

Supports three transports declared in `mcp.schema.json`:

| Type | Use case | Security constraint |
|------|----------|-------------------|
| `stdio` | Local process | `cwd` restricted to plugin directory; `PLUGIN_ROOT`/`PLUGIN_DATA` env vars forbidden |
| `streamable-http` | HTTP MCP endpoint | Custom headers; URL specified by user at configure time |
| `sse` | SSE push endpoint | Same as above |

```json
{
  "$schema": "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
  "mcpServers": {
    "filesystem": {
      "type": "stdio",
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-filesystem", "/workspace"],
      "env": {},
      "cwd": "./"
    },
    "http-api": {
      "type": "streamable-http",
      "url": "http://localhost:8080/mcp",
      "headers": { "Authorization": "Bearer ${MCP_API_KEY}" }
    }
  }
}
```

---

## 5. Skill Loading

1. Scan `skills/*/SKILL.md` under the plugin directory
2. Parse frontmatter: `name` + `description`
3. Register each skill as a `Tool` in the Agent's `ToolRegistry`
4. Model's tool list dynamically expands

---

## 6. Security Model (based on v1 capability gates)

| Component | Reuse v1 | v2 extension |
|-----------|-----------|---------------|
| Capability gate | ✅ `Policy::evaluate()` | Plugins declare required capabilities |
| Command mediation | ✅ `CommandMediator` | New `plugin.install` / `plugin.uninstall` |
| Environment variable isolation | New | Prevent `PLUGIN_ROOT`/`PLUGIN_DATA` leakage |
| cwd restriction | New | `cwd` must be `./` or `${PLUGIN_ROOT}/...` |
| MCP secret management | New | `${SECRET}` in headers injected at runtime; not stored in plaintext |

---

## 7. Lifecycle

| Operation | Command | State transition |
|-----------|---------|-----------------|
| Install | `aura plugin install ./my-plugin` | `Ready → PluginInstalled` |
| List | `aura plugin list` | Read-only |
| Enable/Disable | `aura plugin enable/disable <name>` | In-memory state change |
| Uninstall | `aura plugin uninstall <name>` | `PluginInstalled → Ready` |
| Update | `aura plugin update <name>` | Version check + incremental overwrite |

---

## 8. Module Layout (v2 additions)

```
src/
  plugin/
    manifest.rs     # plugin.json parsing + validation (conforms to agent-plugins.org schema)
    resolver.rs     # Plugin directory scan + skills/ discovery
    lifecycle.rs    # Install/uninstall/enable/disable state machine
    mcp.rs         # MCP server config parsing (mcp.schema.json)
    secret.rs      # Secret injection (${SECRET} template substitution)
  tools/
    plugin_install.rs
    plugin_list.rs
    plugin_uninstall.rs
```

---

## 9. Key Acceptance Criteria

- After loading a conformant plugin, its skills appear in the model's available tools list
- MCP stdio server can spawn inside the plugin directory; cwd restriction enforced
- `PLUGIN_ROOT`/`PLUGIN_DATA` env vars cannot be overridden by plugin `env`
- Secret injection: `${MY_KEY}` substituted at runtime; plaintext secret never stored in manifest

---

## 10. Relationship to Main Design Document

- Main design doc: [`coding-agent-design.md`](coding-agent-design.md) §11 Phase 7 now links here
- v1 security model source: main doc §5.3, §6
- v1 tool registration mechanism: main doc §5.3.1
