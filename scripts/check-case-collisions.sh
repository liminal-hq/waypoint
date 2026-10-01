#!/usr/bin/env bash
# Check no two tracked source files share a module name that differs only by case
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

# `TabMenus.tsx` and `tabMenus.ts` are two modules on Linux and one on Windows and macOS, where the
# file system ignores case: `tsc` rejects the imports there while the Rust build still succeeds and
# embeds a stale `dist`. A module name is the path without its extension, compared in lower case.
# Exits 1 and lists the colliding files; 0 if clean.
set -euo pipefail

cd "$(git rev-parse --show-toplevel 2>/dev/null || dirname "$(dirname "${BASH_SOURCE[0]}")")"

# Two files with the same name and different extensions (`a.ts`, `a.module.css`) are not a collision.
# Only the code modules are compared; each is keyed by its lower-cased path without the extension.
keys="$(git ls-files -- '*.ts' '*.tsx' '*.js' '*.jsx' '*.mjs' '*.rs' \
  | sed -E 's/\.[^./]+$//' | tr 'A-Z' 'a-z' | sort | uniq -d)"

# A repeated key is only a problem when the original spellings differ (a `.ts` beside a `.test.ts`
# has a different key, and the same file never appears twice), so list the files behind each key.
status=0
while IFS= read -r key; do
  [ -z "$key" ] && continue
  files="$(git ls-files -- '*.ts' '*.tsx' '*.js' '*.jsx' '*.mjs' '*.rs' \
    | awk -v k="$key" '{ f=$0; sub(/\.[^./]+$/, "", f); if (tolower(f) == k) print $0 }')"
  spellings="$(printf '%s\n' "$files" | sed -E 's/\.[^./]+$//' | sort -u | wc -l)"
  if [ "$spellings" -gt 1 ]; then
    echo "Module names that differ only by case (they are one file on Windows and macOS):"
    printf '  %s\n' $files
    status=1
  fi
done <<< "$keys"

exit "$status"
