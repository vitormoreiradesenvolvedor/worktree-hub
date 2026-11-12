#!/usr/bin/env bash
# Cross-builds the Windows .exe from Linux.
#
# Preferred: cargo-xwin (MSVC target, matches the CI artifact).
#   cargo install cargo-xwin && rustup target add x86_64-pc-windows-msvc
# Fallback: the GNU target with mingw-w64 installed.
#   rustup target add x86_64-pc-windows-gnu && dnf install mingw64-gcc
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
DIST="$ROOT/dist"
mkdir -p "$DIST"

if command -v cargo-xwin >/dev/null 2>&1; then
  TARGET=x86_64-pc-windows-msvc
  echo "==> cargo xwin build --release --target $TARGET"
  cargo xwin build --release --target "$TARGET"
elif command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
  TARGET=x86_64-pc-windows-gnu
  echo "==> cargo build --release --target $TARGET"
  cargo build --release --target "$TARGET"
else
  cat >&2 <<'MSG'
Nenhum toolchain de cross-compile encontrado.

Opções:
  1. cargo install cargo-xwin && rustup target add x86_64-pc-windows-msvc
  2. dnf install mingw64-gcc && rustup target add x86_64-pc-windows-gnu
  3. deixar o GitHub Actions gerar o .exe (.github/workflows/release.yml)
MSG
  exit 1
fi

cp "target/$TARGET/release/worktree-hub.exe" "$DIST/worktree-hub.exe"
echo
echo "pronto: $DIST/worktree-hub.exe"
