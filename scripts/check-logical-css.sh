#!/usr/bin/env bash
# Check stylesheets use logical properties, so the layout mirrors under `dir="rtl"`
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

# Waypoint mirrors for right-to-left languages (A68), which only works when CSS says "start" and
# "end" and not "left" and "right". `scripts/check-logical-css.ts` fails on a physical `margin-left`,
# `padding-right`, `border-left`, `left:`, `right:`, `text-align: left`, a corner radius such as
# `border-top-left-radius`, or a four-value `margin`, `padding` or `inset` whose sides differ, in any
# tracked `.css`, and on the camelCase forms in `.tsx`. A line where physical really is right (a
# window-edge resize handle, drag geometry that follows the pointer) carries `physical:` and the
# reason in a comment on that line or the one above. Exits 1 and lists each offence; 0 if clean.
set -euo pipefail

cd "$(git rev-parse --show-toplevel 2>/dev/null || dirname "$(dirname "${BASH_SOURCE[0]}")")"

exec bun scripts/check-logical-css.ts
