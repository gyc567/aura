# 凭据助手 — 将 GitHub HTTPS PAT 存入 macOS Keychain

## 概述

`scripts/add-github-credential.sh` 将 GitHub HTTPS 个人访问令牌（PAT）通过 macOS 原生 `security` 命令写入登录钥匙串。此后 `git push https://github.com/...` 无需手动输入密码。

---

## 前置要求

- macOS（含 Keychain Access）
- 具有 `repo` 权限的 GitHub PAT
- `security` 命令行工具（macOS 内置）

## 使用方法

```bash
# 1. 赋予执行权限
chmod +x scripts/add-github-credential.sh

# 2. 运行脚本，按提示输入
./scripts/add-github-credential.sh
```

脚本会依次询问：

```
GitHub username (account name): gyc567
GitHub HTTPS PAT: ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
```

输入或粘贴 PAT。脚本会验证凭据写入成功后才退出。

---

## 脚本执行步骤

```bash
# 1. 读取用户输入
read -r -p "GitHub username: " GITHUB_USER
read -r -s -p "GitHub HTTPS PAT: " GITHUB_TOKEN

# 2. 写入登录钥匙串
security add-internet-password \
  -s "github.com" \
  -a "${GITHUB_USER}" \
  -l "git:https://github.com" \
  -T "/usr/bin/security" \
  -T "/usr/bin/git" \
  -P 443 \
  -r "htps" \
  -w "${GITHUB_TOKEN}"

# 3. 验证
security find-internet-password -s "github.com" -a "${GITHUB_USER}" -g
```

## 写入的钥匙串条目

| 字段 | 值 |
|------|-----|
| **服务** | `github.com` |
| **账户** | 你的 GitHub 用户名 |
| **标签** | `git:https://github.com` |
| **协议** | `HTTPS` |
| **端口** | `443` |
| **钥匙串** | `~/Library/Keychains/login.keychain-db` |

## 手动验证

```bash
# 查找并显示条目（密码会以明文输出）
security find-internet-password -s "github.com" -a "gyc567" -g

# 或通过标签查找
security find-internet-password -s "github.com" -a "gyc567" -l "git:https://github.com" -g
```

## 删除条目

```bash
security delete-internet-password -s "github.com" -a "gyc567"
```

## 故障排查

| 症状 | 原因 | 解决方法 |
|------|------|----------|
| `SecKeychainSearchCopyNext: The specified item could not be found` | 凭据未成功写入 | 重新运行脚本，确认无报错输出 |
| `Device not configured` | git 凭据助手冲突 | 运行 `git config --global credential.helper ""` 后重试 |
| `security find-internet-password` 返回 exit 44 | 服务名/账户名不匹配 | 检查 `security dump-keychain ~/Library/Keychains/login.keychain-db` 获取精确条目名 |
| Push 仍要求输入密码 | 远程地址使用 SSH 而非 HTTPS | 检查 `git remote -v`，URL 应以 `https://` 开头 |
