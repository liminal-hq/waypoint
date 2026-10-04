#!/bin/bash
# Time a clean release build of each candidate
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
export CARGO_HOME=/home/scott/.claude/jobs/1ae7fd8a/tmp/cargohome CARGO_INCREMENTAL=0
cd /tmp/claude-1000/spike-remote
for spec in "proxy:." "sftp-russh:." "sftp-ssh2:." "dav:." "s3-aws:." "s3-rusts3:." "s3-opendal:." "smb-smb:smb"; do
  pkg=${spec%%:*}; dir=${spec##*:}
  export CARGO_TARGET_DIR=/home/scott/.claude/jobs/1ae7fd8a/tmp/target-$pkg
  rm -rf $CARGO_TARGET_DIR
  s=$(date +%s.%N)
  (cd $dir && cargo build --release -p $pkg >/dev/null 2>&1)
  e=$(date +%s.%N)
  f=$CARGO_TARGET_DIR/release/$pkg
  strip -o /tmp/claude-1000/spike-remote/stripped-$pkg $f
  echo "$pkg wall_s=$(echo "$e - $s" | bc) unstripped=$(stat -c %s $f) stripped=$(stat -c %s /tmp/claude-1000/spike-remote/stripped-$pkg) target_mb=$(du -sm $CARGO_TARGET_DIR | cut -f1) crates=$(cd $dir && cargo tree -p $pkg -e normal --prefix none 2>/dev/null | sort -u | wc -l)"
  rm -rf $CARGO_TARGET_DIR
done
