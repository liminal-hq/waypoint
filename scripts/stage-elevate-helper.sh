#!/usr/bin/env bash
# Build the elevated helper and stage it where the release package configs expect it
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

# Usage: scripts/stage-elevate-helper.sh [target-triple]
#
# The installers carry `waypoint-elevate-helper` (`docs/architecture/ci-cd.md` §4), and the files
# that say so (`apps/waypoint/src-tauri/tauri.package.{linux,windows}.json`) name one fixed path,
# whatever the target directory or the architecture. This builds the helper as the release build
# builds the app (a release profile; the profile environment of the job is inherited, so
# `CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS` is whatever the workflow pinned), honours
# `CARGO_TARGET_DIR` and the optional target triple, and copies the result to
# `apps/waypoint/src-tauri/packaging/staged/` (git-ignored). The file is
# `waypoint-elevate-helper.exe` for a Windows target or host and `waypoint-elevate-helper` otherwise.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

target="${1:-}"
staged_dir="apps/waypoint/src-tauri/packaging/staged"

build_args=(build --release -p waypoint-elevate-helper)
if [[ -n "${target}" ]]; then
  build_args+=(--target "${target}")
fi
cargo "${build_args[@]}"

# Where Cargo put it: `<target dir>[/<triple>]/release/`. Windows hosts and targets add `.exe`.
target_dir="${CARGO_TARGET_DIR:-${repo_root}/target}"
if [[ -n "${target}" ]]; then
  profile_dir="${target_dir}/${target}/release"
else
  profile_dir="${target_dir}/release"
fi

case "${target:-$(uname -s)}" in
  *windows* | MINGW* | MSYS* | CYGWIN*) name="waypoint-elevate-helper.exe" ;;
  *) name="waypoint-elevate-helper" ;;
esac

built="${profile_dir}/${name}"
if [[ ! -f "${built}" ]]; then
  echo "Expected build output not found: ${built}" >&2
  exit 1
fi

# One helper at a time: a leftover from another target must never be packaged by mistake.
rm -rf "${staged_dir}"
mkdir -p "${staged_dir}"
cp "${built}" "${staged_dir}/${name}"
if [[ "${name}" != *.exe ]]; then
  chmod 0755 "${staged_dir}/${name}"
fi

staged="${staged_dir}/${name}"
if command -v sha256sum >/dev/null 2>&1; then
  sum="$(sha256sum "${staged}" | cut -d' ' -f1)"
else
  sum="$(shasum -a 256 "${staged}" | cut -d' ' -f1)"
fi
size="$(wc -c <"${staged}" | tr -d ' ')"
echo "Staged ${staged}"
echo "  sha256 ${sum}"
echo "  size   ${size} bytes"
