#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="${DIST_DIR:-$ROOT_DIR/dist}"
ARCH="$(uname -m)"
TAG="${NIGHTLY_TAG:-nightly}"
PKG_NAME="tntron-${TAG}-linux-${ARCH}"
STAGE_DIR="$DIST_DIR/$PKG_NAME"
GIFS_SRC="${TNTRON_GIFS_DIR:-}"

if [ -z "$GIFS_SRC" ]; then
  if [ -d "$ROOT_DIR/assets/gifs" ]; then
    GIFS_SRC="$ROOT_DIR/assets/gifs"
  elif [ -d "$HOME/Pictures/gifs" ]; then
    GIFS_SRC="$HOME/Pictures/gifs"
  fi
fi

echo "==> Building tntron release binaries..."
cd "$ROOT_DIR"
if command -v nix-shell >/dev/null 2>&1 && [ -f "$ROOT_DIR/shell.nix" ] && [ -z "${IN_NIX_SHELL:-}" ]; then
  nix-shell "$ROOT_DIR/shell.nix" --run "cargo build --release --workspace --offline || cargo build --release --workspace"
else
  cargo build --release --workspace --offline || cargo build --release --workspace
fi

rm -rf "$STAGE_DIR"
mkdir -p "$STAGE_DIR/share/applications" "$STAGE_DIR/share/icons" "$STAGE_DIR/docs" "$STAGE_DIR/extensions" "$STAGE_DIR/gifs"

cp "$ROOT_DIR/target/release/tntron" "$STAGE_DIR/tntron"
cp "$ROOT_DIR/target/release/tntron-ctl" "$STAGE_DIR/tntron-ctl"
chmod +x "$STAGE_DIR/tntron" "$STAGE_DIR/tntron-ctl"

cp "$ROOT_DIR/LICENSE" "$STAGE_DIR/LICENSE"
cp "$ROOT_DIR/README.md" "$STAGE_DIR/README.md"
cp -r "$ROOT_DIR/docs/." "$STAGE_DIR/docs/" 2>/dev/null || true
cp -r "$ROOT_DIR/extensions/." "$STAGE_DIR/extensions/"
cp "$ROOT_DIR/assets/tntron.desktop" "$STAGE_DIR/share/applications/tntron.desktop"
cp -r "$ROOT_DIR/assets/icons/hicolor" "$STAGE_DIR/share/icons/"
cp "$ROOT_DIR/assets/icons/tntron-logo.svg" "$STAGE_DIR/share/tntron-logo.svg"

GIF_COUNT=0
if [ -n "$GIFS_SRC" ] && [ -d "$GIFS_SRC" ]; then
  echo "==> Bundling GIFs from $GIFS_SRC..."
  cp -r "$GIFS_SRC/." "$STAGE_DIR/gifs/"
  GIF_COUNT="$(find "$STAGE_DIR/gifs" -type f -iname '*.gif' | wc -l)"
else
  echo "WARN: No GIF source directory found; nightly bundle will ship without preloaded GIFs." >&2
fi

cat > "$STAGE_DIR/install.sh" << 'EOF'
#!/usr/bin/env bash
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${PREFIX:-$HOME/.local}"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/tntron"
APPS_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
ICONS_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/icons"

mkdir -p "$PREFIX/bin" "$DATA_DIR/gifs" "$DATA_DIR/extensions" "$APPS_DIR" "$ICONS_DIR"

install -Dm755 "$HERE/tntron" "$PREFIX/bin/tntron"
install -Dm755 "$HERE/tntron-ctl" "$PREFIX/bin/tntron-ctl"
cp -r "$HERE/share/icons/hicolor" "$ICONS_DIR/"
install -Dm644 "$HERE/share/applications/tntron.desktop" "$APPS_DIR/tntron.desktop"

if [ -d "$HERE/gifs" ]; then
  cp -rn "$HERE/gifs/." "$DATA_DIR/gifs/" 2>/dev/null || cp -r "$HERE/gifs/." "$DATA_DIR/gifs/"
fi

if [ -d "$HERE/extensions" ]; then
  cp -rn "$HERE/extensions/." "${XDG_CONFIG_HOME:-$HOME/.config}/tntron/extensions/" 2>/dev/null || true
fi

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$APPS_DIR" || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "$ICONS_DIR/hicolor" || true
fi

echo "Installed tntron to $PREFIX/bin/tntron and GIF visualizer assets to $DATA_DIR/gifs"
EOF
chmod +x "$STAGE_DIR/install.sh"

mkdir -p "$DIST_DIR"
TARBALL_GZ="$DIST_DIR/${PKG_NAME}.tar.gz"
tar -C "$DIST_DIR" -czf "$TARBALL_GZ" "$PKG_NAME"
(cd "$DIST_DIR" && sha256sum "$(basename "$TARBALL_GZ")" > "${PKG_NAME}.sha256")

if [ -n "$GIFS_SRC" ] && [ -d "$GIFS_SRC" ]; then
  GIFS_TAR="$DIST_DIR/tntron-gifs-pack.tar.gz"
  tar -C "$STAGE_DIR" -czf "$GIFS_TAR" gifs
  (cd "$DIST_DIR" && sha256sum "$(basename "$GIFS_TAR")" >> "${PKG_NAME}.sha256")
fi

echo "==> Built $TARBALL_GZ ($GIF_COUNT GIFs bundled)"

if [ -x "$ROOT_DIR/minecraft-mod/gradlew" ] && command -v java >/dev/null 2>&1; then
  JAVA_MAJOR="$(java -version 2>&1 | sed -n 's/.*version "\([0-9]*\).*/\1/p' | head -n1)"
  if [ -n "$JAVA_MAJOR" ] && [ "$JAVA_MAJOR" -ge 25 ] 2>/dev/null; then
    echo "==> Building Fabric 26.2 Minecraft mod JAR..."
    "$ROOT_DIR/minecraft-mod/gradlew" -p "$ROOT_DIR/minecraft-mod" build --no-daemon
    MOD_JAR="$(find "$ROOT_DIR/minecraft-mod/build/libs" -maxdepth 1 -name 'tntron-fabric-*.jar' ! -name '*-sources.jar' | head -n1)"
    if [ -n "$MOD_JAR" ]; then
      cp "$MOD_JAR" "$DIST_DIR/"
      (cd "$DIST_DIR" && sha256sum "$(basename "$MOD_JAR")" > "$(basename "$MOD_JAR").sha256")
    fi
  fi
fi

if [ -x "$ROOT_DIR/android-app/gradlew" ] && [ -n "${ANDROID_HOME:-}" ] && [ -d "${ANDROID_HOME}/platforms/android-36" ] && command -v java >/dev/null 2>&1; then
  JAVA_MAJOR="$(java -version 2>&1 | sed -n 's/.*version "\([0-9]*\).*/\1/p' | head -n1)"
  if [ -n "$JAVA_MAJOR" ] && [ "$JAVA_MAJOR" -ge 17 ] && [ "$JAVA_MAJOR" -le 24 ] 2>/dev/null; then
    echo "==> Building Android companion APK..."
    AAPT2_ARGS=()
    if [ -x "${ANDROID_HOME}/build-tools/36.0.0/aapt2" ]; then
      AAPT2_ARGS+=("-Pandroid.aapt2FromMavenOverride=${ANDROID_HOME}/build-tools/36.0.0/aapt2")
    fi
    "$ROOT_DIR/android-app/gradlew" -p "$ROOT_DIR/android-app" assembleRelease "${AAPT2_ARGS[@]}" --no-daemon
    APK_SRC="$(find "$ROOT_DIR/android-app/app/build/outputs/apk/release" -maxdepth 1 -name '*.apk' | head -n1)"
    if [ -n "$APK_SRC" ]; then
      APK_DST="$DIST_DIR/tntron-android-remote-1.0.0.apk"
      cp "$APK_SRC" "$APK_DST"
      (cd "$DIST_DIR" && sha256sum "$(basename "$APK_DST")" > "$(basename "$APK_DST").sha256")
    fi
  fi
fi

ls -lh "$DIST_DIR"/${PKG_NAME}* "$DIST_DIR"/tntron-gifs-pack.tar.gz "$DIST_DIR"/tntron-fabric-*.jar "$DIST_DIR"/tntron-android-*.apk 2>/dev/null || true
