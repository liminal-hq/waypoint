#!/usr/bin/env bash
# Create a directory of many small PNG and JPEG images, for measuring thumbnail delivery on a large folder
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT

# Usage: scripts/thumb-fixture.sh [directory] [count]
# Defaults to 10 000 images in /tmp/waypoint-thumbs. Odd numbers are JPEG, even numbers PNG; sizes run
# from 200 × 150 to 640 × 480; the content comes from a fixed seed, so two runs make the same files.
# Needs python3 with Pillow, and refuses to run with less than 3 GB free where the folder goes.
# The images take about 150 MB. The thumbnails made from them go to $XDG_CACHE_HOME/thumbnails
# (point it somewhere private when measuring, see docs/architecture/milestone-5-spikes.md).
# Clean up with `rm -rf /tmp/waypoint-thumbs` (and the private cache folder you measured with).
set -euo pipefail

dir="${1:-/tmp/waypoint-thumbs}"
count="${2:-10000}"
min_free_kb=$((3 * 1024 * 1024))

mkdir -p "${dir}"
free_kb="$(df --output=avail -k "${dir}" | tail -n 1 | tr -d ' ')"
if [ "${free_kb}" -lt "${min_free_kb}" ]; then
	echo "Refusing to run: only $((free_kb / 1024)) MB free under ${dir}, need 3 GB." >&2
	exit 1
fi

script="$(mktemp --suffix=.py)"
trap 'rm -f "${script}"' EXIT
cat >"${script}" <<'PY'
import random
import sys
from multiprocessing import Pool

from PIL import Image, ImageDraw

directory, count = sys.argv[1], int(sys.argv[2])


def make(n):
    rng = random.Random(n)
    width, height = rng.randrange(200, 641), rng.randrange(150, 481)
    base = tuple(rng.randrange(256) for _ in range(3))
    image = Image.new("RGB", (width, height), base)
    draw = ImageDraw.Draw(image)
    for _ in range(12):
        x0, y0 = rng.randrange(width), rng.randrange(height)
        box = (x0, y0, x0 + rng.randrange(20, width), y0 + rng.randrange(20, height))
        colour = tuple(rng.randrange(256) for _ in range(3))
        (draw.ellipse if rng.random() < 0.5 else draw.rectangle)(box, fill=colour)
    for y in range(0, height, 6):
        draw.line((0, y, width, y), fill=tuple((c + y) % 256 for c in base))
    if n % 2:
        image.save(f"{directory}/img-{n:05d}.jpg", quality=80)
    else:
        image.save(f"{directory}/img-{n:05d}.png")


if __name__ == "__main__":
    with Pool() as pool:
        pool.map(make, range(1, count + 1), chunksize=200)
PY
python3 "${script}" "${dir}" "${count}"
echo "Created ${count} images in ${dir}"
