#!/usr/bin/env bash
set -euo pipefail

SOURCE="/Users/chenyongxiang/Downloads/logo.png"
ICONS_DIR="$(dirname "$0")/../src-tauri/icons"
mkdir -p "$ICONS_DIR"

sq_resize() {
  local size=$1 out=$2
  magick "$SOURCE" -resize "${size}x${size}" -gravity center -background none \
    -extent "${size}x${size}" "$out"
}

for size in 32 128; do
  sq_resize "$size" "$ICONS_DIR/${size}x${size}.png"
done
sq_resize 256 "$ICONS_DIR/128x128@2x.png"

# Windows .ico
magick "$SOURCE" -resize 256x256 -gravity center -background none \
  -extent 256x256 -define icon:auto-resize=256,64,48,32,16 "$ICONS_DIR/icon.ico"

# macOS .icns
mkdir -p "$ICONS_DIR/icon.iconset"
for size in 16 32 128 256 512; do
  sq_resize "$size" "$ICONS_DIR/icon.iconset/icon_${size}x${size}.png"
  sq_resize "$((size * 2))" "$ICONS_DIR/icon.iconset/icon_${size}x${size}@2x.png"
done
iconutil -c icns "$ICONS_DIR/icon.iconset" -o "$ICONS_DIR/icon.icns"
rm -rf "$ICONS_DIR/icon.iconset"

# Windows 磁贴 SquareLogo
for size in 30 44 50 71 89 107 142 150 284 310; do
  sq_resize "$size" "$ICONS_DIR/Square${size}x${size}Logo.png"
done
sq_resize 50 "$ICONS_DIR/StoreLogo.png"

echo "done — all icons under $ICONS_DIR"
