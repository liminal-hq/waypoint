# Milestone 6 remote spike code

Throwaway benchmark programs behind [`../../milestone-6-spikes.md`](../../milestone-6-spikes.md). They are not part of Waypoint's Cargo workspace (each root here is its own `[workspace]`), they are not built by `bun run validate`, and nothing imports them. They are kept so the numbers can be reproduced.

## Layout

| Path                                             | What it is                                                                                             |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------------ |
| `proxy`                                          | TCP proxy that delays each direction by a fixed time (the stand-in for network latency)                |
| `sftp-russh`, `sftp-ssh2`                        | SFTP listing, stat, large read and small read benchmarks, and (russh) an authentication smoke test     |
| `dav`                                            | WebDAV `PROPFIND` listing with a thin `reqwest` and `quick-xml` client and with `reqwest_dav`          |
| `s3-aws`, `s3-rusts3`, `s3-opendal`              | S3 (and, for `opendal`, WebDAV) listing against an S3-compatible endpoint                              |
| `smb/smb-smb`, `smb/smb2-t`                      | SMB listing and read with the `smb` and `smb2` crates (their own workspace, see below)                 |
| `sshd_config`, `smb.conf`                        | Throwaway user-level `sshd` and `smbd` configurations (edit the `/tmp/claude-1000/spike-remote` paths) |
| `run_list.sh`, `smb_tally.sh`, `compile_cost.sh` | The loops that produced the tables                                                                     |

`smb` is a separate workspace because `smb` 0.12 and `russh` 0.63 cannot be resolved together (`curve25519-dalek`). `smb` 0.12.1 also needs `cargo update picky-krb --precise 0.12.4` after the first resolve, or `sspi` fails to compile.

## Reproducing

Keep everything under one scratch directory and set `CARGO_TARGET_DIR` outside the repository.

1. Generate a host key and client keys with `ssh-keygen`, write an `authorized_keys`, and run `sshd -D -e -f sshd_config` as the current user on port 2222 (no root; `UsePAM no`, `StrictModes no`). The benchmarks read the keys from `/tmp/claude-1000/spike-remote/ssh/`.
2. Make the fixtures in `/tmp/claude-1000/spike-remote/data`: `d1000`, `d10000` and `d100000` of empty files named `file_N`, `small` with 3,000 4 KiB files named `s_N`, and a 1 GiB `big.bin` (also hard-linked as `bk/big.bin` for the S3 bucket).
3. Start latency proxies: `proxy 2224 2222 5` (10 ms round trip) and `proxy 2223 2222 50` (100 ms round trip), likewise for the WebDAV, S3 and SMB ports.
4. For WebDAV and S3 run `rclone serve webdav --addr 127.0.0.1:8081 --user scott --pass spikepw --read-only data` and `rclone serve s3 --addr 127.0.0.1:8082 --auth-key AKIASPIKE,secretspike --read-only data` (each top-level directory is a bucket).
5. For SMB add the `scott` user with `pdbedit -a -u scott -t -s smb.conf`, then run `smbd -F --no-process-group -s smb.conf -l <dir> --debug-stdout`.
6. `cargo build --release`, then for example `sftp-russh 2222 list <dir> pipe:64`, `sftp-russh 2223 big <file> 128:32768` with `WINDOW=8388608`, `dav 8081 /d10000/ thin`, `s3-aws 8082 list d10000`, `smb2-t 4445 list d10000`.

Stop every server and delete the scratch directory when done.
