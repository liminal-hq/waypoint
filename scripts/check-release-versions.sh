#!/usr/bin/env bash
# Check every release manifest carries one synchronised version
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

# A release is built from one version, and the tag is that version with a `v` in front
# (`docs/architecture/ci-cd.md` §4). The version lives in:
#
#   - the root `package.json` (the reference the others are compared with);
#   - `apps/waypoint/package.json`;
#   - `apps/waypoint/src-tauri/tauri.conf.json`;
#   - `[workspace.package] version` in the root `Cargo.toml`, when it sets one;
#   - the `[package] version` of every workspace member under `apps/` (the app crate), and, when a
#     member says `version.workspace = true`, the workspace version it inherits, which must exist.
#
# The libraries and plugins under `crates/` and `plugins/` that set a `version` of their own are not
# part of the release version: they are never released on their own, and the reusable plugins are
# versioned by the shared `tauri-plugins-workspace` repository once they graduate. A library that
# should follow the release says `version.workspace = true` and is checked through the workspace.
#
# With no argument, lists every mismatch and exits 1 (0 and one summary line when all agree).
# `--current-version` prints the one version on standard output, and nothing else, and exits 1 with
# the mismatches on standard error when they differ.
# `--root DIR` reads another tree (tests). The version must be `X.Y.Z` or `X.Y.Z-beta.N`.
set -euo pipefail

mode="check"
root=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --current-version) mode="current" ;;
    --root)
      shift
      root="${1:?--root needs a directory}"
      ;;
    *)
      echo "usage: check-release-versions.sh [--current-version] [--root DIR]" >&2
      exit 2
      ;;
  esac
  shift
done

if [ -z "$root" ]; then
  root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
fi
cd "$root"

# The top-level `version` of a JSON file, empty when there is none.
json_version() {
  local script='try{const v=JSON.parse(require("fs").readFileSync(process.argv[1],"utf8")).version;if(typeof v==="string")process.stdout.write(v)}catch(e){}'
  if command -v node >/dev/null 2>&1; then
    node -e "$script" "$1"
  else
    bun -e "$script" "$1"
  fi
}

# The `version` set under one table of a TOML file: its value, or `workspace` for
# `version.workspace = true`; empty when the table sets none.
toml_version() {
  awk -v want="[$2]" '
    /^[[:space:]]*\[/ { line = $0; gsub(/[[:space:]]/, "", line); in_table = (line == want); next }
    in_table && /^[[:space:]]*version[[:space:]]*\.[[:space:]]*workspace[[:space:]]*=[[:space:]]*true/ { print "workspace"; exit }
    in_table && /^[[:space:]]*version[[:space:]]*=/ {
      v = $0
      sub(/^[^=]*=[[:space:]]*"/, "", v)
      sub(/".*$/, "", v)
      print v
      exit
    }
  ' "$1"
}

# The workspace members of the root `Cargo.toml`, one per line.
workspace_members() {
  awk '
    /^[[:space:]]*members[[:space:]]*=/ { inside = 1 }
    inside {
      line = $0
      while (match(line, /"[^"]+"/)) {
        print substr(line, RSTART + 1, RLENGTH - 2)
        line = substr(line, RSTART + RLENGTH)
      }
      if ($0 ~ /\]/) exit
    }
  ' Cargo.toml
}

sources=()
versions=()
problems=()

record() { # <where> <version>
  sources+=("$1")
  versions+=("$2")
}

for file in package.json apps/waypoint/package.json apps/waypoint/src-tauri/tauri.conf.json; do
  if [ ! -f "$file" ]; then
    problems+=("$file: missing")
  else
    v="$(json_version "$file")"
    if [ -z "$v" ]; then problems+=("$file: no \"version\""); else record "$file" "$v"; fi
  fi
done

if [ ! -f Cargo.toml ]; then
  problems+=("Cargo.toml: missing")
else
  workspace="$(toml_version Cargo.toml workspace.package)"
  [ -n "$workspace" ] && record "Cargo.toml [workspace.package]" "$workspace"
  while IFS= read -r member; do
    manifest="$member/Cargo.toml"
    if [ ! -f "$manifest" ]; then
      problems+=("$manifest: missing (listed in the workspace)")
      continue
    fi
    # Only the app crate and the members that inherit the workspace version are release versions.
    case "$member" in
      apps/*) ;;
      *) [ "$(toml_version "$manifest" package)" = "workspace" ] || continue ;;
    esac
    v="$(toml_version "$manifest" package)"
    if [ "$v" = "workspace" ]; then
      if [ -z "$workspace" ]; then problems+=("$manifest: says version.workspace = true but [workspace.package] sets no version"); fi
    elif [ -z "$v" ]; then
      problems+=("$manifest: no version")
    else
      record "$manifest" "$v"
    fi
  done < <(workspace_members)
fi

reference=""
if [ "${#versions[@]}" -gt 0 ]; then reference="${versions[0]}"; fi
for i in "${!versions[@]}"; do
  if [ "${versions[$i]}" != "$reference" ]; then
    problems+=("${sources[$i]}: ${versions[$i]} (expected $reference, the version in ${sources[0]})")
  fi
done
if [ -n "$reference" ] && ! [[ "$reference" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-beta\.[0-9]+)?$ ]]; then
  problems+=("version $reference is not X.Y.Z or X.Y.Z-beta.N")
fi

if [ "${#problems[@]}" -gt 0 ]; then
  {
    echo "Release versions are not synchronised:"
    for p in "${problems[@]}"; do echo "  $p"; done
  } >&2
  exit 1
fi

if [ "$mode" = "current" ]; then
  echo "$reference"
else
  echo "All ${#versions[@]} release versions are $reference."
fi
