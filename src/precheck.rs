//! 命令前置预检（regex 版，Phase 2）。
//!
//! 设计参考 [`docs/coding-agent-design.md`](../../docs/coding-agent-design.md) §5.3.2。
//!
//! 5 类高危形态（v1 全部命中即升级 `RiskTier::High`）：
//! 1. 递归删除：`rm -rf` / `rm -fr` / `rm -Rf` 等。
//! 2. 设备写入：`/dev/sd*` / `/dev/nvme*` / `/dev/disk*` / `/dev/hd*`。
//! 3. 反弹 shell：`bash -i`、heredoc+`/dev/tcp/`、mkfifo+nc、Python 反向 socket。
//! 4. 网络下载即执行：`curl ... | sh` / `wget ... | sh` / `base64 -d ... | sh`。
//! 5. 系统目录修改：`/etc/`、`/boot/`、`/usr/`、`/var/`。
//!
//! Phase 3+ 评估是否升级为 fast model 预检；v1 仅 regex。

use std::path::PathBuf;
use std::sync::LazyLock;

use regex::Regex;

use crate::error::AgentError;

/// 风险等级。`High` 默认拒绝（参见 `policy.rs`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskTier {
    /// 无害。
    Low,
    /// 中等（可执行但有副作用）。
    Medium,
    /// 高危，必须被显式允许。
    High,
}

impl RiskTier {
    /// 数值化以便 `max` 比较（v1 简化为 Low=0, Medium=1, High=2）。
    #[must_use]
    pub fn rank(self) -> u8 {
        match self {
            Self::Low => 0,
            Self::Medium => 1,
            Self::High => 2,
        }
    }
}

/// 预检结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrecheckResult {
    /// 风险等级。
    pub tier: RiskTier,
    /// 命中的高危类别（人类可读）。
    pub categories: Vec<String>,
    /// 从 argv 中提取的文件路径（仅做记录，不参与阻断）。
    pub paths: Vec<PathBuf>,
}

// 5 类高危 regex。`std::sync::LazyLock` 自 Rust 1.80 稳定，无需 once_cell 依赖。

static RE_RM_RF: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\brm\s+(-\s*[rR]\s*[fF]\b|-[rRfF]{2,}\b|--recursive\b)").expect("valid regex")
});

static RE_DEVICE_WRITE: LazyLock<Regex> = LazyLock::new(|| {
    // 匹配写入块设备的意图：(sudo|dd|cat|echo) ... /dev/(sd|nvme|disk|hd)X，
    // 通过 `>` / `>>` / `tee` / `of=` / `to=` 任一重定向形式。
    Regex::new(
        r"(?:\bsudo\b|\bdd\b|\bcat\b|\becho\b)[^|;&]*/dev/(?:sd[a-z]+|nvme[0-9]+[a-z0-9]*|disk[0-9]+[a-z0-9]*|hd[a-z]+)|(?:>|>>|tee|\bof=|\bto=)\s*/dev/(?:sd[a-z]+|nvme[0-9]+[a-z0-9]*|disk[0-9]+[a-z0-9]*|hd[a-z]+)",
    )
    .expect("valid regex")
});

static RE_REVERSE_SHELL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\bbash\s+-i\b|bash\s+-c\s+.*/dev/tcp/|mkfifo.*\|\s*nc\b|\bnc\b.*-e\s*/?bin/(?:ba)?sh\b|python[23]?\s+-c\s+.*socket.*subprocess|perl\s+-e\s+.*socket",
    )
    .expect("valid regex")
});

static RE_DOWNLOAD_EXEC: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?:curl|wget)\b[^|]*\|\s*(?:ba)?sh\b|\bbase64\b[^|]*-d[^|]*\|\s*(?:ba)?sh\b|\beval\s+\$\(.*(?:curl|wget).*\)",
    )
    .expect("valid regex")
});

static RE_SYSTEM_DIR: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"\b(?:rm|mv|cp|chmod|chown|install|tee|dd)\s+[^|;&]*(?:/etc|/boot|/usr|/var)/[^\s|;&]*",
    )
    .expect("valid regex")
});

static RE_PATH_TOKEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:^|[\s=&])(/[^\s|;&']+)").expect("valid regex"));

/// 对一条命令做预检。`argv` 是命令 + 参数数组（已 shell 解析）。
///
/// # Errors
///
/// 当前实现不返回错误；未来若加入语义校验（如最大长度、字符集限制）会从
/// 此处返回 [`AgentError::InvalidArguments`]。
pub fn analyze(argv: &[String]) -> Result<PrecheckResult, AgentError> {
    let joined = argv.join(" ");
    let mut categories: Vec<&'static str> = Vec::new();
    let mut tier = RiskTier::Low;

    let mut flag = |cat: &'static str, regex: &Regex, target_tier: RiskTier| {
        if regex.is_match(&joined) {
            categories.push(cat);
            if target_tier.rank() > tier.rank() {
                tier = target_tier;
            }
        }
    };

    flag("recursive-remove", &RE_RM_RF, RiskTier::High);
    flag("device-write", &RE_DEVICE_WRITE, RiskTier::High);
    flag("reverse-shell", &RE_REVERSE_SHELL, RiskTier::High);
    flag("download-and-exec", &RE_DOWNLOAD_EXEC, RiskTier::High);
    flag("system-dir", &RE_SYSTEM_DIR, RiskTier::High);

    let paths: Vec<PathBuf> = RE_PATH_TOKEN
        .find_iter(&joined)
        .map(|m| PathBuf::from(m.as_str().trim()))
        .filter(|p| p.components().count() > 1) // 过滤单个 "/"
        .collect();

    Ok(PrecheckResult {
        tier,
        categories: categories.into_iter().map(String::from).collect(),
        paths,
    })
}
