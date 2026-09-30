#!/usr/bin/env bash
# Create a directory of many empty files, for re-measuring the file list on a large folder
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

# Usage: scripts/perf-fixture.sh [directory] [count]
# Defaults to 500 000 files in /tmp/waypoint-perf. Open the directory in a development build of
# Waypoint and run `await __waypointPerf.runAll()` in the webview (or through the Tauri MCP bridge).
# Delete it afterwards with `rm -rf`; it is on tmpfs on most systems, so it costs memory, not disk.
set -euo pipefail

dir="${1:-/tmp/waypoint-perf}"
count="${2:-500000}"

mkdir -p "${dir}"
seq -f "entry-%07g.dat" 1 "${count}" | (cd "${dir}" && xargs -n 50000 touch)
echo "Created ${count} files in ${dir}"
