#!/usr/bin/env bash
# Aura 一键安装脚本：检测系统 → 下载对应平台的 release 资产 → 解压 → 装到 PATH。
#
# 用法：
#   curl -sSL https://raw.githubusercontent.com/gyc567/aura/main/release/install.sh | bash
#
# 可覆盖的环境变量：
#   AURA_REPO        GitHub 仓库（默认 gyc567/aura）
#   AURA_VERSION     版本 tag，如 v0.1.0（默认 latest）
#   AURA_INSTALL_DIR 安装目录（默认 $HOME/.local/bin）
#   AURA_RELEASE_URL release 下载基址（默认 https://github.com/<repo>/releases/<version>/download）
#   AURA_SHA256      期望的资产 SHA256（可选；设置后强制校验）

set -euo pipefail

REPO="${AURA_REPO:-gyc567/aura}"
VERSION="${AURA_VERSION:-latest}"
INSTALL_DIR="${AURA_INSTALL_DIR:-$HOME/.local/bin}"
BASE_URL="${AURA_RELEASE_URL:-https://github.com/${REPO}/releases/${VERSION}/download}"

# 1. 检测操作系统
case "$(uname -s)" in
  Linux*)  OS=linux ;;
  Darwin*) OS=macos ;;
  MINGW* | MSYS* | CYGWIN*) OS=windows ;;
  *) echo "不支持的操作系统: $(uname -s)" >&2; exit 1 ;;
esac

# 2. 检测架构
case "$(uname -m)" in
  x86_64 | amd64)  ARCH=x64 ;;
  arm64 | aarch64) ARCH=arm64 ;;
  *) echo "不支持的架构: $(uname -m)" >&2; exit 1 ;;
esac

# 3. 映射到 release 资产名
case "${OS}-${ARCH}" in
  linux-x64)   ARTIFACT=aura-linux-x64.tar.gz ;;
  linux-arm64) ARTIFACT=aura-linux-arm64.tar.gz ;;
  macos-x64)   ARTIFACT=aura-macos-x64.tar.gz ;;
  macos-arm64) ARTIFACT=aura-macos-arm64.tar.gz ;;
  windows-x64) ARTIFACT=aura-windows-x64.zip ;;
  *) echo "暂不支持 ${OS}-${ARCH}" >&2; exit 1 ;;
esac

DEST="${INSTALL_DIR}/aura"
URL="${BASE_URL}/${ARTIFACT}"

echo "安装 aura ${VERSION} (${OS}-${ARCH}) → ${DEST}"
mkdir -p "${INSTALL_DIR}"

TMP="$(mktemp -d)"
trap 'rm -rf "${TMP}"' EXIT

case "${ARTIFACT}" in
  *.tar.gz)
    # 选项在前，-- 紧贴 URL：防止 URL 以 - 开头被解析为选项
    curl -fsSL -o "${TMP}/pkg.tar.gz" -- "${URL}"
    ;;
  *.zip)
    curl -fsSL -o "${TMP}/pkg.zip" -- "${URL}"
    ;;
esac

# 可选 SHA256 校验（设置 AURA_SHA256 时强制）
if [[ -n "${AURA_SHA256:-}" ]]; then
  # 优先 shasum：macOS 自带的 /sbin/sha256sum 不支持 -c，而 shasum 支持；
  # Linux 无 shasum 时退回 GNU sha256sum。
  if command -v shasum >/dev/null 2>&1; then
    CHECKER="shasum -a 256 -c"
  else
    CHECKER="sha256sum -c"
  fi
  # ${ARTIFACT#*.} = 去掉首段（aura-macos-arm64）→ tar.gz / zip，对应下载文件名 pkg.<ext>
  echo "${AURA_SHA256}  ${TMP}/pkg.${ARTIFACT#*.}" | ${CHECKER} >/dev/null 2>&1 || {
    echo "错误：SHA256 校验失败（期望 ${AURA_SHA256}）" >&2
    exit 1
  }
fi

case "${ARTIFACT}" in
  *.tar.gz)
    tar -xzf "${TMP}/pkg.tar.gz" -C "${TMP}" aura
    cp "${TMP}/aura" "${DEST}"
    ;;
  *.zip)
    # 只提取单条目，避免 zip-slip / 解压炸弹
    unzip -q -j "${TMP}/pkg.zip" "aura.exe" -d "${TMP}"
    cp "${TMP}/aura.exe" "${DEST}"
    ;;
esac

chmod +x "${DEST}"

# 4. PATH 检查：不在 PATH 中时给出加入提示
case ":${PATH}:" in
  *":${INSTALL_DIR}:"*) ;;
  *)
    echo "提示：${INSTALL_DIR} 不在 PATH 中。请把它加进去："
    case "${SHELL:-}" in
      *zsh)  echo "  echo 'export PATH=\"${INSTALL_DIR}:\$PATH\"' >> ~/.zshrc && source ~/.zshrc" ;;
      *bash) echo "  echo 'export PATH=\"${INSTALL_DIR}:\$PATH\"' >> ~/.bashrc && source ~/.bashrc" ;;
      *)     echo "  export PATH=\"${INSTALL_DIR}:\$PATH\"" ;;
    esac
    ;;
esac

echo "已安装: ${DEST}"
"${DEST}" --version
