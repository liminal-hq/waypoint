#!/usr/bin/env bash
# Check every source file has the licence header AGENTS.md requires
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

# Exits 1 and lists offenders if any file is missing an
# SPDX-License-Identifier line within its first few lines; 0 if clean.
set -euo pipefail

cd "$(git rev-parse --show-toplevel 2>/dev/null || dirname "$(dirname "${BASH_SOURCE[0]}")")"

# Files that are exempt: generated output the build regenerates from
# scratch (adding a header would just create drift against the
# generator), and tool-scaffolded boilerplate nobody hand-writes.
EXEMPT_FILES=(
  "apps/*/vite.config.ts"                # build-tool config
  "apps/*/src/vite-env.d.ts"             # Vite-scaffolded ambient types
  "*/generated/*.ts"                     # ts-rs output, regenerated from Rust structs
  "plugins/*/guest-js/rollup.config.js"  # build-tool config
)

is_exempt() {
  local f="$1"
  for exempt in "${EXEMPT_FILES[@]}"; do
    # shellcheck disable=SC2053  # intentional glob match, not literal compare
    [[ "$f" == $exempt ]] && return 0
  done
  return 1
}

# Only scan directories that actually exist yet -- this list is the full
# set of places source lives; missing ones are simply skipped.
SCAN_DIRS=()
for d in apps packages plugins crates scripts tools; do
  [[ -d "$d" ]] && SCAN_DIRS+=("$d")
done

missing=()

while IFS= read -r -d '' f; do
  is_exempt "$f" && continue
  if ! head -8 "$f" | grep -q "SPDX-License-Identifier"; then
    missing+=("$f")
  # The header format is a one-line (or wrapped) purpose summary, a blank comment line, then the
  # copyright block — a file starting directly on "(c) Copyright" skipped the summary.
  elif head -1 "$f" | grep -q "(c) Copyright"; then
    missing+=("$f (missing its purpose summary before the copyright line)")
  fi
done < <(find "${SCAN_DIRS[@]}" \
  \( -name node_modules -o -name dist -o -name dist-js -o -name target -o -name gen \) -prune -o \
  \( -name "*.rs" -o -name "*.ts" -o -name "*.tsx" -o -name "*.js" -o -name "*.mjs" -o -name "*.sh" \) -print0)

if [ "${#missing[@]}" -gt 0 ]; then
  echo "Missing licence header (see AGENTS.md 'Licence and Copyright'):"
  for f in "${missing[@]}"; do
    echo "  $f"
  done
  echo
  echo "${#missing[@]} file(s) missing a header."
  exit 1
fi

echo "All source files have licence headers."
