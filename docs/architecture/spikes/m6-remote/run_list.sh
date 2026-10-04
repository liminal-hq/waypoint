#!/bin/bash
# List a directory with each SFTP client at one latency
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
B=/home/scott/.claude/jobs/1ae7fd8a/tmp/target/release; D=/tmp/claude-1000/spike-remote/data
port=$1
for n in 1000 10000 100000; do
  echo "== port $port dir d$n"
  for m in hl seq pipe:4 pipe:16 pipe:64; do echo -n "russh $m: "; $B/sftp-russh $port list $D/d$n $m | tr '\n' ' '; echo; done
  echo -n "ssh2: "; $B/sftp-ssh2 $port list $D/d$n | tr '\n' ' '; echo
done
