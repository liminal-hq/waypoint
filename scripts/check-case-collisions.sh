#!/usr/bin/env bash
# Check no two tracked source files can be mistaken for one another on a case-insensitive file system
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

# `TabMenus.tsx` and `tabMenus.ts` are two modules on Linux and one on Windows and macOS, where the
# file system ignores case: `tsc` rejects the imports there while the Rust build still succeeds and
# embeds a stale `dist`. The extensionless `import './TabMenus'` resolves against every script
# extension, so the TypeScript and JavaScript family (`ts`, `tsx`, `js`, `jsx`, `mjs`, `cjs`) shares
# one module namespace: two of those files collide when their paths match without the extension in
# lower case but not exactly. Any other extension is only ever opened by its full name, so it
# collides only with a file of the same extension (`Foo.rs` and `foo.rs`); `Foo.rs` beside `foo.ts`,
# or `a.ts` beside `a.module.css`, is fine.
# Exits 1 and lists the colliding files; 0 if clean.
#
# `--stdin` reads the NUL-separated file list from standard input instead of `git ls-files` (tests).
set -euo pipefail

cd "$(git rev-parse --show-toplevel 2>/dev/null || dirname "$(dirname "${BASH_SOURCE[0]}")")"

list_files() {
  if [ "${1:-}" = "--stdin" ]; then
    cat
  else
    git ls-files -z -- '*.ts' '*.tsx' '*.js' '*.jsx' '*.mjs' '*.cjs' '*.rs'
  fi
}

# Each file is keyed by its lower-cased stem plus a namespace (`script` for the module family, the
# extension itself otherwise). A key shared by files whose original stems differ is a collision.
list_files "${1:-}" | awk -v RS='\0' '
  $0 == "" { next }
  {
    path = $0
    stem = path; sub(/\.[^.\/]+$/, "", stem)
    ext = path; sub(/^.*\./, "", ext)
    ns = (ext ~ /^(ts|tsx|js|jsx|mjs|cjs)$/) ? "script" : ext
    key = tolower(stem) "\t" ns
    if (!(key in first)) first[key] = stem
    files[key] = (key in files) ? files[key] "\n" path : path
    if (stem != first[key]) bad[key] = 1
  }
  END {
    for (key in bad) {
      if (!header) { print "Module names that differ only by case (they are one file on Windows and macOS):"; header = 1 }
      n = split(files[key], list, "\n")
      for (i = 1; i <= n; i++) print "  " list[i]
      status = 1
    }
    exit status
  }
'
