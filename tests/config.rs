//! 配置文件端到端测试：通过 `AURA_CONFIG` 指向临时配置，验证 CLI 行为。

use std::process::Command;

fn aura() -> Command {
    Command::new(env!("CARGO_BIN_EXE_aura"))
}

/// 配置给出 `endpoint`/`model`/`api_key` 时，不带任何 CLI 参数也应走 HTTP 分支
/// （无 fake-mode 警告；错误来自连接失败而非"缺少配置"）。
#[test]
fn config_selects_http_path_without_cli_args() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = dir.path().join("config.toml");
    std::fs::write(
        &cfg,
        "endpoint = \"http://127.0.0.1:9/v1\"\nmodel = \"gpt-4o\"\napi_key = \"sk-x\"\n",
    )
    .expect("write config");

    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).expect("create ws");

    let out = aura()
        .env("AURA_CONFIG", &cfg)
        .arg("--workspace")
        .arg(&ws)
        .arg("do something")
        .output()
        .expect("run aura");

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains("running in fake mode"),
        "config should select HTTP path, got: {stderr}"
    );
    // HTTP 路径会尝试连接 127.0.0.1:9（丢弃端口）→ 连接错误，而非 fake-mode 回退。
    assert!(!out.status.success(), "HTTP connection must fail fast");
    assert!(
        stderr.contains("aura error") || stderr.to_lowercase().contains("error"),
        "expected connection error, got: {stderr}"
    );
}

/// 配置文件损坏 → fail fast（即使带 --fake-model 也报错退出）。
#[test]
fn invalid_config_fails_fast() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = dir.path().join("config.toml");
    std::fs::write(&cfg, "not = [valid toml").expect("write config");

    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).expect("create ws");

    let out = aura()
        .env("AURA_CONFIG", &cfg)
        .arg("--workspace")
        .arg(&ws)
        .arg("--fake-model")
        .arg("plan the work")
        .output()
        .expect("run aura");

    assert!(!out.status.success(), "invalid config must abort");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("parse config"),
        "expected parse-config error, got: {stderr}"
    );
}

/// 配置不存在 → 正常运行（fake-model 全链路 OK）。
#[test]
fn missing_config_is_harmless() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ws = dir.path().join("ws");
    std::fs::create_dir_all(&ws).expect("create ws");

    let out = aura()
        .env("AURA_CONFIG", dir.path().join("nope.toml"))
        .arg("--workspace")
        .arg(&ws)
        .arg("--fake-model")
        .arg("plan the work")
        .output()
        .expect("run aura");

    assert!(
        out.status.success(),
        "missing config must not break fake-model run"
    );
}
