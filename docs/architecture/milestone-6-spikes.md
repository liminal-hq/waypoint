# Milestone 6 spike results: remote protocol libraries and SFTP listing throughput

This records the remote spike (#278): which library each remote provider should use, and how fast a remote listing can be made (the remote half of milestone 0 spike 5, [`milestone-0-spikes.md`](milestone-0-spikes.md)). It says what was measured, on what, and what is **unmeasured**. Every number is from a loopback test against a local server with latency simulated by a proxy. Nothing was measured on Windows, over a real network, against Nextcloud, AWS S3, Windows file servers or a hardware security key.

The spike code is throwaway. It is kept, with a README that reproduces every table, in [`spikes/m6-remote/`](spikes/m6-remote/).

## Setup

| Item        | Value                                                                                                                                                                                                                                                                                                                                                               |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Machine     | AMD Ryzen 7 7700X (8 cores, 16 threads), 30 GB RAM, NVMe, Linux 7.1.9 (Arch), Rust 1.98.0; fixtures on tmpfs, so reads are from memory                                                                                                                                                                                                                              |
| SFTP server | OpenSSH 10.5p1 `sshd` run as the current user (`-D -f <config> -p 2222`, no root, `UsePAM no`), `sftp-server` subsystem, ed25519 host key; default ciphers                                                                                                                                                                                                          |
| SMB server  | Samba 4.24.6 `smbd` as the current user on port 4445, SMB2+, NTLMv2, read-only share                                                                                                                                                                                                                                                                                |
| WebDAV, S3  | `rclone` 1.75.0 `serve webdav` and `serve s3`, read-only, over the same directories. These servers set the floor for those protocols: rclone builds each answer in full, so the WebDAV and S3 figures are as much a measurement of rclone as of the clients                                                                                                         |
| Fixtures    | Directories of 1,000, 10,000 and 100,000 empty files; 3,000 random 4 KiB files; one 1 GiB random file                                                                                                                                                                                                                                                               |
| Latency     | A tiny TCP proxy (`spikes/m6-remote/proxy`) that delays each direction by a fixed time, queued so order is kept, with no bandwidth limit (`tc netem` needs root). **LAN** is the direct connection (about 0.1 ms), **10 ms** is 5 ms each way, **100 ms** is 50 ms each way. The proxy does not delay the TCP handshake, so connect time is excluded or understated |
| Method      | Release builds, one process per measurement, a connection already authenticated before the clock starts (except where said), `Instant` timers, `VmHWM` from `/proc` for peak resident memory (which includes the runtime and the library, about 7 MB at idle)                                                                                                       |
| Repeats     | One run per cell unless a range or a count is given; loopback timings repeated within a few per cent where repeated. The tables are not statistics                                                                                                                                                                                                                  |

## SFTP and SSH

### The candidates

| Library                                            | Licence             | Maintenance (crates.io, 2026-10-03)                                                                 | Async                                     | Build needs                                                                                                                                          |
| -------------------------------------------------- | ------------------- | --------------------------------------------------------------------------------------------------- | ----------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| `russh` 0.63.3 with `russh-sftp` 3.0.1             | Apache-2.0          | Updated 2026-09-09 and 2026-09-28; 3.2 M recent downloads; MSRV 1.89                                | Tokio, native                             | Pure Rust; `aws-lc-rs` by default (a C build) or the `ring` feature                                                                                  |
| `ssh2` 0.9.6 (libssh2)                             | MIT OR Apache-2.0   | Updated 2026-06-30; 1.2 M recent downloads                                                          | Blocking only; needs a thread per session | Vendored libssh2 plus system OpenSSL on Linux (`libssl.so.3` linked here); WinCNG backend on Windows                                                 |
| `openssh` 0.11.6 and `openssh-sftp-client` 0.15.10 | MIT OR Apache / MIT | Updated 2025-12-03 and 2026-10-01                                                                   | Tokio                                     | Spawns the system `ssh` binary. **Unix only**: `openssh` fails with `compile_error!("This crate can only be used on unix")` under `cargo xwin check` |
| `thrussh` 0.49.0                                   | Apache-2.0          | The ancestor of `russh`; 7 k recent downloads                                                       | Tokio                                     | Not evaluated further                                                                                                                                |
| `async-ssh2-tokio` 0.13.0, `libssh-rs` 0.3.8       | non-standard, MIT   | Not evaluated: a `russh` wrapper with a non-standard licence field, and a binding to the C `libssh` | n/a                                       | Not evaluated                                                                                                                                        |

`russh-config` 0.58.0 (Apache-2.0) and `ssh2-config` 0.8.1 (MIT) parse `~/.ssh/config`; neither was exercised. Both licences are compatible with Waypoint's `Apache-2.0 OR MIT`. All licences in the dependency trees of the chosen candidates were checked with `cargo metadata`: the only non-MIT/Apache terms are BSD-3-Clause (`curve25519-dalek`, `ed25519-dalek`, `subtle`), ISC, Zlib, Unicode-3.0, CDLA-Permissive-2.0 (`webpki-roots`), CC0-1.0 and, for `rust-s3` only, MPL-2.0 (`attohttpc`). All are permissive or file-level copyleft, none a blocker.

### Authentication, host keys and jump hosts

| Capability                          | `russh` + `russh-sftp`                                                                                                                                                  | `ssh2`                                                                                                                           |
| ----------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| Key, no passphrase                  | **Measured**: ed25519 accepted                                                                                                                                          | **Measured**: ed25519 accepted                                                                                                   |
| Key with passphrase                 | **Measured**: ed25519 with passphrase accepted (183 ms including connect and the key's KDF)                                                                             | In the API (`userauth_pubkey_file` takes a passphrase); unmeasured                                                               |
| RSA                                 | **Measured**: needs `best_supported_rsa_hash()` first. Passing `None` selects `ssh-rsa` (SHA-1), which this OpenSSH rejects; with SHA-2 it works                        | Unmeasured                                                                                                                       |
| ssh-agent                           | **Measured** through `AgentClient` on `SSH_AUTH_SOCK`; Windows has Pageant and named pipe clients (compiles under `cargo xwin check`)                                   | In the API (`userauth_agent`); unmeasured                                                                                        |
| Password, keyboard-interactive      | In the API (`authenticate_password`, `authenticate_keyboard_interactive_start` and `_respond`). **Unmeasured**: a user-level `sshd` cannot check passwords without root | In the API; unmeasured for the same reason                                                                                       |
| FIDO (`sk-ssh-ed25519`, `sk-ecdsa`) | `ssh-key` parses the `sk` algorithms; signing must go through an agent (`authenticate_publickey_with` takes a signer). **Unmeasured**: no hardware key                  | Whether `ssh2` 0.9.6 exposes `sk` key signing was not checked; assume agent only. **Unmeasured**                                 |
| GSSAPI (Kerberos)                   | `authenticate_gssapi_with_mic` exists; unmeasured                                                                                                                       | Not checked                                                                                                                      |
| Host key verification               | `check_server_key` is an async callback that sees the key and returns accept or reject: the known-hosts prompt plugs in here (the spike accepted everything)            | `Session::host_key()` after the handshake plus `KnownHosts`; the caller decides before authenticating                            |
| Jump host (ProxyJump)               | **Measured**: `channel_open_direct_tcpip` on a first session, then `client::connect_stream` over `channel.into_stream()` ran a second authenticated session             | A `Channel` is not a socket and a libssh2 session needs one (`AsRawFd`), so a jump needs a local socket relay thread. Unmeasured |
| SSH config                          | Not parsed by the library                                                                                                                                               | Not parsed by the library                                                                                                        |

### Build cost

Clean release builds on 16 threads, each in an empty target directory. The baseline is a program that only links Tokio (`full`). Binaries are stripped as Waypoint's release profile does. These are not incremental builds: a change to Waypoint's own code relinks, which was not measured separately.

| Candidate                          | Wall time | Crates | Stripped binary | Delta over the Tokio baseline |
| ---------------------------------- | --------- | ------ | --------------- | ----------------------------- |
| Tokio only (baseline, the proxy)   | 4.5 s     | 22     | 0.77 MB         | 0                             |
| `russh` + `russh-sftp` + `futures` | 25.9 s    | 211    | 7.2 MB          | +6.5 MB                       |
| `ssh2`                             | 5.7 s     | 13     | 0.67 MB         | no Tokio; links `libssl`      |

**Windows (unmeasured at run time).** `cargo xwin check --target x86_64-pc-windows-msvc` passes for both `russh` (with the agent code behind `cfg(windows)`) and `ssh2`. That compiles the C parts (`aws-lc-sys`, vendored libssh2) but links nothing and runs nothing. `ssh2`'s default Windows backend is WinCNG, which as far as libssh2's documentation says has no ed25519 support (not verified here), so key types would differ by platform. `russh` offers the `ring` feature if `aws-lc-rs` proves hard to build on the Windows CI image.

### Listing throughput

Directory listing, one connection, `opendir` followed by `readdir` until end of directory. OpenSSH's `sftp-server` returns about 100 entries (name, long name and attributes) per `SSH_FXP_READDIR`: 11 requests for 1,000 entries, 1,001 for 100,000. **Attributes arrive with the listing**; no per-entry `stat` is needed for size, times and type.

`first` is the time until the first batch of entries is in hand, counted from sending `opendir`, so it is two round trips plus server work. `seq` waits for each reply before sending the next request. `pipe:N` keeps N `readdir` requests on the wire at once (`RawSftpSession`, a sliding window of futures); the server answers them in order. `hl` is `russh-sftp`'s `SftpSession::read_dir`, which returns only when the whole directory is read.

Total time to list, milliseconds (first-batch time in brackets where it matters):

| Latency | Entries | `russh` hl | `russh` seq | `russh` pipe:4 | `russh` pipe:16 | `russh` pipe:64 | `ssh2` (sequential) |
| ------- | ------- | ---------- | ----------- | -------------- | --------------- | --------------- | ------------------- |
| LAN     | 1,000   | 4          | 4           | 3              | 3               | 4               | 4                   |
| LAN     | 10,000  | 92         | 73          | 26             | 26              | 26              | 31                  |
| LAN     | 100,000 | 2,688      | 734         | 249            | 252             | 249             | 319                 |
| 10 ms   | 1,000   | 178        | 165         | 76             | 50              | 51              | 163                 |
| 10 ms   | 10,000  | 1,392      | 1,351       | 433            | 126             | 75              | 1,306               |
| 10 ms   | 100,000 | 15,196     | 13,210      | 4,038          | 914             | 297             | 12,752              |
| 100 ms  | 1,000   | 1,443      | 1,341       | 516            | 350             | 351             | 1,340               |
| 100 ms  | 10,000  | 10,757     | 10,610      | 2,923          | 928             | 422             | 10,615              |
| 100 ms  | 100,000 | 106,263    | 103,387     | 26,202         | 7,027           | 1,905           | 103,356             |

First batch: 1.1 ms (LAN), 26 ms (10 ms), 206 ms (100 ms) for every library and mode except `hl`, which has no first batch. That is two round trips and does not depend on the directory's size.

Peak resident memory of the whole process listing 100,000 entries: 12 to 14 MB for `seq` and `pipe`, 14 MB for `ssh2`, **34 to 35 MB for `hl`**. Only file names were kept, so a provider holding full metadata would hold more.

**What the numbers say.**

- **Latency, not CPU, decides a remote listing, and only pipelining beats it.** A sequential listing costs one round trip per 100 entries whatever the library: `russh` `seq` and `ssh2` agree within 1 % at 100 ms (103.4 s and 103.4 s for 100,000 entries). With 64 requests in flight the same listing takes 1.9 s, 54 times faster, and 0.3 s at 10 ms. At 16 in flight it takes 7.0 s at 100 ms. `ssh2` is a blocking, one-request-at-a-time API and cannot pipeline `readdir` in one session.
- **At LAN speed the CPU still matters a little.** `ssh2` lists 100,000 entries in 319 ms against `russh`'s 734 ms sequential, but `russh` pipelined takes 249 ms.
- **`russh-sftp`'s high-level `read_dir` is the wrong call.** It collects everything before returning (no first rows), and at LAN speed 100,000 entries take 2,688 ms against 734 ms for the same requests issued by hand, with 34 MB against 13 MB of memory. Its loop rebuilds the accumulated vector for every batch (`name.files.into_iter()…chain(files).collect()`), which is quadratic in the number of batches. Waypoint must use `RawSftpSession` (`opendir`, `readdir`, `close`) and stream each batch to the listing handle.
- **A `stat` per entry is not affordable.** Over a 10,000-entry directory `lstat` costs 0.027 to 0.035 ms per entry at LAN speed sequentially, 12.3 ms per entry at 10 ms (an entry per round trip: 100,000 would take 20 minutes) and 102.7 ms per entry at 100 ms. Pipelined with 64 requests in flight it is 0.002 ms (LAN), 0.24 ms (10 ms) and 1.65 ms (100 ms) per entry, and with 256 in flight 0.44 ms at 100 ms: 10,000 entries still take 4.4 s. Use the attributes from `readdir`, and resolve symlink targets only for the visible rows.

### Reading files

**Large file**, 1 GiB random, one connection, read to memory and discarded:

| Latency          | `russh` hl (`File` reads, 16 requests in flight) | `russh` raw, 64 × 32 KiB | `russh` raw, 128 × 32 KiB | `ssh2`    |
| ---------------- | ------------------------------------------------ | ------------------------ | ------------------------- | --------- |
| LAN              | 1,037 MiB/s                                      | 955 MiB/s                | 968 MiB/s                 | 519 MiB/s |
| 10 ms (256 MiB)  | 80 MiB/s                                         | 81 MiB/s                 | 81 MiB/s                  | 244 MiB/s |
| 100 ms (128 MiB) | 9 MiB/s                                          | 10 MiB/s                 | 10 MiB/s                  | 37 MiB/s  |

The default `russh` client holds a 2 MiB SSH channel window, and no number of SFTP requests in flight can exceed the window per round trip. Raising `client::Config::window_size` fixes it:

| `window_size` (100 ms, 128 MiB) | 2 MiB    | 8 MiB    | 32 MiB   |
| ------------------------------- | -------- | -------- | -------- |
| `russh` hl                      | 9 MiB/s  | 30 MiB/s | 31 MiB/s |
| `russh` raw, 128 × 32 KiB       | 10 MiB/s | 32 MiB/s | 33 MiB/s |

With an 8 MiB window, `russh` reaches 252 MiB/s at 10 ms and 1,012 MiB/s on LAN, close to `ssh2`'s adaptive window (244 and 37 MiB/s) at 10 ms and 100 ms. The plateau near 32 MiB/s at 100 ms was not explained (the server side or the proxy). Peak memory was 9 to 14 MB throughout.

**Small files**, 3,000 files of 4 KiB, each opened, read to end and closed:

| Latency | `russh` 1 at a time                 | `russh` 8 | `russh` 32       | `russh` 128    | `ssh2` 1 at a time                  |
| ------- | ----------------------------------- | --------- | ---------------- | -------------- | ----------------------------------- |
| LAN     | 925 ms (3,242/s)                    | 86 ms     | 38 ms (79,798/s) | not run        | 596 ms (5,036/s)                    |
| 10 ms   | 151.9 s (20/s)                      | 19.6 s    | 6.1 s (490/s)    | not run        | 261.4 s (11/s)                      |
| 100 ms  | 410 ms per file (100 files: 41.1 s) | not run   | 40.0 s (75/s)    | 10.8 s (278/s) | 718 ms per file (100 files: 71.8 s) |

Opening, reading to the end and closing a file costs four to five round trips with `russh` and seven to nine with `ssh2`; concurrency is the only remedy. A copy or a thumbnail pass over many small remote files has to run many files at once.

### Recommendation: SFTP

**`russh` with `russh-sftp`, through `RawSftpSession`.** It is the only candidate that is async, pure Rust, Apache-2.0, reaches every capability the SPEC §7 row needs (agent, passphrase keys, host key hook, jump host over `direct-tcpip`) and can pipeline, and pipelining is what meets the listing budget at 100 ms. `ssh2` would need a thread per connection and cannot get within 50 times of the budget over latency. `openssh` is unix-only and needs the system `ssh`. Rules this puts on the provider:

- Stream the listing: issue `opendir`, then a sliding window of at least 32 `readdir` requests, send each reply's entries to the `Channel` as it arrives and stop issuing at `SSH_FX_EOF`. Take size, times, permissions and type from the listing.
- Set `window_size` to 8 MiB or more and issue 64 × 32 KiB reads for file transfer; open many files at once for small-file work.
- Never call the high-level `SftpSession::read_dir`, and never `lstat` every entry.
- A server may limit requests in flight. The window must back off (and fall back to sequential) on an error rather than assume OpenSSH.
- Wrap the host key callback in the known-hosts store and run the prompt in the frontend; do not accept unknown keys.

**Risks.** `russh` is 0.x and moves quickly; pin it and expect upgrade work. It implements the protocol and builds on RustCrypto crates and `aws-lc-rs` rather than on OpenSSH's or libssh2's long-audited code, so it deserves a security review before release. It cannot be resolved in one Cargo workspace with the `smb` crate (below). Password and keyboard-interactive, FIDO and GSSAPI are unmeasured. The Windows build is unlinked.

**Remote listing budget for the verification feature.** Proposed from the table, with the measured value beside each:

| Condition                          | Budget                                       | Measured (`russh`, 64 in flight) |
| ---------------------------------- | -------------------------------------------- | -------------------------------- |
| First rows visible                 | Within 3 round trips plus 50 ms              | 2 round trips plus about 1 ms    |
| 100,000 entries, LAN               | 0.5 s                                        | 0.25 s                           |
| 100,000 entries, 10 ms round trip  | 1 s                                          | 0.30 s                           |
| 100,000 entries, 100 ms round trip | 5 s                                          | 1.9 s                            |
| Process memory over idle           | 50 MB for 100,000 entries without thumbnails | about 6 MB (names only)          |

The budget is for the transport, with the same caveats as A18's: loopback, OpenSSH, empty files and no JSON bridge. The listing pipeline from the provider to the page adds the 15 to 20 per cent measured in milestone 0.

## SMB

### The candidates

| Library                    | Licence           | What it is                                                                                                                                                                                                                | Windows                       |
| -------------------------- | ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------- |
| `smb` 0.12.1               | MIT               | Pure Rust SMB 2/3 client, Tokio or threaded, NTLM and Kerberos through `sspi`, signing, encryption, compression, QUIC; 14 k recent downloads; updated 2026-09-20                                                          | Compiles (`cargo xwin check`) |
| `smb2` 0.27.0              | MIT OR Apache-2.0 | Pure Rust SMB 2/3 client, Tokio or smol, NTLM and Kerberos, signing and encryption, pipelined reads and writes, compound requests, share enumeration, DFS, change notification; 38 k recent downloads; updated 2026-09-29 | Compiles (`cargo xwin check`) |
| `pavao` 0.3.1              | MIT               | Binds `libsmbclient` (Samba's C client). Needs the system library and its headers; not built or measured. Because it needs `libsmbclient` it has no Windows story and ties Waypoint's runtime to a system package         | No                            |
| `gio` or `smbclient` shell | n/a               | Last resort: parse the text output. `smbclient` was run only as a reference for what the server can do                                                                                                                    | No                            |

### Measurements

Against the user-level Samba, with attributes (size, times) delivered in the listing by both crates:

| Measurement                  | `smb` 0.12.1                                                                                | `smb2` 0.27.0          | `smbclient` (reference)  |
| ---------------------------- | ------------------------------------------------------------------------------------------- | ---------------------- | ------------------------ |
| List 1,000, LAN              | 172 to 175 ms (first entry 86 ms); **6 of 12 runs hung** (30 s limit)                       | 7 to 8 ms              | not run                  |
| List 10,000, LAN             | 966 to 1,042 ms; 0 of 12 hung                                                               | 68 to 69 ms            | 126 ms                   |
| List 100,000, LAN            | 8.9 s (2 runs); **3 of 12 hung**                                                            | 675 to 687 ms (3 runs) | 877 ms                   |
| List 10,000, 100 ms          | 2.3 s (first entry 208 ms)                                                                  | 2.2 s                  | not run                  |
| List 100,000, 100 ms         | not run                                                                                     | 19.7 s                 | not run                  |
| Read 1 GiB sequentially, LAN | 24 MiB/s with 1 MiB reads (512 MiB in 21.4 s); 64 KiB reads did not finish 512 MiB in 100 s | 1,007 MiB/s (1.02 s)   | 2.5 GiB/s to `/dev/null` |
| Read 128 MiB, 100 ms         | not run                                                                                     | 35 MiB/s               | not run                  |
| Clean build, stripped binary | 18.3 s, 7.2 MB, 279 crates                                                                  | 8.5 s, 2.2 MB          | n/a                      |
| Peak memory listing 100,000  | 9 MB                                                                                        | 14 MB                  | n/a                      |

`smb` sends one request at a time (`smb2`'s README says the same, and its pipelined reads are the difference here). The 86 ms first entry and the fixed cost per round trip look like a delayed-acknowledgement stall (no `TCP_NODELAY` was found in `smb-transport`'s source), but the cause is **not established**. The hangs are real and intermittent: the listing stream stalled with no error mid-way through and had to be killed. They were seen on loopback against Samba only. Neither listing API is streaming: `smb2`'s `list_directory` returns a `Vec` (fast enough that the pause is 0.7 s at 100,000 entries on LAN, but 19.7 s at 100 ms, because the requests are not pipelined in a directory query); `smb` offers a stream.

**Dependency findings.** `smb` 0.12 and `russh` 0.63 **cannot be resolved in one Cargo workspace**: `smb` → `sspi` 0.21.3 pins `curve25519-dalek =5.0.0-rc.1`, `russh` needs `^5`. A fresh resolve of `smb` alone also failed to compile until `picky-krb` was pinned to 0.12.4 (0.12.5 added an enum variant that `sspi` does not match). `smb2` and `russh` resolve together (199 packages; resolved, not built together); `ssh2` and `smb` also resolve.

**Authentication, platform and tests.** Both pure Rust crates offer NTLM and Kerberos (domain accounts through the user's ticket). Only NTLM with a local user against Samba was run; domain, Kerberos, guest, signing-required and encryption-required servers are **unmeasured**. On Windows the best SMB client is the OS: a UNC path (`\\host\share\folder`) goes through the SMB redirector with single sign-on, the stored credentials and Kerberos, and the local `waypoint-vfs` provider already lists and copies it. That path was not run here. A user-level `smbd` is easy to stand up (`spikes/m6-remote/smb.conf`: `smb ports`, private `private dir`, `lock directory`, `state directory`, `cache directory`, `pid directory`, `ncalrpc dir`, a `tdbsam` password file and `pdbedit -a`), so the conformance suite can run `smbd` in the CI job without a container; a Samba container image is the alternative on runners where the package is missing. Neither was tried in CI.

### Recommendation: SMB

- **Linux: `smb2`**, behind the provider's Cargo feature, with the conformance suite run against `smbd`. It is 14 times faster to list and 40 times faster to read than `smb` on the same server, resolves alongside `russh`, is dual-licensed like Waypoint and builds in 8.5 s. Listing needs a streaming wrapper (read pages as the library produces them, or add one upstream) before it can meet the budget over latency; until then show a spinner for the whole `list_directory`.
- **Windows: the OS's UNC paths through the local provider**, with `smb2` as the portable fallback (it compiles). Unmeasured.
- **Do not use `smb` 0.12** (hangs, 24 MiB/s, dependency conflict). Do not use `pavao` or shell out except as a documented last resort.
- **Risk:** `smb2` is young and has one main author; 0.27 means a fast-changing API. Pin it, vendor-audit it before release and keep a path to swap in `smb`'s or the OS's implementation behind the same provider.

## WebDAV

### The candidates

| Option                                            | Licence           | Notes                                                                                                                                                                   |
| ------------------------------------------------- | ----------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Thin client: `reqwest` 0.13 with `quick-xml` 0.41 | MIT OR Apache-2.0 | `PROPFIND` and `GET` with `Range` are a few dozen lines each; basic and bearer authentication are in `reqwest`; digest is not (a small implementation or another crate) |
| `reqwest_dav` 0.3.3                               | MIT OR Apache-2.0 | A ready client over `reqwest` (lists, gets, puts, moves, copies); documented authentication modes include basic and digest; not exercised here                          |
| `dav-server` 0.11.0                               | Apache-2.0        | A **server** library, not a client; useful for the test server (CI conformance) but not for the provider                                                                |
| `rustydav` 0.1.3                                  | GPL-3.0           | Rejected on licence                                                                                                                                                     |
| `opendal` WebDAV service                          | Apache-2.0        | See the `opendal` section                                                                                                                                               |

### Measurements

`PROPFIND` with `Depth: 1` against rclone's WebDAV server, and a `GET` with a `Range` header. A `Depth: 1` `PROPFIND` is **one response for the whole directory**: the server builds all of it before sending the first byte (rclone's first byte took 417 ms for 100,000 entries), so the first rows arrive at the first bytes and there is nothing to stream before that.

| Latency | Entries | Thin: first bytes | Thin: all bytes | Thin: parsed | Body   | Thin peak memory | `reqwest_dav` total | `reqwest_dav` peak memory | `opendal` WebDAV |
| ------- | ------- | ----------------- | --------------- | ------------ | ------ | ---------------- | ------------------- | ------------------------- | ---------------- |
| LAN     | 1,000   | 1.5 ms            | 9 ms            | 10 ms        | 0.6 MB | 8 MB             | 31 ms               | 9 MB                      | 17 ms            |
| LAN     | 10,000  | 15 ms             | 97 ms           | 104 ms       | 6.2 MB | 14 MB            | 298 ms              | 25 MB                     | 133 ms (39 MB)   |
| LAN     | 100,000 | 417 ms            | 1,240 ms        | 1,312 ms     | 62 MB  | 67 MB            | 3,116 ms            | 210 MB                    | not run          |
| 100 ms  | 1,000   | 114 ms            | 124 ms          | 125 ms       | 0.6 MB | 9 MB             | not run             | not run                   | not run          |
| 100 ms  | 10,000  | 137 ms            | 215 ms          | 222 ms       | 6.2 MB | 14 MB            | not run             | not run                   | 242 ms           |

The thin client parsed the body after it arrived. `reqwest_dav` is 2.4 to 3 times slower and holds 3 times the memory at 10,000 to 100,000 entries, because it builds a full typed model of every property. `Range: bytes=1000-1999` returned `206 Partial Content` with `Content-Range: bytes 1000-1999/1073741824` and 1,000 bytes. A 1 GiB `GET` ran at 962 MiB/s on LAN and 860 MiB/s through the 100 ms proxy; the second figure only shows that a single TCP stream is not window-limited on loopback, not what a real WAN does.

**Unmeasured:** Nextcloud and ownCloud (their DAV root is `/remote.php/dav/files/<user>/`, they answer `Depth: 1` but refuse `Depth: infinity` by default, they want app passwords where two-factor is on, and they add `oc:` and `nc:` properties such as `oc:size` and the file id), IIS and Apache `mod_dav` quirks, digest authentication, TLS and redirects, locks, and `Content-Length` of a collection (not always present). Servers differ in `getcontentlength` for directories and in how they spell `href`s, so the parser must tolerate both absolute and relative, percent-encoded and unencoded forms.

### Recommendation: WebDAV

**A thin client over `reqwest` with `quick-xml`**, with the PROPFIND body parsed from the response stream in a way that yields entries in batches to the `Channel` (the whole body is one response, but the parse can begin with the first bytes and the page need not wait for 62 MB). It is the cheapest in memory and time, adds only crates Waypoint's HTTP stack needs anyway, and keeps full control of authentication prompts and redirect handling. Use `reqwest_dav` only if the thin client's authentication (digest) proves a time sink. **Risks:** all the Nextcloud and server quirks above are untested; a 100,000-entry directory is a 62 MB response, so a ceiling on the response size and an error that says so are needed.

## S3

### The candidates

| Option                                    | Licence                                   | Compile cost (clean, 16 threads)                                                                    | Notes                                                                                                                                   |
| ----------------------------------------- | ----------------------------------------- | --------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| `aws-sdk-s3` 1.152.0                      | Apache-2.0                                | 45.8 s, 267 crates, **13.2 MB stripped** (+12.5 MB over the Tokio baseline), 622 MB of build output | Official; paginator stream; credential chain (environment, profiles, SSO, container and instance metadata); retries; MSRV 1.94.1        |
| `rust-s3` 0.37.2                          | MIT (and MPL-2.0 `attohttpc` in its tree) | 23.8 s, 175 crates, 5.2 MB stripped                                                                 | Community client; `list` collects every page (a `list_page` exists, not exercised)                                                      |
| `opendal` 0.59.3 (S3 and WebDAV services) | Apache-2.0                                | 25.5 s, 238 crates, 11.8 MB stripped                                                                | See the `opendal` section                                                                                                               |
| `object_store` 0.14.2                     | MIT OR Apache-2.0                         | **not built or measured**                                                                           | S3, Azure and GCS behind one trait with a streaming `list` and `get_range`; named in the issue; its credential support was not compared |

### Measurements

Listing the bucket against rclone's S3 server, path-style with a custom endpoint, 1,000 keys per page. rclone appears to re-read the directory for every page, so its time grows with the position in the listing: at 100,000 keys the page time reached over a second. **These totals measure rclone, not the SDK**; they only show that all three clients paginate, use the endpoint and path style and return every key.

| Measurement                   | `aws-sdk-s3`                                | `rust-s3`          | `opendal`                  |
| ----------------------------- | ------------------------------------------- | ------------------ | -------------------------- |
| 1,000 keys, LAN               | 23 ms, one page                             | not run            | 11 ms                      |
| 10,000 keys, LAN              | 393 ms, 10 pages (first page 122 ms)        | 304 ms (all pages) | 308 ms (first entry 34 ms) |
| 100,000 keys, LAN             | 34.6 s, 100 pages (first page 1.27 s)       | 33.2 s (all pages) | not run                    |
| 1,000 and 10,000 keys, 100 ms | 120 ms and 1,412 ms (about 120 ms per page) | not run            | 1,327 ms for 10,000        |
| Range `bytes=1000-1999`       | `Content-Range` and 1,000 bytes returned    | not run            | not run                    |
| Peak memory listing           | 11 to 12 MB                                 | not measured       | 11 MB                      |

Listing is **sequential by nature**: each page needs the previous page's continuation token, so 100,000 keys cost at least 100 round trips (about 12 s at 100 ms with an infinitely fast server; not measured at 100 ms). Splitting the work by prefix with `delimiter=/` and listing the sub-prefixes at once is the way to go faster. Large object reads, multipart upload, presigned URLs, SigV4 with real AWS, R2 and MinIO quirks, virtual-host style and region redirects were **not measured**.

### Recommendation: S3

**`aws-sdk-s3`**, behind the provider's Cargo feature, with `force_path_style` and `endpoint_url` for MinIO, R2 and other S3-compatible stores. It is the one whose behaviour on real AWS is certain, and its credential chain is what users with an `~/.aws` profile or SSO expect. The price is measured: about 46 s of clean build and 12.5 MB of binary, which is why the feature must be switchable and why the release size should be watched. `rust-s3` halves both but is community-maintained and has no paginator stream. `object_store` is the credible lighter alternative and should be measured if the AWS SDK's cost is rejected. **Risk:** build time and size; the SDK's MSRV (1.94.1) follows the toolchain closely.

## `opendal` as one abstraction

`opendal` 0.59 offers one `Operator` over S3, WebDAV, SFTP and more, with a streaming `lister`, ranged `read` and `stat`, and no watch, which are the verbs of Waypoint's provider contract. Measured here: S3 listing streams a page at a time (first entry 34 ms for 10,000 keys, against 122 ms to the first page with the AWS SDK on the same server), WebDAV listing returns everything at once (first entry equals total, 133 ms and 39 MB for 10,000 entries against the thin client's 104 ms and 14 MB), and the S3 plus WebDAV build is 25.5 s and 11.8 MB.

**It is not right for Waypoint's provider contract.** (1) Its SFTP service is built on the `openssh` crate, which drives the system `ssh` binary and does not compile on Windows (verified under `cargo xwin check`), so it cannot replace `russh` and gives no host key callback, no `window_size` and no pipelined listing. (2) Its listing entries are a path plus whatever metadata the service returned, without the permissions, owners, symlink targets, tags, storage classes and versions that Waypoint's columns and the Inspector show; it would need a `stat` per entry or a side channel. (3) Authentication prompts, the known-hosts prompt, keyboard-interactive and the Services status panel's reasons all live in protocol-specific code that `opendal` hides. (4) It is a 0.x crate that changed under the spike's feet: with default features off, 0.59 refused to list ("default HTTP transport is not installed") until the `http-transport-reqwest`, `executors-tokio` and `auto-register-services` features were enabled. (5) It adds 238 crates to cover protocols Waypoint already covers with a better-suited library each. SMB was not evaluated in `opendal`. Use the native libraries above, one provider crate each, and keep `opendal` as a possible future plugin backend for exotic stores.

## Findings that surprised

1. **`russh-sftp`'s `SftpSession::read_dir` is quadratic** in the number of batches and is not streaming; 3.7 times slower than the same requests issued by hand at 100,000 entries.
2. **The default `russh` window caps a transfer at 9 to 10 MiB/s at 100 ms**, whatever the number of SFTP requests in flight; 8 MiB triples it.
3. **`smb` 0.12 hangs intermittently** while listing (6 of 12, 0 of 12 and 3 of 12 runs at 1,000, 10,000 and 100,000 entries), reads at 24 MiB/s, conflicts with `russh` in a Cargo workspace and does not compile from a fresh resolve without a pin. `smb2` beats it by 14 to 40 times.
4. **`openssh` (and so `opendal`'s SFTP) does not compile for Windows.**
5. **OpenSSH returns only about 100 entries per `readdir`**, so a 100,000-entry directory is 1,001 round trips: 103 s at 100 ms without pipelining.
6. **A user-level `sshd` and `smbd` need no root**, which makes real-server conformance tests possible in CI without containers (`sshd` needs `-D`; its forking mode with `-E` failed to answer here).

## Not verified

- **Windows at run time**, for every library: only `cargo xwin check` (compiles, does not link or run). WinCNG's key-type limits, Pageant and named pipe agents, `aws-lc-sys` and OS UNC paths were not exercised.
- **A real network**: only loopback with a fixed delay, no loss, jitter, bandwidth limit or TCP handshake delay, and a fast server. Real SFTP servers (Synology, Windows OpenSSH, embedded) may return fewer entries per `readdir` or limit requests in flight.
- **Authentication beyond keys and the agent**: password, keyboard-interactive, GSSAPI, certificates, FIDO keys, `ssh2`'s passphrase and agent paths; SMB domain, Kerberos, signing and encryption; WebDAV digest and bearer; real AWS credentials and SSO.
- **Servers other than OpenSSH, Samba and rclone**: Nextcloud, IIS, Apache `mod_dav`, MinIO, AWS S3, R2.
- **SSH config parsing** (`russh-config`, `ssh2-config`), **`object_store`**, **`pavao`** and the **`gio` backend**.
- **Large S3 reads and S3 at 100,000 keys with latency**, `rust-s3` and `opendal` at 100,000 keys, and every WebDAV and S3 figure beyond the server's own limits.
- **Incremental compile time** (only clean builds were timed), release size with Waypoint's own profile and link-time optimisation, and the cost of several of these libraries in one binary.
- **Memory per entry with full metadata**, and the full pipeline to the page (`Channel`, listing handle, rendering): the transport was measured alone.
- **Flatpak**: nothing was run in a sandbox. None of the recommended libraries needs a daemon; the SSH agent needs the `ssh-auth` socket permission, and `smb2` and `russh` need no system library.
