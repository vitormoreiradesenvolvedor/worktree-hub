#!/usr/bin/env bash
# Builds dist/Worktree_Hub-<version>-x86_64.AppImage from a release build.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
ARCH="${ARCH:-x86_64}"
APPDIR="$ROOT/target/AppDir"
DIST="$ROOT/dist"
TOOL_CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/worktree-hub"
APPIMAGETOOL="$TOOL_CACHE/appimagetool-$ARCH.AppImage"

echo "==> building release binary"
cargo build --release

echo "==> assembling AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" \
         "$APPDIR/usr/share/applications" \
         "$APPDIR/usr/share/icons/hicolor/256x256/apps" \
         "$APPDIR/usr/share/metainfo"

install -m755 target/release/worktree-hub "$APPDIR/usr/bin/worktree-hub"

# The icon comes from the binary itself, so it can never drift from the window icon.
"$APPDIR/usr/bin/worktree-hub" --dump-icon "$APPDIR/worktree-hub.png" 256 >/dev/null
cp "$APPDIR/worktree-hub.png" "$APPDIR/usr/share/icons/hicolor/256x256/apps/worktree-hub.png"

cp assets/worktree-hub.desktop "$APPDIR/worktree-hub.desktop"
cp assets/worktree-hub.desktop "$APPDIR/usr/share/applications/worktree-hub.desktop"

cat > "$APPDIR/AppRun" <<'APPRUN'
#!/usr/bin/env bash
HERE="$(dirname "$(readlink -f "${0}")")"
export PATH="$HERE/usr/bin:$PATH"
exec "$HERE/usr/bin/worktree-hub" "$@"
APPRUN
chmod +x "$APPDIR/AppRun"

if [ ! -x "$APPIMAGETOOL" ]; then
  echo "==> fetching appimagetool"
  mkdir -p "$TOOL_CACHE"
  curl -fsSL -o "$APPIMAGETOOL" \
    "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-$ARCH.AppImage"
  chmod +x "$APPIMAGETOOL"
fi

mkdir -p "$DIST"
OUT="$DIST/Worktree_Hub-$VERSION-$ARCH.AppImage"
echo "==> packing $OUT"
# extract-and-run keeps this working on machines without FUSE (containers, CI).
APPIMAGE_EXTRACT_AND_RUN=1 ARCH="$ARCH" "$APPIMAGETOOL" --no-appstream "$APPDIR" "$OUT"
chmod +x "$OUT"

echo
echo "pronto: $OUT"
