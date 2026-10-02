#!/usr/bin/env bash
# Build a disposable tree of files for trying file operations by hand or from an agent
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

# Usage: scripts/ops-fixture.sh [--help] DIRECTORY [MEGABYTES]
#
# Makes DIRECTORY (which must not exist, and must be under a temporary directory such as /tmp) with:
#
#   src/                 nested folders, names with spaces and Unicode, a symlink, an empty folder
#   src/big.bin          a file of MEGABYTES MiB (default 24), more than two copy chunks
#   src/clash.txt        the same name as dst/clash.txt, with different content, to meet a conflict
#   src/tree/clash.txt   and a folder `tree` that also exists in dst, to meet a merge
#   dst/                 an empty destination, with clash.txt and tree/clash.txt already in it
#   other/               a second folder to move things to
#
# Nothing outside DIRECTORY is touched. Delete it afterwards with `rm -rf DIRECTORY`.
set -euo pipefail

usage() {
  sed -n '/^# Usage:/,/^# Nothing outside/p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
}

case "${1:-}" in
  -h | --help)
    usage
    exit 0
    ;;
  "")
    usage >&2
    exit 2
    ;;
esac

dir="$1"
megabytes="${2:-24}"

# A fixture is made to be deleted, so it only goes under a temporary directory.
case "$(realpath -m -- "${dir}")" in
  /tmp/* | /var/tmp/* | "${TMPDIR:-/tmp}"/*) ;;
  *)
    echo "refusing to build a fixture outside a temporary directory: ${dir}" >&2
    exit 2
    ;;
esac
if [ -e "${dir}" ]; then
  echo "${dir} already exists; choose a new name" >&2
  exit 2
fi
case "${megabytes}" in
  '' | *[!0-9]*)
    echo "MEGABYTES must be a whole number" >&2
    exit 2
    ;;
esac

mkdir -p "${dir}/src/nested/deeper" "${dir}/src/tree" "${dir}/src/folder with spaces" \
  "${dir}/src/empty" "${dir}/src/Ünïcödé dossier" "${dir}/dst/tree" "${dir}/other"

printf 'alpha\n' >"${dir}/src/a.txt"
printf 'second file\n' >"${dir}/src/nested/b.txt"
printf 'deep\n' >"${dir}/src/nested/deeper/c.txt"
printf 'spaces\n' >"${dir}/src/folder with spaces/a file with spaces.txt"
printf 'unicode\n' >"${dir}/src/Ünïcödé dossier/café – naïve 日本語.txt"
printf 'the source copy\n' >"${dir}/src/clash.txt"
printf 'the source copy\n' >"${dir}/src/tree/clash.txt"
printf 'only in the source\n' >"${dir}/src/tree/only-in-src.txt"
printf 'the file already in the destination\n' >"${dir}/dst/clash.txt"
printf 'the file already in the destination\n' >"${dir}/dst/tree/clash.txt"
ln -s a.txt "${dir}/src/link-to-a"
ln -s nowhere "${dir}/src/dangling-link"

# A repeating pattern rather than random bytes, so a copy can be checked with `cmp` and the file
# compresses on a snapshotted file system.
head -c "$((megabytes * 1024 * 1024))" < <(yes 'waypoint fixture line') >"${dir}/src/big.bin"

echo "Built ${dir}"
( cd "${dir}" && find . -mindepth 1 | LC_ALL=C sort | sed 's|^\./||' )
