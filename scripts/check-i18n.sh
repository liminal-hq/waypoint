#!/usr/bin/env bash
# Check the translation files and catalogues agree with the English source
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

# `scripts/i18n.ts check` fails when a `translations/*.json` file is out of date with its TypeScript
# catalogue (run `bun run i18n:export`), when importing a JSON file would change its catalogue (run
# `bun run i18n:import`), when a translation's `{placeholders}` differ from English's, and when a
# locale not listed in `INCOMPLETE_LOCALES` has a missing, stale or extra message. Exits 1 and lists
# each problem; 0 if clean. See `docs/translating.md`.
set -euo pipefail

cd "$(git rev-parse --show-toplevel 2>/dev/null || dirname "$(dirname "${BASH_SOURCE[0]}")")"

exec bun scripts/i18n.ts check
