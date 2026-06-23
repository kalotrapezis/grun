#!/usr/bin/env bash
# Build a grun .deb package. Run from the project root: packaging/build-deb.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
ARCH="$(dpkg --print-architecture)"
PKG="grun_${VERSION}_${ARCH}"
STAGE="$ROOT/build/$PKG"

echo "Building grun $VERSION ($ARCH)…"
cargo build --release

# Lay out the package tree.
rm -rf "$STAGE"
mkdir -p "$STAGE/DEBIAN" \
         "$STAGE/usr/bin" \
         "$STAGE/usr/share/applications" \
         "$STAGE/usr/share/icons/hicolor/256x256/apps" \
         "$STAGE/usr/share/icons/hicolor/512x512/apps"

install -m755 target/release/grun "$STAGE/usr/bin/grun"
install -m644 packaging/org.grun.Launcher.desktop \
        "$STAGE/usr/share/applications/org.grun.Launcher.desktop"
install -m644 Assets/AppIcon-256.png \
        "$STAGE/usr/share/icons/hicolor/256x256/apps/org.grun.Launcher.png"
install -m644 Assets/AppIcon-512.png \
        "$STAGE/usr/share/icons/hicolor/512x512/apps/org.grun.Launcher.png"

INSTALLED_KB="$(du -sk "$STAGE/usr" | cut -f1)"

cat > "$STAGE/DEBIAN/control" <<EOF
Package: grun
Version: $VERSION
Section: utils
Priority: optional
Architecture: $ARCH
Depends: libgtk-4-1 (>= 4.6), xclip, xdotool
Installed-Size: $INSTALLED_KB
Maintainer: teo <kalotrapezis@gmail.com>
Description: Fast GTK4 application launcher
 grun is a keyboard-driven launcher inspired by KRunner: search installed
 apps, files, and clipboard history in a grouped grid, with per-row actions.
 Layout-aware (matches across keyboard layouts) and fuzzy.
EOF

# Refresh icon/desktop caches after (un)install.
cat > "$STAGE/DEBIAN/postinst" <<'EOF'
#!/bin/sh
set -e
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q /usr/share/icons/hicolor || true
fi
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q /usr/share/applications || true
fi
EOF
cp "$STAGE/DEBIAN/postinst" "$STAGE/DEBIAN/postrm"
chmod 755 "$STAGE/DEBIAN/postinst" "$STAGE/DEBIAN/postrm"

fakeroot dpkg-deb --build "$STAGE" "$ROOT/build/$PKG.deb"
echo "Done: build/$PKG.deb"
