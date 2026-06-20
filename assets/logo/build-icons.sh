#!/usr/bin/env bash
# Builds all icon assets from the master SVGs using only macOS-native tools
# (qlmanage + sips + iconutil). Re-run after editing claudar-dark.svg / claudar-light.svg.
set -euo pipefail
cd "$(dirname "$0")"

render() { # <svg> <out.png> <size>
  rm -f "$2"
  qlmanage -t -s "$3" -o . "$1" >/dev/null 2>&1
  mv "$1.png" "$2"
}

echo "→ rendering 1024 masters"
render claudar-dark.svg  master-dark.png  1024
render claudar-light.svg master-light.png 1024

echo "→ PNG set (dark)"
mkdir -p png
for s in 16 32 64 128 256 512 1024; do
  sips -z "$s" "$s" master-dark.png --out "png/claudar-${s}.png" >/dev/null
done

echo "→ claudar.icns"
ICON=claudar.iconset; rm -rf "$ICON"; mkdir "$ICON"
sips -z 16   16   master-dark.png --out "$ICON/icon_16x16.png"      >/dev/null
sips -z 32   32   master-dark.png --out "$ICON/icon_16x16@2x.png"   >/dev/null
sips -z 32   32   master-dark.png --out "$ICON/icon_32x32.png"      >/dev/null
sips -z 64   64   master-dark.png --out "$ICON/icon_32x32@2x.png"   >/dev/null
sips -z 128  128  master-dark.png --out "$ICON/icon_128x128.png"    >/dev/null
sips -z 256  256  master-dark.png --out "$ICON/icon_128x128@2x.png" >/dev/null
sips -z 256  256  master-dark.png --out "$ICON/icon_256x256.png"    >/dev/null
sips -z 512  512  master-dark.png --out "$ICON/icon_256x256@2x.png" >/dev/null
sips -z 512  512  master-dark.png --out "$ICON/icon_512x512.png"    >/dev/null
cp master-dark.png "$ICON/icon_512x512@2x.png"
iconutil -c icns "$ICON" -o claudar.icns
rm -rf "$ICON"

echo "→ favicon.ico (multi-res 16/32/48) + public assets"
sips -z 48 48 master-dark.png --out fav-48.png >/dev/null
sips -z 32 32 master-dark.png --out fav-32.png >/dev/null
sips -z 16 16 master-dark.png --out fav-16.png >/dev/null
# sips can write a single .ico; bundle the 32px as the primary
sips -s format ico fav-32.png --out ../../ui/public/favicon.ico >/dev/null
cp png/claudar-32.png  ../../ui/public/favicon-32.png
cp png/claudar-512.png ../../ui/public/apple-touch-icon.png 2>/dev/null || sips -z 180 180 master-dark.png --out ../../ui/public/apple-touch-icon.png >/dev/null
rm -f fav-16.png fav-32.png fav-48.png

echo "✓ done"
ls -1 claudar.icns png/ ../../ui/public/favicon.* 2>/dev/null
