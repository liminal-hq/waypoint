#!/usr/bin/env bash
# Check the operations engine reaches the world only through its providers and injected seams
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

# `crates/waypoint-ops/src` must not touch the file system, the process environment or the user's
# directories itself: every read and write goes through a `Provider`, and the home folder, the clock
# and the Trash arrive as injected values (A46, A48). That is what lets the same code run against the
# local provider, the in-memory provider and, later, a remote one, and what keeps every test inside
# its temporary directory. Exits 1 and lists the offending lines; 0 if clean. There is no allow-list:
# the test fixtures under `src/testing/` are held to the same rule, and tests that need a real
# directory live under `tests/`, where `tempfile` is available.
set -euo pipefail

cd "$(git rev-parse --show-toplevel 2>/dev/null || dirname "$(dirname "${BASH_SOURCE[0]}")")"

SRC="${OPS_SANDBOX_SRC:-crates/waypoint-ops/src}"

# Each pattern is an extended regular expression. `.canonicalize()` is the standard library's
# argument-less `Path` method; `Provider::canonicalize(path)` takes an argument and is allowed.
PATTERNS=(
  'std::fs'
  '\bfs::[a-z_]+\('
  'dirs::'
  'std::process'
  'std::env'
  '\benv::(var|vars|current_dir|home_dir|temp_dir|set_var)'
  'HOME'
  'XDG_'
  'tempfile'
  '\.canonicalize\(\)'
  '\.exists\(\)'
  '\.is_dir\(\)'
  '\.is_file\(\)'
  '\.is_symlink\(\)'
  '\.read_dir\('
  '\.symlink_metadata\('
  'File::(open|create)'
  'OpenOptions'
)

if [ ! -d "${SRC}" ]; then
  echo "No ${SRC} to check."
  exit 0
fi

found=0
for pattern in "${PATTERNS[@]}"; do
  if matches="$(grep -rnE --include='*.rs' -e "${pattern}" "${SRC}" || true)" && [ -n "${matches}" ]; then
    echo "Forbidden in ${SRC} (${pattern}):"
    echo "${matches}"
    found=1
  fi
done

if [ "${found}" -ne 0 ]; then
  echo
  echo "The operations engine must go through a Provider and its injected seams (see scripts/check-ops-sandbox.sh)."
  exit 1
fi

echo "The operations engine touches no file system, environment or home folder directly."
