#!/usr/bin/env bash
# Check the Services panel lists every native plugin the app registers
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

# `apps/waypoint/src-tauri/src/lib.rs` registers each native plugin with `tauri_plugin_{name}::`;
# `apps/waypoint/src/services/serviceStatuses.ts` must have a `SERVICE_SOURCES` entry for it, so the
# panel on Settings → Integrations says whether it works (A66). Plugins that are Tauri's own
# (`log`, `os`, `opener`, `store`, `dialog`, the MCP bridge, the global shortcut: its work is reported under
# `desktop-integration`) report nothing and are skipped. A plugin used only from Rust has no guest-js
# `getStatus`, so its `SERVICE_SOURCES` entry asks an app command instead (`xdg-portal` and
# `desktop-integration` ask `get_integration_statuses`); `RUST_ONLY` is for one with no panel entry
# at all. Exits 1 and lists what is missing; 0 if every plugin is covered.
set -euo pipefail

cd "$(git rev-parse --show-toplevel 2>/dev/null || dirname "$(dirname "${BASH_SOURCE[0]}")")"

LIB="${SERVICES_LIB:-apps/waypoint/src-tauri/src/lib.rs}"
SOURCES="${SERVICES_SOURCES:-apps/waypoint/src/services/serviceStatuses.ts}"

SKIP=' log os opener store dialog mcp_bridge global_shortcut '
RUST_ONLY=' '
# A plugin whose panel name is not its crate name.
declare -A RENAMED=([waypoint_vfs]=file-system)

missing=0
for crate in $(grep -o 'tauri_plugin_[a-z_]*::' "$LIB" | sed 's/^tauri_plugin_//; s/::$//' | sort -u); do
  case "$SKIP" in *" $crate "*) continue ;; esac
  case "$RUST_ONLY" in *" $crate "*) continue ;; esac
  key="${RENAMED[$crate]:-${crate//_/-}}"
  if ! grep -Eq "^[[:space:]]*'?${key}'?:" "$SOURCES"; then
    echo "The Services panel has no entry for the '${key}' plugin (tauri_plugin_${crate} in ${LIB}); add it to SERVICE_SOURCES in ${SOURCES}." >&2
    missing=1
  fi
done

if [ "$missing" -ne 0 ]; then
  exit 1
fi
echo "Every registered plugin is in the Services panel."
