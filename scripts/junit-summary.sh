#!/usr/bin/env bash
# Print a Markdown table of test totals from JUnit XML files, for a job summary
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

# Usage: scripts/junit-summary.sh <file.xml>...
# Reads the `tests`, `failures` and `errors` attributes of each file's root <testsuites> element
# (Vitest and cargo-nextest both write one) and prints one row per file plus a total. Missing
# files are listed as "not found" rather than failing, so a summary is still produced when a job
# dies before writing results.
set -euo pipefail

echo "| Report | Tests | Failures | Errors |"
echo "|---|---:|---:|---:|"

total_tests=0
total_failures=0
total_errors=0

for file in "$@"; do
  if [ ! -f "$file" ]; then
    echo "| \`${file}\` | not found | | |"
    continue
  fi
  root="$(grep -m1 -o '<testsuites[^>]*>' "$file" || true)"
  attr() { printf '%s' "$root" | sed -n "s/.* $1=\"\\([0-9]*\\)\".*/\\1/p" | head -n1; }
  tests="$(attr tests)"; failures="$(attr failures)"; errors="$(attr errors)"
  tests="${tests:-0}"; failures="${failures:-0}"; errors="${errors:-0}"
  echo "| \`${file}\` | ${tests} | ${failures} | ${errors} |"
  total_tests=$((total_tests + tests))
  total_failures=$((total_failures + failures))
  total_errors=$((total_errors + errors))
done

echo "| **Total** | **${total_tests}** | **${total_failures}** | **${total_errors}** |"
