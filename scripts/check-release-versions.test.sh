#!/usr/bin/env bash
# Fixture tests for check-release-versions.sh
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
set -uo pipefail

script="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/check-release-versions.sh"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
failures=0
count=0

# A tree where every manifest says $1, and the workspace has the app, a library that keeps a version
# of its own, a library that inherits the workspace version and a reusable plugin.
make_tree() {
  local dir="$work/tree-$count" v="$1"
  mkdir -p "$dir/apps/waypoint/src-tauri" "$dir/crates/own" "$dir/crates/inherits" "$dir/plugins/reusable"
  printf '{\n\t"name": "waypoint",\n\t"version": "%s",\n\t"dependencies": {\n\t\t"x": { "version": "9.9.9" }\n\t}\n}\n' "$v" >"$dir/package.json"
  printf '{\n\t"name": "app",\n\t"version": "%s"\n}\n' "$v" >"$dir/apps/waypoint/package.json"
  printf '{\n\t"productName": "Waypoint",\n\t"version": "%s"\n}\n' "$v" >"$dir/apps/waypoint/src-tauri/tauri.conf.json"
  cat >"$dir/Cargo.toml" <<TOML
[workspace]
members = [
  "apps/waypoint/src-tauri",
  "crates/own",
  "crates/inherits",
  "plugins/reusable",
]
resolver = "2"

[workspace.package]
edition = "2021"
version = "$v"

[workspace.dependencies]
serde = { version = "1" }
TOML
  printf '[package]\nname = "waypoint"\nversion = "%s"\n\n[dependencies]\nserde = { version = "1" }\n' "$v" >"$dir/apps/waypoint/src-tauri/Cargo.toml"
  printf '[package]\nname = "own"\nversion = "0.1.0"\npublish = false\n' >"$dir/crates/own/Cargo.toml"
  printf '[package]\nname = "inherits"\nversion.workspace = true\n' >"$dir/crates/inherits/Cargo.toml"
  printf '[package]\nname = "reusable"\nversion = "0.2.0"\n' >"$dir/plugins/reusable/Cargo.toml"
  echo "$dir"
}

# check <name> <exit status> <output substring or empty> <args...>; reads $dir, set by the caller.
check() {
  local name="$1" want="$2" needle="$3"
  shift 3
  local out status
  out="$(bash "$script" --root "$dir" "$@" 2>&1)"
  status=$?
  if [ "$status" -ne "$want" ] || { [ -n "$needle" ] && [[ "$out" != *"$needle"* ]]; }; then
    echo "FAIL: $name (exit $status, output: $out)"
    failures=$((failures + 1))
  fi
}

# edit <file> <sed expression>
edit() { sed -i "$2" "$1"; }

count=1; dir="$(make_tree 1.2.3)"
check "all in sync" 0 "All 5 release versions are 1.2.3."
check "libraries and plugins with their own versions are not release versions" 0 ""

count=2; dir="$(make_tree 1.2.3-beta.4)"
check "a beta version in sync" 0 "1.2.3-beta.4"

count=3; dir="$(make_tree 1.2.3)"
edit "$dir/package.json" '3s/1.2.3/1.2.4/'
check "root package.json differs" 1 "apps/waypoint/package.json: 1.2.3 (expected 1.2.4, the version in package.json"

count=4; dir="$(make_tree 1.2.3)"
edit "$dir/apps/waypoint/package.json" '3s/1.2.3/1.2.4/'
check "app package.json differs" 1 "apps/waypoint/package.json: 1.2.4 (expected 1.2.3"

count=5; dir="$(make_tree 1.2.3)"
edit "$dir/apps/waypoint/src-tauri/tauri.conf.json" 's/1.2.3/1.2.4/'
check "tauri.conf.json differs" 1 "tauri.conf.json: 1.2.4"

count=6; dir="$(make_tree 1.2.3)"
edit "$dir/Cargo.toml" 's/^version = "1.2.3"/version = "1.2.4"/'
check "workspace version differs" 1 "Cargo.toml [workspace.package]: 1.2.4"

count=7; dir="$(make_tree 1.2.3)"
edit "$dir/apps/waypoint/src-tauri/Cargo.toml" 's/^version = "1.2.3"/version = "1.2.4"/'
check "app crate with its own version differs" 1 "apps/waypoint/src-tauri/Cargo.toml: 1.2.4"

count=8; dir="$(make_tree 1.2.3)"
edit "$dir/Cargo.toml" '/^version = /d'
check "a member inherits a workspace version that does not exist" 1 "sets no version"

count=9; dir="$(make_tree 1.2.3)"
edit "$dir/package.json" '3s/1.2.3/1.2.4/'
edit "$dir/apps/waypoint/package.json" '3s/1.2.3/1.2.5/'
check "every mismatch is listed (first)" 1 "apps/waypoint/package.json: 1.2.5"
check "every mismatch is listed (second)" 1 "tauri.conf.json: 1.2.3"

count=10; dir="$(make_tree 1.2.3)"
edit "$dir/package.json" '3s/1.2.3/1.2/'
edit "$dir/apps/waypoint/package.json" '3s/1.2.3/1.2/'
edit "$dir/apps/waypoint/src-tauri/tauri.conf.json" 's/1.2.3/1.2/'
edit "$dir/Cargo.toml" 's/^version = "1.2.3"/version = "1.2"/'
edit "$dir/apps/waypoint/src-tauri/Cargo.toml" 's/^version = "1.2.3"/version = "1.2"/'
check "in sync but not a release version" 1 "is not X.Y.Z"

count=11; dir="$(make_tree 1.2.3)"
rm "$dir/apps/waypoint/package.json"
check "a missing manifest" 1 "apps/waypoint/package.json: missing"

# --current-version: exactly the version on stdout, nothing else.
count=12; dir="$(make_tree 1.2.3)"
out="$(bash "$script" --root "$dir" --current-version 2>/dev/null)"
[ "$out" = "1.2.3" ] || { echo "FAIL: --current-version prints only the version (got: $out)"; failures=$((failures + 1)); }
count=13; dir="$(make_tree 1.2.3-beta.1)"
out="$(bash "$script" --root "$dir" --current-version 2>/dev/null)"
[ "$out" = "1.2.3-beta.1" ] || { echo "FAIL: --current-version for a beta (got: $out)"; failures=$((failures + 1)); }

count=14; dir="$(make_tree 1.2.3)"
edit "$dir/package.json" '3s/1.2.3/1.2.4/'
out="$(bash "$script" --root "$dir" --current-version 2>/dev/null)"
status=$?
if [ "$status" -eq 0 ] || [ -n "$out" ]; then
  echo "FAIL: --current-version with a mismatch must print nothing and fail (exit $status, stdout: $out)"
  failures=$((failures + 1))
fi

count=15; dir="$(make_tree 1.2.3)"
check "an unknown argument" 2 "usage:" --bogus

[ "$failures" -eq 0 ] && echo "check-release-versions: all fixtures passed"
exit "$failures"
