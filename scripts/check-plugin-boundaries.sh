#!/usr/bin/env bash
# Check JavaScript reaches native plugins only through their guest-js APIs
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

# A plugin's command names are an implementation detail of its `guest-js` package. Application
# and package code must call the typed guest-js functions, never `invoke('plugin:name|command')`
# directly. Exits 1 and lists offenders; 0 if clean.
set -euo pipefail

cd "$(git rev-parse --show-toplevel 2>/dev/null || dirname "$(dirname "${BASH_SOURCE[0]}")")"

SCAN_DIRS=()
for d in apps packages; do
  [[ -d "$d" ]] && SCAN_DIRS+=("$d")
done

# Tests may mock `invoke`; production code may not call a plugin command by name.
offenders="$(grep -rnE "(invoke|invokeCommand)[^(]*\((\s*)[\`'\"]plugin:" "${SCAN_DIRS[@]}" \
  --include='*.ts' --include='*.tsx' --include='*.js' --include='*.mjs' \
  --exclude-dir=node_modules --exclude-dir=dist --exclude-dir=generated --exclude-dir=gen \
  --exclude='*.test.ts' --exclude='*.test.tsx' || true)"

if [ -n "$offenders" ]; then
  echo "Direct plugin invocation found (see AGENTS.md 'Tauri v2'). Use the plugin's guest-js API:"
  echo "$offenders"
  exit 1
fi

echo "JavaScript reaches native plugins only through guest-js."
