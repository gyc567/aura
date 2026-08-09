//! 路径解析：把模型/用户给的路径解析为 workspace 内的规范绝对路径。
//!
//! 统一处理两类坑（真实模型 E2E 在 macOS 上发现）：
//! 1. 符号链接：macOS `/tmp` → `/private/tmp`，直接 canonicalize 后与未规范化的
//!    workspace 比较会误报 `escapes workspace`；
//! 2. 尚不存在的目标文件（`write_file` 新建）：`canonicalize` 失败只能回退原始路径，
//!    同样导致误报。
//!
//! 解法：canonicalize 最深的已存在祖先，再拼回缺失尾段；workspace 同样 canonicalize
//! 后比较两侧，路径必须真的落在 workspace 内（顺带堵住「workspace 内符号链接指向
//! 外部」的越界）。

use std::path::{Path, PathBuf};

use crate::error::AgentError;

/// 解析并校验 workspace 内路径。
///
/// 相对路径基于 `workspace` 展开；返回符号链接已解析的规范绝对路径。
///
/// # Errors
///
/// - [`AgentError::PathPolicy`]：解析后的路径不在 workspace 内。
pub fn resolve_in_workspace(path: &Path, workspace: &Path) -> Result<PathBuf, AgentError> {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace.join(path)
    };
    let resolved = resolve_symlinks(&abs);
    let ws = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    if !resolved.starts_with(&ws) {
        return Err(AgentError::PathPolicy(format!(
            "path {} escapes workspace {}",
            path.display(),
            workspace.display()
        )));
    }
    Ok(resolved)
}

/// 规范化路径：canonicalize 最深的已存在祖先，再拼回缺失的尾段。
fn resolve_symlinks(path: &Path) -> PathBuf {
    let mut missing: Vec<std::ffi::OsString> = Vec::new();
    let mut cur = path;
    loop {
        match cur.canonicalize() {
            Ok(canonical) => {
                let mut out = canonical;
                for seg in missing.iter().rev() {
                    out.push(seg);
                }
                return out;
            }
            Err(_) => match (cur.file_name(), cur.parent()) {
                (Some(name), Some(parent)) => {
                    missing.push(name.to_os_string());
                    cur = parent;
                }
                _ => return path.to_path_buf(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_path_resolves_inside_workspace() {
        let ws = tempfile::tempdir().unwrap();
        let resolved = resolve_in_workspace(Path::new("src/main.rs"), ws.path()).unwrap();
        assert!(resolved.ends_with("src/main.rs"));
        assert!(resolved.starts_with(ws.path().canonicalize().unwrap()));
    }

    #[test]
    fn nonexistent_file_under_symlinked_workspace_ok() {
        // 模拟 macOS /tmp -> /private/tmp：workspace 本身经过符号链接。
        let real = tempfile::tempdir().unwrap();
        let link = real.path().join("link");
        std::os::unix::fs::symlink(real.path(), &link).unwrap();
        let ws = link.join("ws");
        std::fs::create_dir_all(&ws).unwrap();
        // 目标文件尚不存在（write_file 场景）。
        let target = ws.join("new_file.rs");
        let resolved = resolve_in_workspace(&target, &ws).unwrap();
        assert!(resolved.ends_with("new_file.rs"));
        assert!(resolved.starts_with(real.path().canonicalize().unwrap()));
    }

    #[test]
    fn path_outside_workspace_rejected() {
        let ws = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let err = resolve_in_workspace(&outside.path().join("x.txt"), ws.path()).unwrap_err();
        assert!(err.to_string().contains("escapes workspace"));
    }

    #[test]
    fn symlink_inside_workspace_pointing_outside_rejected() {
        let ws = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let link = ws.path().join("evil");
        std::os::unix::fs::symlink(outside.path(), &link).unwrap();
        let err = resolve_in_workspace(&link.join("secret.txt"), ws.path()).unwrap_err();
        assert!(err.to_string().contains("escapes workspace"));
    }
}
