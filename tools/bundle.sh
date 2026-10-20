#!/bin/sh
# A Polter game as a Mac app: builds the game, makes the icon from a 1024 x 1024 picture, and
# puts NAME.app into target/release/; with --install also into ~/Applications, where a double
# click (or Spotlight) starts it. A console cart started from its app opens its window at once.
#
#   sh polter/tools/bundle.sh PACKAGE "Display Name" ICON.png [--install]
#
# ICON.png: 1024 x 1024 (a game's own picture, scaled by whole pixels: e.g. a 128 x 128 crop
# of a shot at 8x); tools/icon.py rounds its corners and makes every size macOS wants.
set -eu

if [ $# -lt 3 ]; then
  echo "usage: sh tools/bundle.sh PACKAGE \"Display Name\" ICON.png [--install]" >&2
  exit 3
fi
package=$1
name=$2
icon_png=$3
install=${4:-}
here=$(cd "$(dirname "$0")" && pwd)
engine=$(cd "$here/.." && pwd)
export PATH="$HOME/.cargo/bin:$PATH"

cd "$engine"
cargo build --release -p "$package"
bin="$engine/target/release/$package"
app="$engine/target/release/$name.app"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

python3 "$here/icon.py" "$icon_png" "$work/$package.iconset" >/dev/null
iconutil -c icns "$work/$package.iconset" -o "$work/$package.icns"

rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin" "$app/Contents/MacOS/$package"
cp "$work/$package.icns" "$app/Contents/Resources/$package.icns"
version=$(cargo pkgid -p "$package" | sed 's/.*[#@]//')
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>$name</string>
  <key>CFBundleDisplayName</key><string>$name</string>
  <key>CFBundleIdentifier</key><string>studio.bndr.$package</string>
  <key>CFBundleExecutable</key><string>$package</string>
  <key>CFBundleIconFile</key><string>$package</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>10.13</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
plutil -lint "$app/Contents/Info.plist" >/dev/null
echo "$app"

if [ "$install" = "--install" ]; then
  mkdir -p "$HOME/Applications"
  rm -rf "$HOME/Applications/$name.app"
  cp -R "$app" "$HOME/Applications/$name.app"
  # Finder and the Dock read the new icon
  touch "$HOME/Applications/$name.app"
  echo "$HOME/Applications/$name.app"
fi
