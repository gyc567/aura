# Credential Helper — GitHub HTTPS PAT in macOS Keychain

> 🌐 **Language / 语言**: [English](credential-helper.md) · [中文](credential-helper.zh.md)

## Overview

`scripts/add-github-credential.sh` adds a GitHub HTTPS Personal Access Token (PAT) to the macOS login keychain using the `security` command. After this, `git push https://github.com/...` works without manual password entry.

---

## Prerequisites

- macOS with Keychain Access
- A GitHub PAT with `repo` scope
- `security` CLI (built into macOS)

## Usage

```bash
# 1. Make the script executable
chmod +x scripts/add-github-credential.sh

# 2. Run it — you'll be prompted for the PAT
./scripts/add-github-credential.sh
```

The script will ask:

```
GitHub username (account name): gyc567
GitHub HTTPS PAT: ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
```

Type or paste the PAT when prompted. The script validates that the entry was written to the keychain before exiting.

---

## What the script does

```bash
# 1. Read inputs
read -r -p "GitHub username: " GITHUB_USER
read -r -s -p "GitHub HTTPS PAT: " GITHUB_TOKEN

# 2. Write to login keychain
security add-internet-password \
  -s "github.com" \
  -a "${GITHUB_USER}" \
  -l "git:https://github.com" \
  -T "/usr/bin/security" \
  -T "/usr/bin/git" \
  -P 443 \
  -r "htps" \
  -w "${GITHUB_TOKEN}"

# 3. Verify
security find-internet-password -s "github.com" -a "${GITHUB_USER}" -g
```

## Keychain entry created

| Field | Value |
|-------|-------|
| **Service** | `github.com` |
| **Account** | your GitHub username |
| **Label** | `git:https://github.com` |
| **Protocol** | `HTTPS` |
| **Port** | `443` |
| **Keychain** | `~/Library/Keychains/login.keychain-db` |

## Verify manually

```bash
# Find and display the entry (password shown as readable)
security find-internet-password -s "github.com" -a "gyc567" -g

# Or with label
security find-internet-password -s "github.com" -a "gyc567" -l "git:https://github.com" -g
```

## Delete the entry

```bash
security delete-internet-password -s "github.com" -a "gyc567"
```

## Troubleshooting

| Symptom | Cause | Fix |
|---------|-------|-----|
| `SecKeychainSearchCopyNext: The specified item could not be found` | The entry was not written successfully | Re-run the script and confirm no error output |
| `Device not configured` | Git credential helper conflicts | Run `git config --global credential.helper ""` then retry |
| `exit 44` from `security find-internet-password` | Wrong server/account combo | Check `security dump-keychain ~/Library/Keychains/login.keychain-db` for exact entry name |
| Push still asks for password | Remote URL uses SSH, not HTTPS | Verify remote: `git remote -v` — URL should start with `https://` |
