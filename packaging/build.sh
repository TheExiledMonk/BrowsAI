#!/usr/bin/env bash
set -euo pipefail

variant="${1:-headless}"
output_dir="${2:-dist}"

case "$variant" in
  headless)
    cargo build --release -p browsai-cli
    mkdir -p "$output_dir/headless"
    cp target/release/browsai "$output_dir/headless/browsai"
    cp -R test-sites "$output_dir/headless/test-sites"
    cp -R docs "$output_dir/headless/docs"
    cp packaging/manifest.json "$output_dir/headless/manifest.json"
    ;;
  desktop-shell)
    cargo build --release -p browsai-desktop
    mkdir -p "$output_dir/desktop-shell"
    cp target/release/libbrowsai_desktop.rlib "$output_dir/desktop-shell/libbrowsai_desktop.rlib"
    cp -R docs "$output_dir/desktop-shell/docs"
    cp packaging/manifest.json "$output_dir/desktop-shell/manifest.json"
    ;;
  *)
    echo "unsupported buildable variant: $variant" >&2
    exit 2
    ;;
esac
