#!/usr/bin/env bash
# Cross-build a portable Windows exe from Linux for trying on another machine
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

# Builds the frontend, cross-compiles `waypoint.exe` for x86_64-pc-windows-msvc
# with `cargo-xwin` (the same route as the other Liminal HQ apps), and stages it
# in `dist-windows-remote-dev/` with a SHA256SUMS.txt. The exe links the
# Universal CRT from Windows itself and needs only the WebView2 runtime, so no
# Visual C++ Redistributable or installer is involved.
#
# Usage: scripts/build-windows-remote-dev.sh [host]
# `host` (default: the first address from `hostname -I`) is only used for the
# printed scp line.
set -euo pipefail

cd "$(git rev-parse --show-toplevel 2>/dev/null || dirname "$(dirname "${BASH_SOURCE[0]}")")"

target="x86_64-pc-windows-msvc"
out="dist-windows-remote-dev"
host="${1:-$(hostname -I 2>/dev/null | awk '{print $1}')}"

if ! command -v cargo-xwin >/dev/null 2>&1; then
	echo "cargo-xwin is required: cargo install --locked cargo-xwin" >&2
	exit 1
fi

if ! rustup target list --installed | grep -qx "$target"; then
	rustup target add "$target"
fi

bun install --frozen-lockfile
bun run build:plugins
bun run --cwd apps/waypoint build

# `tauri/custom-protocol` embeds the frontend; without it the exe loads the dev URL.
cargo xwin build --release --target "$target" -p waypoint --features tauri/custom-protocol

exe="${CARGO_TARGET_DIR:-target}/$target/release/waypoint.exe"

rm -rf "$out"
mkdir -p "$out"
cp "$exe" "$out/waypoint.exe"
(cd "$out" && sha256sum waypoint.exe >SHA256SUMS.txt)

echo
echo "Built $(git rev-parse --short HEAD): $out/waypoint.exe ($(du -h "$out/waypoint.exe" | cut -f1))"
cat "$out/SHA256SUMS.txt"
echo
echo "From the Windows machine:"
echo "  scp -r $USER@${host:-<this-host>}:$PWD/$out ."
