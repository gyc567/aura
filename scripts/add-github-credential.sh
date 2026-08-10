#!/usr/bin/env bash
# =============================================================================
# add-github-credential.sh
# Adds a GitHub HTTPS PAT to the macOS login keychain.
#
# Usage:
#   chmod +x scripts/add-github-credential.sh
#   ./scripts/add-github-credential.sh
#
# Requirements: macOS, security CLI, a GitHub PAT with 'repo' scope.
# =============================================================================

set -euo pipefail

KEYCHAIN="${HOME}/Library/Keychains/login.keychain-db"

echo "=== GitHub HTTPS PAT -> macOS Keychain ==="
echo ""

read -r -p "GitHub username (account name): " GITHUB_USER
while true; do
    read -r -s -p "GitHub HTTPS PAT: " GITHUB_TOKEN
    echo ""
    if [[ "${#GITHUB_TOKEN}" -lt 8 ]]; then
        echo "PAT looks too short. Please paste a valid GitHub token (ghp_...)."
    else
        break
    fi
done

echo ""
echo "Writing to keychain: service=github.com, account=${GITHUB_USER}, label=git:https://github.com"

security add-internet-password \
    -s "github.com" \
    -a "${GITHUB_USER}" \
    -l "git:https://github.com" \
    -T "/usr/bin/security" \
    -T "/usr/bin/git" \
    -P 443 \
    -r "htps" \
    -w "${GITHUB_TOKEN}"

echo "Verifying..."

if security find-internet-password -s "github.com" -a "${GITHUB_USER}" -g >/dev/null 2>&1; then
    echo "✓ Credential written and verified successfully."
    echo ""
    echo "Git remote should use HTTPS for this to work:"
    echo "  git remote -v"
    echo "  # origin  https://github.com/${GITHUB_USER}/<repo>.git (fetch)"
    echo ""
    echo "To push:"
    echo "  git push https://github.com/${GITHUB_USER}/<repo>.git"
else
    echo "✗ Verification failed — credential may not have been written."
    echo "  Run: security find-internet-password -s github.com -a ${GITHUB_USER} -g"
    exit 1
fi
