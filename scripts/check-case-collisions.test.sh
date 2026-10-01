#!/usr/bin/env bash
# Fixture tests for check-case-collisions.sh
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
set -uo pipefail

script="$(dirname "${BASH_SOURCE[0]}")/check-case-collisions.sh"
failures=0

# expect <name> <exit status> <output substring or empty> <file>...
expect() {
  local name="$1" want="$2" needle="$3"
  shift 3
  local out status
  out="$(printf '%s\0' "$@" | bash "$script" --stdin)"
  status=$?
  if [ "$status" -ne "$want" ] || { [ -n "$needle" ] && [[ "$out" != *"$needle"* ]]; }; then
    echo "FAIL: $name (exit $status, output: $out)"
    failures=$((failures + 1))
  fi
}

expect "different extensions in different namespaces" 0 "" 'src/Foo.rs' 'src/foo.ts'
expect "extension and compound extension" 0 "" 'a.ts' 'a.module.css' 'a.test.ts'
expect "same stem, two script extensions, same case" 0 "" 'src/a.ts' 'src/a.tsx'
expect "tsx versus tsx with different case" 1 'src/tabMenus.tsx' 'src/TabMenus.tsx' 'src/tabMenus.tsx'
expect "the original incident (tsx versus ts)" 1 'src/TabMenus.tsx' 'src/TabMenus.tsx' 'src/tabMenus.ts'
expect "rust files that differ by case" 1 'src/Foo.rs' 'src/Foo.rs' 'src/foo.rs'
expect "case differing directories" 1 'Src/a.ts' 'Src/a.ts' 'src/a.ts'
expect "paths with spaces are listed whole" 1 '  my dir/Foo bar.ts' 'my dir/Foo bar.ts' 'my dir/foo bar.ts'
expect "empty list" 0 ""

[ "$failures" -eq 0 ] && echo "check-case-collisions: all fixtures passed"
exit "$failures"
