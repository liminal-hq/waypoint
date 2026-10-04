#!/bin/bash
# Count completed and hung SMB listings
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
B=/home/scott/.claude/jobs/1ae7fd8a/tmp/target/release
for d in d1000 d10000 d100000; do ok=0; hang=0; for i in $(seq 1 12); do
 out=$(timeout 30 $B/smb-smb 4445 list $d 2>/dev/null | tail -1); if echo "$out" | grep -q "smb list"; then ok=$((ok+1)); echo "$d $out" | cut -c1-100 >> smb_runs.txt; else hang=$((hang+1)); fi; done
 echo "$d ok=$ok hung=$hang"; done
