#!/usr/bin/env bash
# Builds dist/yarnfoundry_<version>_amd64.deb and, when `appimagetool` is on PATH (or APPIMAGETOOL points
# at it), dist/YarnFoundry-<version>-x86_64.AppImage. Unsigned. Run from anywhere on Linux x86_64.
set -euo pipefail
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
here="$root/packaging/linux"
cd "$root"

version="$(grep -m1 -E '^version[[:space:]]*=' Cargo.toml | sed -E 's/^version[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/')"
[ -n "$version" ] || { echo "cannot read the version from Cargo.toml" >&2; exit 1; }
cargo build --release --locked -p yarnfoundry
bin="$root/target/release/yarnfoundry"
dist="$root/dist"
mkdir -p "$dist"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

# .deb
deb="$work/deb"
install -Dm755 "$bin" "$deb/usr/bin/yarnfoundry"
install -Dm644 "$here/yarnfoundry.desktop" "$deb/usr/share/applications/yarnfoundry.desktop"
install -Dm644 LICENSE-MIT "$deb/usr/share/doc/yarnfoundry/LICENSE-MIT"
install -Dm644 LICENSE-APACHE "$deb/usr/share/doc/yarnfoundry/LICENSE-APACHE"
mkdir -p "$deb/DEBIAN"
cat >"$deb/DEBIAN/control" <<CONTROL
Package: yarnfoundry
Version: $version
Section: graphics
Priority: optional
Architecture: amd64
Depends: libc6, libgcc-s1, libxkbcommon-x11-0
Maintainer: StitchFoundry <noreply@users.noreply.github.com>
Description: Colorwork charts for knitting and crochet
 Type a word or drop in a picture and get a chart sized to your gauge, with
 row-by-row written instructions and a true-size printable chart.
CONTROL
dpkg-deb --root-owner-group --build "$deb" "$dist/yarnfoundry_${version}_amd64.deb"

# AppImage (optional: needs appimagetool)
tool="${APPIMAGETOOL:-$(command -v appimagetool || true)}"
if [ -n "$tool" ]; then
  app="$work/YarnFoundry.AppDir"
  install -Dm755 "$bin" "$app/usr/bin/yarnfoundry"
  install -Dm644 "$here/yarnfoundry.desktop" "$app/yarnfoundry.desktop"
  printf '#!/bin/sh\nexec "$(dirname "$(readlink -f "$0")")/usr/bin/yarnfoundry" "$@"\n' >"$app/AppRun"
  chmod 755 "$app/AppRun"
  ARCH=x86_64 "$tool" "$app" "$dist/YarnFoundry-${version}-x86_64.AppImage"
else
  echo "warning: appimagetool not found; skipping the AppImage" >&2
fi
echo "warning: these packages are unsigned" >&2
ls -l "$dist"
