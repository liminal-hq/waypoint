#!/usr/bin/env bash
# Regenerates every TypeScript binding that the Rust crates and plugins export
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

# Run after changing any `#[ts(export)]`-annotated type in `crates/waypoint-protocol/src/`,
# `crates/waypoint-vfs/src/` or a plugin's `src/`. Each type names its own output directory with `#[ts(export, export_to = ...)]`,
# so nothing here needs configuring. Do not hand-edit generated files: the domain bindings live in
# `packages/protocol/src/generated/` and each reusable plugin's in `guest-js/bindings/`.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
cargo test -p waypoint-protocol -p waypoint-vfs -p tauri-plugin-system-appearance -p tauri-plugin-window-manager
echo "Regenerated the protocol and plugin bindings."
