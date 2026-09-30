#!/usr/bin/env bash
# Regenerates the TypeScript types the frontend imports from `waypoint-protocol`
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

# Run after changing any `#[ts(export)]`-annotated type in
# `crates/waypoint-protocol/src/`. Each type names its own output directory with
# `#[ts(export, export_to = ...)]` — do not hand-edit anything under
# `apps/waypoint/src/domain/protocol/generated/`.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
cargo test -p waypoint-protocol
echo "Regenerated apps/waypoint/src/domain/protocol/generated/"
