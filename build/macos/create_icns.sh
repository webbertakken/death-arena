#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
iconset_dir="${script_dir}/AppIcon.iconset"

mkdir -p "${iconset_dir}"
find "${iconset_dir}" -mindepth 1 -delete

sips -z 16 16 "${script_dir}/icon_1024x1024.png" --out "${iconset_dir}/icon_16x16.png"
sips -z 32 32 "${script_dir}/icon_1024x1024.png" --out "${iconset_dir}/icon_16x16@2x.png"
sips -z 32 32 "${script_dir}/icon_1024x1024.png" --out "${iconset_dir}/icon_32x32.png"
sips -z 64 64 "${script_dir}/icon_1024x1024.png" --out "${iconset_dir}/icon_32x32@2x.png"
sips -z 128 128 "${script_dir}/icon_1024x1024.png" --out "${iconset_dir}/icon_128x128.png"
sips -z 256 256 "${script_dir}/icon_1024x1024.png" --out "${iconset_dir}/icon_128x128@2x.png"
sips -z 256 256 "${script_dir}/icon_1024x1024.png" --out "${iconset_dir}/icon_256x256.png"
sips -z 512 512 "${script_dir}/icon_1024x1024.png" --out "${iconset_dir}/icon_256x256@2x.png"
sips -z 512 512 "${script_dir}/icon_1024x1024.png" --out "${iconset_dir}/icon_512x512.png"

cp "${script_dir}/icon_1024x1024.png" "${iconset_dir}/icon_512x512@2x.png"
cp "${iconset_dir}/icon_256x256.png" "${script_dir}/../../assets/textures/app_icon.png"
iconutil -c icns "${iconset_dir}" -o "${script_dir}/AppIcon.icns"
mkdir -p "${script_dir}/src/Game.app/Contents/Resources"
mv "${script_dir}/AppIcon.icns" "${script_dir}/src/Game.app/Contents/Resources/"
