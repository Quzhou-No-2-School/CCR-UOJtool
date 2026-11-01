#!/usr/bin/env bash
# 校验触发 tag（GITHUB_REF_NAME，形如 v0.1.0）与三处版本号一致。
# 用法：GITHUB_REF_NAME=v0.1.0 bash .github/scripts/check-tag-version.sh（需在仓库根目录运行）
set -euo pipefail

REF="${GITHUB_REF_NAME:-}"
if [[ ! "$REF" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "::error::GITHUB_REF_NAME 必须形如 vX.Y.Z：'${REF}'" >&2
  exit 1
fi
TAG="${REF#v}"
CONF=$(node -p "require('./src-tauri/tauri.conf.json').version")
PKG=$(node -p "require('./package.json').version")
CARGO=$(sed -n 's/^version = "\(.*\)"/\1/p' src-tauri/Cargo.toml | head -1)

echo "tag=v$TAG tauri.conf.json=$CONF package.json=$PKG Cargo.toml=$CARGO"

if [ "$TAG" != "$CONF" ] || [ "$TAG" != "$PKG" ] || [ "$TAG" != "$CARGO" ]; then
  echo "::error::版本不一致：tag v$TAG ≠ tauri.conf.json($CONF) / package.json($PKG) / Cargo.toml($CARGO)" >&2
  exit 1
fi
echo "版本一致：v$TAG"
