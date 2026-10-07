# Milestone 7 spike results: search engine and its budget

This records the search spike (#315): how fast a search can be by walking the folders, what an optional index adds and costs, and the budgets that follow (A120). It says what was measured, on what, and what is **unmeasured**. Every number is from one Linux machine; nothing was run on Windows, where only a type-check was done. The terminal spike (#316) records its results in this file under a heading of its own.

The benchmark was throwaway: a scratch Cargo workspace outside the repository, deleted after the run. Its shape is described under [Reproduce](#reproduce) so the tables can be measured again.

## Setup

| Item        | Value                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Machine     | AMD Ryzen 7 7700X (8 cores, 16 threads), 30 GB RAM of which only about 4 GB was free during the runs (other builds held the rest and the 15 GB of swap was full), NVMe, Linux 7.1.9 (Arch), Rust 1.98.0                                                                                                                                                                                                                                          |
| File system | ext4 in a 7 GB image file on the btrfs Home, attached as a loop device and mounted with `udisksctl` (no root needed)                                                                                                                                                                                                                                                                                                                             |
| Cold cache  | Dropping caches needs root, so the tree was made cold by unmounting it, detaching the loop device, `fsync` and `POSIX_FADV_DONTNEED` on the image (`fincore` then reports 0 bytes cached), and attaching and mounting it again. That drops the tree's dentries, inodes and pages, so cold here is truly cold; it replaces the planned method of reading the tree first after creating a second one, which on 30 GB of RAM would have stayed warm |
| Loop device | Buffered and single-queue, so a cold read is slower than a native NVMe mount, and cached pages are held twice (by the loop file system and by the image file). Cold figures are pessimistic for an NVMe disk and optimistic for a spinning one                                                                                                                                                                                                   |
| Tree        | 500,002 files and 53,040 folders (552,502 entries with the symlinks), 1.92 GB of data, 3.1 GB on disk; see below                                                                                                                                                                                                                                                                                                                                 |
| Method      | Release builds, one process per measurement, 16 threads unless a row says otherwise, `Instant` timers, `VmHWM` from `/proc` for the peak resident memory of the whole process. **First result** is the time from the start to the first match handed on; each match is `lstat`ed, as the results listing needs its size and date. Matching ignores case                                                                                          |
| Repeats     | Warm runs repeated three times; the middle one is given, and the three were within about 10 per cent. Cold runs are one run each. The tables are not statistics                                                                                                                                                                                                                                                                                  |

The tree is shaped like a Home: `Documents` (60,000 text and document files in nested folders), `Pictures` (40,000 `IMG_nnnn.jpg` whose names repeat across folders), `Music` (20,000), `Downloads/flat` (20,000 files in one folder) with a 512 MiB text log (a rare word near its end) and a 256 MiB random binary, `Projects` and `Work` (40 repositories, 220,000 files, each with a `.gitignore` of `target/`, `node_modules/` and `*.log`, and a `.git`), a hidden `.cache` of 100,000 files and `.config` and `.local/share` of 40,000. Text files are 100 B to 4 KB of words or code; binary files are 128 B to 8 KB of random bytes with a NUL near the start. It also holds a folder nobody can read (mode `000`, 100 files), two symlinks that point at each other, a link to `..` and a link to `Projects`.

## Name search by walking

All five walkers found the same 766 matches for "report" in 552,502 entries; the unreadable folder is the one error (jwalk reports none).

| Walker                                                     | Warm: first result | Warm: all | Cold: first result | Cold: all | Peak memory |
| ---------------------------------------------------------- | -----------------: | --------: | -----------------: | --------: | ----------: |
| `walkdir` 2.5 (one thread)                                 |             112 ms |    381 ms |           1,297 ms |  3,086 ms |        5 MB |
| `jwalk` 0.9 (rayon, 16)                                    |              41 ms |     91 ms |           1,026 ms |  2,006 ms |    90–97 MB |
| `ignore` 0.4.33 `WalkParallel`, filters off                |            2–14 ms |     51 ms |             422 ms |  1,985 ms |       17 MB |
| `ignore` with `.gitignore` (481,410 entries, 71,092 fewer) |           10–18 ms |     53 ms |             424 ms |  2,100 ms |       17 MB |
| `std::fs::read_dir` on 16 threads, written for the spike   |             0.4 ms |     41 ms |              38 ms |  2,138 ms |        7 MB |

`ignore` across thread counts (warm, all / first): 1 thread 434 / 124 ms, 2 threads 225 / 69 ms, 4 threads 116 / 9 ms, 8 threads 68 / 13 ms, 16 threads 47 / 11 ms.

Other queries with `ignore` on 16 threads, warm (all / first): a rare name, "invoice_2024" (35 matches), 51 / 11 ms; the glob `*.pdf` with `globset` (16,117 matches) 56 / 11 ms; the regex `^img_\d{4}\.jpg$` with `regex` (32,160 matches) 58 / 16 ms; every entry, each `lstat`ed, 121 / 0.2 ms. The hand-written walk was 39, 45, 46 and 112 ms for the same four.

A size or date filter needs the metadata of every entry, not only of matches: `lstat` of all 552,502 entries took 111 to 217 ms warm and 2,041 ms cold. On Linux a walk without it reads only folders (the kind comes from `d_type`).

What the walk shows:

1. **Warm, a name search of 500,000 entries is done in about 50 ms** on 16 threads and about 120 ms on 4. The first result comes in under 20 ms. No index is needed for that case.
2. **Cold, it takes about 2 seconds** on this disk, whatever the walker, because it is bound by reading 53,040 folders; the first result takes 40 to 1,300 ms depending on the order the walker visits folders in. The cold case is what an index is for.
3. **Respecting `.gitignore` saves nothing warm** (53 against 51 ms here) and 13 per cent of the entries. It is a filter for what people want to see, not a speed-up.
4. `jwalk` costs five times the memory of `ignore` for a slower result; `walkdir` is single-threaded. The hand-written walk is 20 per cent faster than `ignore` and gives its first result sooner, but `ignore` brings ignore files, `same_file_system`, `max_filesize`, hidden-file rules and years of use on Windows through ripgrep.

## Content search

`ignore` feeding `grep-searcher` 0.1.17 with `grep-regex` 0.1.14: the first match in a file lists it (with its line as the snippet) and the search moves on; binary detection quits a file at its first NUL; files over a size cap are counted and passed over.

**Warm**, on the `Projects` subtree (110,000 files, 173 MB), which fitted in the free page cache:

| Variant                                                         | All    | First result |
| --------------------------------------------------------------- | ------ | ------------ |
| 16 threads, reading (`MmapChoice::never`)                       | 66 ms  | 6–18 ms      |
| 16 threads, `MmapChoice::auto`                                  | 556 ms | 17 ms        |
| 16 threads, the regex `fn\s+\w+_handler\(` (87,272 files match) | 67 ms  | 3 ms         |
| 4 threads, reading                                              | 174 ms | 27 ms        |
| 1 thread, reading                                               | 680 ms | 196 ms       |

**The whole tree** (499,456 files, 1,060 MiB under a 64 MiB cap) took **63 s cold** (first result after 1.7 s). Run again it took 38 to 47 s, not the half second the warm rate above predicts: the tree's 2 GB or more of pages did not fit in the 3.7 GB the machine had free, so most reads went to the disk again (the process used 6.6 s of processor time in 38 s). A warm full-tree content search was therefore **not measured**; a person's Home on an ordinary machine is in the same position, so a content search over all of Home should be expected to run for tens of seconds and must stream and report progress. With the cap at 1 GiB (the 512 MiB log and the 256 MiB binary searched too) it was 41.6 s and 1,828 MiB; respecting `.gitignore` searched 433,456 files and 945 MiB in 40.4 s.

Memory stayed at 17 to 18 MB for the whole process while reading. With `mmap` the resident figure rose to 260 to 515 MB, the pages of the large files mapped (not allocations).

### Cancelling

The search was cancelled after a delay and timed from the cancel to the moment every worker had returned. Each worker checks the cancel between files; while reading, it also reads through a wrapper that fails once the cancel is set, so a long file stops within one 64 KiB buffer.

| Where                                       | Reading with the cancel check | `mmap`              |
| ------------------------------------------- | ----------------------------- | ------------------- |
| Whole tree, cancelled at 0.4, 0.8 and 1.2 s | 3.4, 1.8 and 1.3 ms           | 373, 195 and 182 ms |
| `Downloads`, cancelled at 30, 60 and 90 ms  | 2.8, 1.8 and 1.7 ms           | 189, 9.2 and 1.8 ms |

A mapped file cannot be interrupted: the worker inside the 512 MiB log finishes it first. Reading through the cancel check meets a 50 ms cancel with room to spare, and is also faster on many small files, so **content search reads and never maps**.

## An optional index

Four ways to keep a names index of the same 552,502 entries (name, folder, kind, size, modified time) were built from one walk with `lstat` of every entry (130 to 190 ms warm), then queried warm (the middle of 20 runs after a first one that includes opening the index) and given 3,000 changes as a watcher would report them (1,000 created, 1,000 removed and 1,000 renamed) and made durable.

| Index                                                                                                                                                 | Build after the walk |  On disk | Memory to build |
| ----------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------: | -------: | --------------: |
| **Plain table**: names (9.4 MB), offsets (2.2 MB), fixed 24-byte records (13.3 MB), and an `fst` 0.4 map of names for prefixes (12.8 MB)              |               539 ms | 35.9 MiB |          143 MB |
| **Trigram postings** over the same table (9.9 MB, delta-coded)                                                                                        |               209 ms | 33.2 MiB |          144 MB |
| **tantivy** 0.26: the name as one raw term, id, parent, size and time as fast fields                                                                  |             2,866 ms | 15.3 MiB |          264 MB |
| **SQLite FTS5** (rusqlite 0.40, bundled SQLite): a table of entries and an external-content FTS5 table with the `trigram` tokenizer and `detail=none` |               902 ms | 37.6 MiB |          106 MB |

Query times, in milliseconds (first query including opening the index in brackets):

| Query (matches)                   |  Plain table |       Trigram | tantivy | SQLite FTS5 |
| --------------------------------- | -----------: | ------------: | ------: | ----------: |
| "report" (766)                    |  0.44 (0.99) |   0.02 (0.22) | 88 (87) | 0.38 (2.18) |
| "invoice_2024" (35)               |         0.17 |          0.03 |      92 |        0.12 |
| "qz", two letters (0)             |         0.11 | 0.11 (a scan) |      81 | 26 (a scan) |
| prefix "img_00" (319)             | 0.03 (`fst`) |          0.11 |     2.2 |        0.19 |
| regex `^img_\d{4}\.jpg$` (32,160) |          4.9 |          0.78 |     206 |        12.8 |
| glob `*.pdf` (16,117)             |         10.8 |          0.53 |     182 |         7.6 |

The regex and glob rows use the longest literal the pattern must contain (from `regex-syntax`) to choose candidates, then match each one. Memory while querying was 10 to 20 MB.

Applying the 3,000 changes:

| Index       | Cost                                                                                                                                                                         |
| ----------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Plain table | Appending records and tombstones, 66 ms with 50 ms of `fsync`; the `fst` cannot change and took 498 ms to rebuild                                                            |
| Trigram     | In memory, 0.69 ms for 2,000 additions; appending as above, 48 ms with `fsync`; rewriting the postings file, 164 ms                                                          |
| tantivy     | 536 ms, of which the commit was 366 ms                                                                                                                                       |
| SQLite FTS5 | 472 ms in one transaction (157 µs a change, including reading each old name to delete it), of which the commit was 100 ms; the file grew to 44 MiB until the next checkpoint |

What the index shows:

1. **Every query on a names index is under 30 ms, and most under 1 ms.** The difference between the formats does not matter at this size; what matters is not reading 53,040 folders. A plain scan of the names (the plain table, no index structure at all) already answers a substring in under half a millisecond.
2. **tantivy is the wrong tool for names.** It is built for words in documents: a substring is a regex over its whole term dictionary (80 to 200 ms), it costs 109 crates and the slowest build and commit. It stays the candidate if contents are ever indexed.
3. **A plain `fst` is the fastest for prefixes but cannot change**: every batch of changes means a rebuild (0.5 s here) or a second, smaller map to merge.
4. **SQLite FTS5 is slower than a hand-made trigram index by a factor of 10 to 20 and still far inside any budget**, and it is transactional (a crash in the middle of an update leaves the last committed state), is one file whose size and entry count are a `stat` and a `count`, filters on size, date and kind in the same query, and costs 9 crates. A hand-made format would have to earn crash safety, compaction and versioning itself.

## Watching for changes

With inotify, one watch per folder: adding watches for all 53,040 folders took 500 ms (the unreadable folder failed), and the 3,000 events of 1,000 files created and removed in a watched folder were all read within 26 ms of the first write (the writes themselves took 25 ms). The kernel's own estimate is about 1 KB of unswappable memory per watch, so about 55 MB for this tree (**not measured**). Watches count against `fs.inotify.max_user_watches` (524,288 here, much lower on older systems), which every program of the user shares. fanotify can watch a whole file system with one mark but needs `CAP_SYS_ADMIN`.

An index that was not running while things changed has to catch up. A folder's modified time changes when an entry is added, removed or renamed in it, so comparing the folders' times finds every folder whose names changed: `lstat` of the 53,040 folders took **10 ms warm and 1,414 ms cold** (against 1,983 ms for the cold walk itself). Cold, a catch-up check saves little over a walk; warm, it is almost free. A file whose contents change does not change its folder's time, which is fine for a names index.

## Reading tags while searching

A tag filter without an index reads each file's `user.xdg.tags`. With the attribute set on one file in a hundred (4,985 files), `lgetxattr` on all 499,458 files took **113 to 229 ms warm and 2,114 ms cold**, the same order as an `lstat` of every entry, because ext4 keeps small attributes in the inode. Writing the attribute on 4,985 files took 2.3 s during a walk. btrfs, which keeps attributes as separate items, and Windows, which has no `user.` attributes, were **not measured**.

## Windows

**Nothing was run on Windows.** The whole scratch crate, including the bundled SQLite (compiled by `cc` with `clang-cl`), tantivy, `ignore`, `grep-searcher`, `jwalk` and a placeholder check through `MetadataExt::file_attributes`, passes `cargo xwin check --target x86_64-pc-windows-msvc`. The rest is from the libraries' and Microsoft's documentation:

- **Listing.** `std::fs::read_dir` uses the `FindFirstFile` family of calls, and `DirEntry::metadata` on Windows returns the attributes, size and times the listing already carried, with no extra call and without opening the file (the standard library documents it as cheap there). `ignore` and `grep-searcher` are ripgrep's, which is used on Windows every day.
- **Cloud placeholders are never hydrated.** OneDrive and other Cloud Files API providers mark a file whose data is not local with `FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS` (reading it downloads it) or `FILE_ATTRIBUTE_OFFLINE`, and a folder whose listing is not local with `FILE_ATTRIBUTE_RECALL_ON_OPEN` (listing it fetches it). The search checks those three bits from the listing's own attributes before it opens anything: a placeholder file is matched by name but never read for content, and a placeholder folder is never descended into. That is the rule `waypoint-vfs` already follows for folder sizes (`local.rs` `is_placeholder`, A64, A70). The attributes say whether a file is pinned or present, so a file that is present locally (`PINNED` or fully hydrated) can be searched; whether OneDrive's "Files On-Demand" ever reports a present file with a recall bit still set is **unverified**.
- **Antivirus.** Windows Defender scans a file when it is opened, which is known to slow ripgrep on many small files; content search on Windows should be expected to be slower than here (**unmeasured**).
- **Watching.** `ReadDirectoryChangesW` watches a whole tree with one handle (`bWatchSubtree`), so Windows needs no watch per folder; its buffer can overflow, after which the folder must be rescanned (`notify` 8, which Waypoint already uses, reports that).
- **The NTFS master file table and the USN journal** are how Everything indexes a disk in seconds and stays current. Both are read through a handle to the volume (`\\.\C:`, `FSCTL_ENUM_USN_DATA`, `FSCTL_READ_USN_JOURNAL`), which needs administrator rights, so they need an elevated helper service, which Waypoint does not have. They are a possible index source for later, **unmeasured** here.
- **Text encodings.** Windows text files are often UTF-16 with a byte order mark, which has NULs and would be taken for binary; ripgrep reads such files through `encoding_rs_io` when it sees the mark. The content search should do the same (**untested**).

## Policies the measurements support

- **Symlinks are never followed** by a search, as the folder size scan does not follow them (A70): a link's own name can match, and it is listed as the link it is. No loop can happen; the two looping links and the link to `..` in the tree were listed and not entered.
- **Unreadable folders are counted, not fatal.** The search goes on and the results say how many folders could not be read.
- **A search stays on the volume of its scope**, as the folder size scan does (A70; `ignore`'s `same_file_system`), so a FUSE or network mount in Home (rclone, a GVFS mount, a mounted share), where reading can mean downloading, is not walked unless it is the scope.
- **Content search skips binary files** at their first NUL (counted) and **files over a size cap** (64 MiB by default, counted); a trial on a smaller tree found the word near the end of the 512 MiB log in 93 ms warm, so the cap is about predictability rather than speed, and a person can raise it.
- **Ignore files are a filter, off by default.** A file manager that hides what `.gitignore` lists would hide `node_modules` and build output people do look for; a toggle respects `.gitignore`, `.ignore` and the global Git excludes.
- **Hidden files follow the search bar's hidden toggle**, with the platform's rules for what is hidden (dot names on Linux, the hidden attribute on Windows).

## Budgets proposed

| What                                          | Budget                                                                                                            | Measured here                                                        |
| --------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| Name search by walking, 500,000 entries, warm | First result under 100 ms; all under 1 s on 4 threads                                                             | 2–16 ms; 47 ms (16 threads), 116 ms (4)                              |
| Name search by walking, cold                  | First progress under 100 ms; results stream; no total budget                                                      | 2 s on this disk                                                     |
| Name search with a complete index             | First result under 50 ms, including opening the index                                                             | 0.2–26 ms (SQLite FTS5)                                              |
| Content search                                | Progress (files and bytes searched, the folder being searched) at least every 250 ms; first progress under 100 ms | — (the harness counted 100 ms ticks)                                 |
| Cancel (Stop, the tab navigating or closing)  | Every worker stopped within 50 ms                                                                                 | 1.3–3.4 ms reading                                                   |
| Memory of a search                            | Under 64 MB above the app                                                                                         | 17 MB for the whole process                                          |
| Index build, 500,000 entries                  | Under 10 s warm, below the operations pool's priority                                                             | 0.15 s walk and 0.9 s build                                          |
| Index size                                    | Under 150 bytes per entry                                                                                         | 71 bytes per entry (37.6 MiB)                                        |
| A change reaching the index                   | Within 2 s of the event                                                                                           | inotify delivers in milliseconds; 157 µs per change in a transaction |

## Recommendation

1. **Both: the walk is the engine and an index only speeds it up.** Search must be complete and correct with the index off (the owner's decision); warm, a walk already meets the name budget, and the index is what turns a 2 s cold walk into a few milliseconds. With the index on and complete for a location, a name search there reads the index and checks each hit with an `lstat` as it is listed (which also refreshes its size and date and drops hits that have gone); a location the index does not cover, or one still being built, is walked.
2. **Walk with `ignore`** (`WalkParallel`, standard filters off unless the ignore-files toggle is on, hidden files per the toggle, `follow_links(false)`, `same_file_system(true)`), on a pool of its own sized to the available parallelism and capped at 8 threads, so the interface and the operations pool keep their share. Match names with `memchr::memmem` for plain text, `globset` for globs and `regex` for regular expressions.
3. **Search contents with `grep-searcher` and `grep-regex`**, reading through a cancel check and never mapping, binary detection on, a size cap, BOM sniffing for UTF-16, and placeholders never opened.
4. **Keep the optional index in SQLite with FTS5** (`rusqlite` with the bundled SQLite): one file in the app's data directory with a schema version (`PRAGMA user_version`), a table of folders and a table of entries (folder, name, kind, size, modified time) with an external-content FTS5 table over the names using the `trigram` tokenizer and `detail=none`. Names only in milestone 7; contents are not indexed. Deleting the index is deleting its file.
5. **Update it from watcher events applied in batches**: inotify per folder on Linux (counting the watches against the system limit and saying in the status when a location had to fall back), `ReadDirectoryChangesW` on Windows through `notify`; changes gathered for about a second and applied in one transaction; a catch-up at start that compares each folder's modified time and relists the folders that changed; a full rebuild on request.
6. **Do not use** tantivy (until contents are indexed), `jwalk` (memory) or `mmap` for content search (cancel). Leave the NTFS master file table and USN journal for a later elevated helper.

## Not verified

- **Windows at run time**: listing speed, the placeholder bits on a real OneDrive, Defender's cost, `ReadDirectoryChangesW` overflow, UTF-16 files. Only `cargo xwin check` (it compiles; it does not link or run).
- **A warm content search of the whole tree**, which did not fit in the free memory; the warm rate is from a 110,000-file subtree.
- **Other disks and file systems**: native NVMe without a loop device, btrfs (the real Home here), a spinning disk, a network share, NTFS.
- **The full pipeline to the page**: results through a listing handle and a `Channel` into the virtual list were not measured, only the engine.
- **inotify's kernel memory**, the watch limit on other distributions, fanotify, and how other programs' watches compete.
- **Lower I/O priority** for content search and indexing (`ioprio_set` idle class on Linux, background mode on Windows) and power: pausing an index build on battery.
- **SQLite under concurrent search and update**, its WAL growth over days, and the index of a Home of several million entries.

## Reproduce

The scratch workspace was one binary crate with `ignore`, `walkdir`, `jwalk`, `grep-searcher`, `grep-regex`, `grep-matcher`, `regex`, `regex-syntax`, `globset`, `memchr`, `memmap2`, `fst`, `tantivy`, `rusqlite` (feature `bundled`) and, on Linux, `inotify` and `libc`, with the subcommands `gen` (the tree, from a seed), `walk <engine> <root> <name|glob|regex|none> <pattern> <threads> <stat-all>`, `content <root> <regex> <mmap> <cap MiB> <threads> <gitignore> <cancel after ms>`, `index-build`, `index-query` and `index-update <table|trigram|tantivy|sqlite>`, `watch`, `statlist` (stat a list of folders) and `xattr <root> <set|read>`. Set `CARGO_TARGET_DIR` to a disk, not to `/tmp`.

```bash
# a file system that can be made cold without root (needs about 3.5 GB)
truncate -s 7G tree.img && chattr +C tree.img
mkfs.ext4 -q -F -N 900000 -E root_owner=$(id -u):$(id -g) -L wpm7 tree.img
udisksctl loop-setup -f tree.img && udisksctl mount -b /dev/loopN   # mounted at /run/media/$USER/wpm7
# cold: unmount, detach, drop the image's pages, attach and mount again
udisksctl unmount -b /dev/loopN && udisksctl loop-delete -b /dev/loopN
python3 -c "import os; f=os.open('tree.img', os.O_RDONLY); os.fsync(f); os.posix_fadvise(f, 0, 0, os.POSIX_FADV_DONTNEED)"
fincore tree.img   # 0B
# clean up: unmount, loop-delete, rm tree.img and the target directory
```

# Milestone 7 spike results: terminal renderer and PTY (#316)

This records the terminal spike (#316): which PTY library and which renderer the terminal drawer, the Terminal tab and SSH terminals (#324 to #328) should use, how the bytes get from the shell to the page, and the budgets that follow (A130 to A132). It says what was measured, on what, and what is **unmeasured**. Everything was measured on one Linux machine; nothing was run on Windows, with Orca, with an input method, in a Flatpak or on KDE.

The prototype was throwaway: a scratch Cargo workspace outside the repository (a Tauri v2 app with a plain HTML page, a PTY benchmark, an `russh` shell client and a `cargo xwin check` crate), deleted after the run. The tables can be measured again from the descriptions in [Method](#method).

## Setup

| Item        | Value                                                                                                                                                                                                                                                    |
| ----------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Machine     | AMD Ryzen 7 7700X (16 threads), 30 GB RAM, NVMe, Linux 7.1.9 (Arch), Rust 1.98.0                                                                                                                                                                         |
| Webview     | WebKitGTK 2.52.6 (`webkit2gtk-4.1`) in a Tauri 2.12 release build, GNOME Shell 50.4 on Wayland, one display at 60 Hz (`requestAnimationFrame` ran at 60 to 62 Hz), device pixel ratio 2, WebGL 2 available                                               |
| Fixtures    | A 500,000,128-byte text file on disk (page cache warm) of 3 lines of `ls -l` style output with colour escapes to 1 line of mixed Japanese, emoji and accented text; a 200 MB file of 24-bit colour escapes (about 3 escapes per word)                    |
| Terminal    | 117 columns by 36 rows (a 1000 by 700 px window, 14 px monospace), `scrollback` 100,000 unless stated                                                                                                                                                    |
| CPU and RSS | Read from `/proc` for the app process and, summed, for the `WebKit*` helper processes beneath it. **Percent of one core**, so 130 % is 1.3 cores. RSS is the sum over the WebKit helpers; an idle page with the terminal not yet open costs about 255 MB |
| Repeats     | One run per cell unless said. Timings were stable within a few per cent where repeated. `performance.now()` in WebKit has 1 ms resolution, so latencies under 1 ms read as 0                                                                             |

## PTY libraries

### The candidates

| Library                                      | Licence (and dependencies)                                                      | Maintenance (crates.io, 2026-10-06)                          | Async                                                       | Windows                                        |
| -------------------------------------------- | ------------------------------------------------------------------------------- | ------------------------------------------------------------ | ----------------------------------------------------------- | ---------------------------------------------- |
| `portable-pty` 0.9.0 (WezTerm)               | MIT; 28 crates, all MIT or Apache-2.0 (`serial2` is BSD-2-Clause OR Apache-2.0) | Last release 2025-02-11 (20 months), 10.5 M recent downloads | Blocking only: a reader thread and a writer                 | ConPTY, compiles under `cargo xwin check`      |
| `pty-process` 0.5.3                          | MIT; 29 crates, all MIT or Apache-2.0 (`rustix` adds LLVM exception)            | 2025-07-12, 1.5 M recent downloads                           | Tokio (`AsyncRead` and `AsyncWrite`) and a blocking flavour | Unix only                                      |
| `nix` 0.31 or `rustix` 1.1 `openpty`         | MIT; MIT OR Apache-2.0 (with LLVM exception)                                    | Both updated in 2026, 190 M and 280 M recent downloads       | Whatever you build (raw file descriptors)                   | Unix only                                      |
| `conpty` 0.7.0                               | MIT; 12 crates, all MIT or Apache-2.0                                           | 2024-09-23, 0.3 M recent downloads                           | Blocking `Read` and `Write` on the process                  | ConPTY only, compiles under `cargo xwin check` |
| `alacritty_terminal` 0.26 (`rustix-openpty`) | Apache-2.0; 55 crates, all MIT or Apache-2.0 (or Unlicense OR MIT)              | 2026-04-06, MSRV 1.85                                        | Its own event loop thread                                   | Has its own ConPTY backend; not checked        |

All of them pass the Apache or MIT rule. `portable-pty` is the only one that covers both targets with one API, and is what the plan in [`crates-and-plugins.md`](crates-and-plugins.md) names. Its risks are the age of the last release and that it is one crate in the WezTerm repository, so the plugin keeps the library behind a small trait.

### Measurements

Four ways to open a PTY and spawn a child were driven by one harness reading in 64 KiB buffers: `portable-pty`, `pty-process` blocking, `pty-process` async on Tokio, and a hand-written `nix::pty::openpty` plus `std::process::Command` with `setsid` and `TIOCSCTTY`.

| Test                                        |         `portable-pty` |  `pty-process` | async `pty-process` |  `nix` by hand |
| ------------------------------------------- | ---------------------: | -------------: | ------------------: | -------------: |
| `cat` of the 500 MB file, MB/s              |                    184 |            183 |                 219 |            191 |
| Reads per second of that run (average size) |         1.07 M (472 B) | 1.02 M (497 B) |      0.93 M (542 B) | 1.06 M (477 B) |
| Keystroke echo through the line discipline  |  4 µs median, 6 µs p99 |     4 µs, 6 µs |         6 µs, 12 µs |     4 µs, 5 µs |
| Round trip through a `cat` child            | 9 µs median, 18 µs p99 |    9 µs, 15 µs |             not run |    9 µs, 16 µs |

- **The libraries do not differ.** Throughput and latency are the kernel's. The choice is by API, platforms and async story, not speed.
- **The kernel path, not the library, is the ceiling:** `cat` of a file runs at 160 to 220 MB/s, `find / -xdev` at 25 to 43 MB/s (output arrives as short lines), and `timeout 5 yes` at **14 MB/s** whatever the reader does, because the tty's output processing (`ONLCR`) handles each of its 3-byte lines one at a time.
- **A reader that sends every `read` as a message sends far too many.** The pty hands over small pieces (an average of 14 bytes for `yes`, 174 to 477 bytes for `find` and `cat`, which is about a million messages for the 500 MB file). Coalescing reads into one buffer, sent when it holds 48 KiB or 4 ms have passed since its first byte, cut 1,071,434 reads to 10,177 batches (average 49,664 bytes) for `cat` and 5.2 M to 1,427 for `yes`, at the same throughput.
- **Reads split UTF-8 sequences.** With 64 KiB buffers, 9,850 of the 1.07 M chunks of the `cat` run ended inside a multi-byte character (116 of 10,177 after coalescing), so the page must decode as a stream, never per chunk. `xterm.js` does when given bytes (`Uint8Array`); it must not be given strings that were decoded in Rust.
- **Resize and signals work the same in all four.** `TIOCSWINSZ` made a shell's `SIGWINCH` trap report the new size (`24 80` then `40 132`); a `0x03` byte written to the master ended a foreground `sleep` in under a millisecond; the child is a session leader with its own process group and the pty as its controlling terminal (`PGID` equal to `SID` equal to the pid, `TTY pts/N`); `kill(-pgid, SIGHUP)` took all three processes of a pipeline (`sleep` twice and the shell) down.
- **Foreground job detection works with no shell cooperation.** `tcgetpgrp` on the master returns the shell's own process group at the prompt and the job's while a command runs (checked with `bash -i` and `sleep 3`). The Linux fallback for the title and for idleness is therefore `tcgetpgrp(master) == shell pid`.

Not measured: `portable-pty`'s ConPTY at run time; see [Windows](#windows).

## The data path to the page

A Tauri v2 command held a `Channel<InvokeResponseBody>` and sent 64 KiB chunks of the fixture; the page counted bytes and acknowledged each chunk. "Window" is the number of unacknowledged bytes the sender may have in flight (0 is none, so no back pressure).

| Transport (500 MB unless noted)                                           | Window | MB/s | App CPU | WebKit CPU |                WebKit RSS | Frames                    |
| ------------------------------------------------------------------------- | -----: | ---: | ------: | ---------: | ------------------------: | ------------------------- |
| `Channel`, raw binary, 64 KiB chunks                                      |  1 MiB |  382 |   138 % |      185 % |                    300 MB | 62 fps, longest gap 18 ms |
| `Channel`, raw, 64 KiB                                                    |  4 MiB |  361 |   130 % |      176 % |                    377 MB | 62 fps                    |
| `Channel`, raw, 256 KiB chunks                                            |  4 MiB |  493 |   124 % |      168 % |                    295 MB | 62 fps                    |
| `Channel`, raw, 16 KiB chunks                                             |  1 MiB |  111 |   132 % |      153 % |                    604 MB | 62 fps                    |
| `Channel`, raw, 64 KiB, **no window**                                     |      0 |  481 |   159 % |      194 % | **1,360 MB** (app 539 MB) | **19 fps, 302 ms stall**  |
| `Channel` carrying JSON number arrays (32 MB)                             |  1 MiB |   59 |    83 % |      141 % |                    578 MB | 62 fps                    |
| `Channel` carrying base64 strings (262 MB, not decoded)                   |  1 MiB |  298 |   143 % |      184 % |                    313 MB | 63 fps                    |
| Events (`emit`) carrying base64 (262 MB), decoded with `atob` in the page |  1 MiB |  221 |    92 % |      119 % |                    670 MB | 63 fps                    |
| Events carrying base64, not decoded                                       |  1 MiB |  482 |   160 % |      134 % |                    375 MB | 60 fps                    |

- **A raw binary `Channel` is the transport.** 360 to 490 MB/s with 64 to 256 KiB chunks is seven to ten times the 40 to 65 MB/s the terminal can parse (below), so it is never the bottleneck, and it arrives as an `ArrayBuffer` that goes straight to `xterm.js` with no base64 and no copy through a string. JSON arrays are a sixth of its speed.
- **Events decoded in the page reach 221 MB/s**, which would also do, but they broadcast to every window listening, carry a third more bytes and use twice the page memory. A `Channel` is per session and ordered. Events are not worth the base64.
- **Chunks of 16 KiB cost three times the throughput**: every message is a task in the page. Batch to 64 KiB.
- **Back pressure is required, not optional.** Without a window the sender ran 2.5 times ahead of the page: the page held 1.36 GB and the app 539 MB for a 500 MB stream, and the page stalled for 302 ms (frames fell to 19 fps). A window of 256 KiB to 4 MiB held memory at 300 to 380 MB and cost nothing in speed.
- **A command that blocks waiting for acknowledgements must not run on the main thread.** The first prototype made the sender a synchronous `#[tauri::command]` (which Tauri runs on the main thread) and deadlocked, because the acknowledgement was a command too. The sender is an `async` command running on a blocking thread, or its own thread.

### Flow control design

The same shape was then run end to end (the PTY of `portable-pty`, then the page, then `xterm.js`): three threads and two counters per session.

1. **Reader thread.** Blocks in `read`, appends to a shared buffer, and **stops reading while the buffer holds 1 MiB or more**. A reader that stops makes the pty's kernel buffer fill and the child block in `write(2)`, so a runaway `yes` or `cat` costs nothing while the page is behind.
2. **Sender thread.** Waits for data; when the buffer is short of 64 KiB, waits up to 4 ms to coalesce; then waits while **unacknowledged bytes are at or above the window**; then sends up to 64 KiB as a raw `Channel` message and adds its size to the unacknowledged count.
3. **Acknowledgement.** The page calls `term.write(bytes, callback)` and, in the callback (the bytes were parsed), sends a small `ack(id, n)` command.

Window sizes from 64 KiB to 16 MiB all gave 41 to 52 MB/s end to end into `xterm.js` with WebGL; 256 KiB to 1 MiB was best. **Use a 1 MiB window and a 1 MiB buffer.** Ctrl-C needs no special case: the key writes `0x03`, which the pty turns into `SIGINT` for the foreground group, and the output the child stops writing is the flow control.

**The 4 ms coalescing wait is a latency cost** (keystroke echo to `xterm.js` parsed: 0 to 1 ms median with no wait, 4 to 5 ms with it, a 1 ms clock). The first prototype slept 4 ms per chunk and so capped the stream at 16 MB/s (64 KiB per 4 ms); sleep only when the buffer is short of a full chunk, and not at all for the first bytes after the shell was quiet.

## The renderer

### Candidates

| Renderer                                                                         | Licence                       | Maintenance                                                                | Notes                                                                                                                                                                  |
| -------------------------------------------------------------------------------- | ----------------------------- | -------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `@xterm/xterm` 6.0.0 with `@xterm/addon-webgl` 0.19.0                            | MIT                           | Updated 2026-08-30; 6.1 betas current; the engine of VS Code               | DOM renderer built in; WebGL addon is a separate file (66.9 KB gzipped, 247 KB raw; `xterm.js` 120 KB gzipped, 489 KB raw)                                             |
| `@xterm/xterm` 5.5.0 with `@xterm/addon-canvas` 0.7.0 and `addon-webgl` 0.18.0   | MIT                           | The canvas addon's last release is 2024-07-14 and **6.0 removed it**       | The only canvas path, on the older major (xterm 67 KB gzipped, webgl 25 KB)                                                                                            |
| hterm (`libapps`; npm `hterm` 2.0.2, BSD)                                        | BSD                           | npm package last published 2023-07-10; the code is in Chromium's `libapps` | **Not measured.** DOM only (no GPU renderer), no Unicode 11+ width tables, no screen reader mode; fewer features than `xterm.js` for no gain in the measurements below |
| A Rust-side parser rendering to a canvas (`vte` 0.15, `alacritty_terminal` 0.26) | Apache-2.0 OR MIT, Apache-2.0 | Maintained                                                                 | The parse speed was measured (below); the renderer, selection, IME, accessibility and the cell-diff protocol would all be Waypoint's to write                          |

### Throughput and rendering: 100 MB through the whole pipeline

`head -c 100000000` of the fixture, PTY then `Channel` (1 MiB window, 64 KiB chunks) then `xterm.js`, 100,036 lines in the buffer afterwards. Then the filled buffer was scrolled by 300 lines a frame for five seconds.

| Renderer           | End to end MB/s | WebKit CPU while writing | Frames while writing  | WebKit CPU while scrolling | Scroll frame rate |
| ------------------ | --------------: | -----------------------: | --------------------- | -------------------------: | ----------------- |
| xterm 6, **WebGL** |            51.3 |                    133 % | 56 fps, gap 42 ms     |                   **10 %** | 60 fps            |
| xterm 6, DOM       |            39.9 |                    134 % | 58 fps, one gap 62 ms |                       33 % | 60 fps            |
| xterm 5, WebGL     |            60.2 |                    138 % | 60 fps, gap 33 ms     |                       13 % | 60 fps            |
| xterm 5, DOM       |            45.9 |                    136 % | 59 fps, gap 41 ms     |                       35 % | 60 fps            |
| xterm 5, canvas    |            28.9 |                    121 % | 55 fps, gap 46 ms     |                       66 % | 60 fps            |

- **WebGL is the renderer.** The frame rate is the display's in every case, so what separates them is the cost of keeping it: three times less CPU than DOM while scrolling, six times less than canvas, and 28 % faster than DOM while writing. Canvas is the slowest of the three and its addon is gone in 6.0.
- **The parser, not the renderer, limits throughput:** 40 to 60 MB/s end to end, and 45.7 MB/s with the data generated in the page (no IPC), against 160 to 220 MB/s from the PTY. A command that prints faster than that is throttled by the flow control, as above. The page kept drawing 55 to 60 fps while 100 MB went through, with no frame gap over 62 ms.
- **Other output:** the 24-bit colour fixture ran at 37 MB/s with WebGL and 32 MB/s with DOM; `find / -xdev` at 24 MB/s (WebGL) and 35 MB/s (DOM), bounded by the find; `yes` at 5.4 MB/s (many one-character lines, so about 1.8 million lines a second).
- **Startup.** In this prototype the first 20 animation frames after the page opened took 3.3 to 4.1 s with either renderer, from an empty page: that is the new window being presented by the compositor, not the terminal (the same stall appeared with the terminal idle, and the stream tests, which began later, did not see it). It is **not** a terminal cost, but a Terminal tab opened in a new window should expect it.

### Memory for 100,000 lines of scrollback

| Case                                                    | WebKit RSS before |  After | Per row                        |
| ------------------------------------------------------- | ----------------: | -----: | ------------------------------ |
| Scrollback 100,000, 100,036 lines of 117 columns, WebGL |            257 MB | 544 MB | about 2.9 KB (23 bytes a cell) |
| Same with the DOM renderer                              |            253 MB | 554 MB | about 3.0 KB                   |
| Same with the 5.5 canvas renderer                       |            260 MB | 535 MB | about 2.8 KB                   |
| 83,761 full rows of 117 columns, WebGL                  |            258 MB | 492 MB | 2.8 KB                         |

The peak during the 100 MB flood was 0.9 to 1.2 GB (parser garbage and in-flight chunks), settling to the figures above. For an 80-column terminal the buffer would be about 1.9 KB a row, 190 MB for 100,000 lines. **The renderer makes no difference to memory; the scrollback does.** Default to a smaller scrollback than 100,000 (for example 10,000 lines, 30 MB at this width) and let a setting raise it.

### Rust-side parsing, on paper and by measurement

The same fixtures were fed to `vte` (a parser that only counts) and to `alacritty_terminal`'s full `Term` (grid, scrollback, damage), single-threaded, in 64 KiB slices:

| Parser                                   | 500 MB text | 200 MB colour escapes |
| ---------------------------------------- | ----------: | --------------------: |
| `vte` 0.15, parser only                  |    988 MB/s |              405 MB/s |
| `alacritty_terminal`, scrollback 0       |    193 MB/s |              254 MB/s |
| `alacritty_terminal`, scrollback 10,000  |    161 MB/s |              205 MB/s |
| `alacritty_terminal`, scrollback 100,000 |    132 MB/s |              192 MB/s |

`alacritty_terminal` parses three to four times faster than `xterm.js` (132 to 254 MB/s against 40 to 60) and holds 100,000 80-column rows in 190 MB (peak RSS 207 MB for a 12 MB input; 15 MB with no scrollback), the same as xterm's. So a Rust-side terminal would not use less memory and would parse faster than the page needs: the data path and the 60 Hz paint, not the parse, set the pace. What it would cost is everything above the parser: a canvas or WebGL renderer, a diff protocol to the page, selection, search in the buffer, IME and the accessibility tree, which `xterm.js` provides. **Not worth it for a first terminal**; the `pty` plugin's event stream (below) keeps the renderer replaceable if the parse cost ever matters.

### What `xterm.js` gives in WebKitGTK

| Check                     | Result                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 24-bit colour             | `\x1b[38;2;12;34;56m` stored as an RGB cell (`isFgRGB()` true, `0x0c2238`) in 5.5 and 6.0. Pixels were not compared                                                                                                                                                                                                                                                                                                           |
| Wide characters and emoji | By default CJK is width 2, but emoji (🎉) are width 1 and a ZWJ family splits into three cells. `addon-unicode11` (5.x) makes 🎉 width 2 but still splits the family; **`addon-unicode-graphemes` (6.x) gives width 2 and one cell for the family and for a combining accent**. Waypoint loads the graphemes addon                                                                                                            |
| Ligatures                 | `@xterm/addon-ligatures` needs the Local Font Access API; `window.queryLocalFonts` is **undefined in WebKitGTK**, so ligatures are **not available** here. The font family is set by the user; no ligature promise in the settings                                                                                                                                                                                            |
| Selection                 | `selectAll()` and `getSelection()` returned the text of the buffer. Pointer drag selection and the clipboard (`navigator.clipboard` exists) were not exercised by hand                                                                                                                                                                                                                                                        |
| Input method              | `xterm.js` uses a hidden `textarea` (`.xterm-helper-textarea`, labelled "Terminal input") and its own composition handling. **Not measured**: no IBus composition was driven (IBus was running on the machine)                                                                                                                                                                                                                |
| Screen reader mode        | With `screenReaderMode: true` the AT-SPI tree of the running window (read with `pyatspi`) is a `list` of `list item`s, one per terminal row, each carrying its text (`'hello from the terminal'`, `'second line of output'`), an `entry` named "Terminal input", and a live-region `section` holding the newest output. WebKitGTK exposes it; **Orca itself was not run**, so what it reads and how it keeps up is unmeasured |
| Screen reader mode, cost  | Writing 100 MB fell from 51 MB/s to **20.5 MB/s** (WebGL) and from 40 to 20.1 MB/s (DOM): 2.5 times slower, with 60 fps kept. Enabled only when a screen reader is in use, or by a setting                                                                                                                                                                                                                                    |
| OSC 0, 2, 7 and 133       | `parser.registerOscHandler` received `7` (`file://host/home/scott/dir%20x`, terminated with `ESC \`), `133` (`A`, `C`, `D;0`), and `onTitleChange` fired for OSC 0 and 2 (5.5 and 6.0)                                                                                                                                                                                                                                        |

## Shells, environment and what the shell says

- **`$SHELL`.** Read `$SHELL` (the login shell in `getpwuid` if it is unset), run it interactively (`-i`, and `-l` where the user's terminal does), fall back to `/bin/sh`. On Windows prefer `pwsh.exe`, then `powershell.exe`, then `%ComSpec%`. Not measured beyond `sh` and `bash -i` here.
- **Environment.** The PTY child gets Waypoint's environment with `TERM=xterm-256color` and `COLORTERM=truecolor` set; the prototype's children saw both. A bundled app must not leak its own variables (`APPDIR`, `LD_LIBRARY_PATH`, the AppImage and Flatpak ones); the plugin takes an explicit allow or deny list. Not tested in a packaged build.
- **Title and folder: what a real shell sends.** The machine's `zsh` (with WezTerm's shell integration loaded, over an SSH session to a local `sshd`) printed `OSC 2 ; scott@host: ~`, `OSC 1`, **`OSC 7 ; file://scott-desktop/home/scott/` terminated with `ESC \`**, and **`OSC 133 ; A`, `C` and `D;0`** around each prompt and command. So `OSC 7` carries a **host name** that must be compared with the machine's before it is trusted as a local folder (a session over SSH reports the remote host: that is how the drawer knows not to `cd` a remote path into a local pane), and it is percent-encoded. `OSC 133` arrives only when the shell has integration; bash, zsh and fish need a snippet, PowerShell needs `$PROFILE` code.
- **Idle at a prompt (for the drawer's `cd`).** Two signals, in order: `OSC 133 ; A` (prompt start) to `OSC 133 ; C` (command start) says the shell is at a prompt, exactly, but only with integration; without it, `tcgetpgrp(master) == shell pid` says no foreground job is running (verified above). That is "no job running", not "at an empty prompt": the user may be half-way through typing a line, so write the `cd` only when idle by that test **and** no input was typed since the last prompt, otherwise show the folder and offer it. On Windows there is no `tcgetpgrp`; use `OSC 133` where PSReadLine or the user's profile emits it and otherwise do not follow.
- **Folder fallback.** Without `OSC 7`, the Linux fallback is `readlink /proc/{shell pid}/cwd` (the shell's own, which is what the title wants). Windows has no such fallback; `OSC 9;9` (the folder, from a PowerShell profile that emits it) is the only candidate and is **unverified**.
- **History per workspace (D75).** `HISTFILE` is read by bash and zsh at start (and fish uses `fish_history`, a session name, not a path); PowerShell's PSReadLine takes its path from `Set-PSReadLineOption -HistorySavePath` in a startup command. So the reach is **bash and zsh by environment variable, fish by `fish_history` name, PowerShell by an injected startup command**, and no other shell. A shell that ignores them keeps its own history, and the Services panel says so. `OSC 133` marks (command start and exit status, with the command text from the shell's own history) are the route for a Waypoint-owned history later; not built or measured here.

## SSH terminals

A shell over the SFTP provider's SSH connection is **feasible with no new dependency**: the provider's `Session` already holds a `russh::client::Handle`, and a shell is `handle.channel_open_session()`, `channel.request_pty("xterm-256color", cols, rows, 0, 0, &[])`, `channel.request_shell(true)` on the same authenticated handle (host key checking, jump hosts and the agent are shared). Measured with `russh` 0.63.3 (the provider's pins) against a throwaway user-level OpenSSH `sshd` on loopback, with an 8 MiB window:

| Measurement                                  | Result                                                       |
| -------------------------------------------- | ------------------------------------------------------------ |
| Connect and public-key login                 | 114 ms                                                       |
| Shell request to first output                | 1.8 ms; the shell reported `24 80` and `TERM=xterm-256color` |
| `window_change(132, 40)`                     | The remote `stty size` printed `40 132`                      |
| Keystroke echo, remote pty over loopback     | 30 µs median, 42 µs p99                                      |
| `cat` of the 500 MB file through the channel | 156 MB/s (the local PTY gave 184); `find / -xdev`, 43 MB/s   |
| OSC 2, 7 and 133 through the channel         | Delivered unchanged                                          |

Over a real network the window and latency rule, as in the SFTP results ([`milestone-6-spikes.md`](milestone-6-spikes.md): the default window caps a transfer at about 9 MiB/s at 100 ms). The Waypoint-side change is an `open_shell` operation on `waypoint-provider-sftp` that returns a handle with `write`, `resize` and an output stream; `src-tauri` adapts it to the same `PtyEvent` stream. Not measured: a real network, password and keyboard-interactive logins into a shell, keepalives on an idle terminal, and `set_env` against servers that do not `AcceptEnv`.

## Windows

**Nothing was run on Windows.** From documentation and `cargo xwin check --target x86_64-pc-windows-msvc`:

- `portable-pty` 0.9.0 and `conpty` 0.7.0 both compile for `x86_64-pc-windows-msvc` (the check also passed a spawn, a read, a write and a resize with `portable-pty`). `pty-process` and the `nix` and `rustix` routes are Unix only, so they belong behind `cfg(unix)`; `portable-pty` is the single dependency for both.
- ConPTY needs Windows 10 1809 or later; Windows 11 is the target, so it is always there. `portable-pty` uses the system `kernel32` ConPTY (and `conpty.dll` with `OpenConsole.exe` beside the executable when present). The inbox ConPTY re-renders the screen it sends rather than passing bytes through (known effects: it may rewrite or repaint output, and reflow on resize); the sideloaded one passes through. Whether to ship `OpenConsole.exe` is an open question for #325, to be settled by running `htop`-style and full-screen programs on the Windows 11 VM.
- `xterm.js` in WebView2 (Chromium) has the WebGL and Local Font Access APIs, so ligatures may be available there; **unmeasured**. Narrator and screen reader mode are unmeasured.
- The window-size and signal story differs: no `SIGWINCH` (ConPTY's resize call), Ctrl-C is written as `0x03` and ConPTY raises the console event, and there is no process group to signal (use the job object `portable-pty`'s `kill` uses, or `taskkill /T`).

## Flatpak

**Unmeasured**: `flatpak-spawn` is not installed here and nothing was run in a sandbox. From documentation: inside a sandbox a PTY child is sandboxed too, so the host shell needs `flatpak-spawn --host` and the `--talk-name=org.freedesktop.Flatpak` permission (which Flathub reviews restrictively). Through `flatpak-spawn` the PTY slave is not the host process's controlling terminal, so job control and Ctrl-Z can fail in the shell, and a resize reaches `flatpak-spawn`, not the shell. Until that is tested, the `pty` plugin reports in a Flatpak that the host shell is **unavailable** unless the permission is present (`flatpak-sandbox` reason), and offers the sandbox's own shell, in line with how the Trash and volumes plugins treat the sandbox.

## Recommendation

1. **Renderer: `xterm.js` 6 with the WebGL addon, the DOM renderer as the fallback, and the Unicode graphemes addon (A130).** WebGL used 10 % of a core to scroll 100,000 lines at 60 fps, against 33 % for DOM and 66 % for the old canvas addon, and wrote at 51 MB/s. Fall back to DOM when `webgl2` is unavailable or the context is lost. No canvas addon (gone in 6.0), no hterm, no Rust-side renderer.
2. **PTY: `portable-pty` behind a trait in `tauri-plugin-pty`, with `cfg(unix)` for a `pty-process`-style Tokio path only if needed (A131).** The libraries do not differ in speed; `portable-pty` is the one with ConPTY, and the plugin owns reading, coalescing, flow control and shutdown, so a replacement stays cheap. Reader on its own thread, not on the Tokio pool.
3. **Data path: a raw binary `Channel` of 64 KiB batches with a 1 MiB acknowledgement window and a 1 MiB reader buffer, stopping the PTY reader when the page lags (A131).** Coalesce for at most 4 ms but never delay the first bytes after quiet; acknowledge from `term.write`'s callback; send bytes, not strings.
4. **SSH terminals: an `open_shell` on the SFTP provider's `russh` handle behind the same event stream (A132).** Feasible with no new dependency; measured on loopback only.
5. **Budgets** (this machine, WebKitGTK): echo to parsed in the page under 5 ms median (1 ms without coalescing), 60 fps with 100 MB flowing, scrolling 100,000 lines under 15 % of a core with WebGL, 40 MB/s or more end to end, WebKit memory under 3 KB a row (use a 10,000-line default), the window and buffer at 1 MiB, and screen reader mode only on demand (it costs 2.5 times the throughput).

### On the contract proposals (A126, D216)

- **`PtyEvent` `Output`, `Title`, `Cwd`, `Exit`: confirmed**, with three amendments. `Output(bytes)` is a raw binary `Channel` message (not JSON, not a string) and the page's `ack(id, n)` command is part of the contract, because without it the stream outruns the page 2.5 times and holds 1.4 GB. `Cwd` carries the host from `OSC 7` with the path, so the consumer can tell a remote folder from a local one. Add **`Prompt(state)`** (from `OSC 133` marks, `Idle` and `Busy`) and a `get_status` field saying whether a foreground job can be seen (`tcgetpgrp`, Linux), so idleness is not guessed in the page.
- **The terminal tab as a `terminal:?cwd=` virtual location: confirmed.** Nothing measured argues against it: the terminal's state (the PTY, the buffer) is runtime state outside the session store and a restored tab starts a fresh shell, as the contract says.
- **Settled by this spike:** the renderer (WebGL with DOM fallback, A130), the PTY library (`portable-pty`, A131), the data path and budgets above, how idleness is known (`OSC 133` where the shell sends it, otherwise "no foreground job" through `tcgetpgrp` plus no typing since the prompt; not at all on Windows without marks), how far history reaches (bash and zsh by `HISTFILE`, fish by `fish_history`, PowerShell by a startup command), and SSH terminals' feasibility (A132).
- **Still open:** Orca and Narrator reading, IME, ConPTY behaviour at run time (and `OpenConsole.exe`), the Flatpak host shell, KDE, fractional scaling, packaged-build environment hygiene, and SSH over a real network.

## Findings that surprised

1. **The output of a PTY arrives in tiny reads** (14 bytes on average for `yes`, about 500 bytes for `cat` of a file), so a reader that forwards each read makes a million messages for 500 MB. Coalescing fixed it at no cost in speed.
2. **`yes` tops out at 14 MB/s through any PTY**, from the tty's `ONLCR` processing of each line, whichever library or reader.
3. **The renderer did not change the parsing speed**, only the CPU spent drawing; all five configurations moved 100 MB in 1.5 to 3.5 s, bound by `xterm.js`'s parser.
4. **`@xterm/addon-canvas` is dead**: last published July 2024, and removed in 6.0.
5. **Screen reader mode costs 2.5 times the throughput**, and WebKitGTK does expose it over AT-SPI as a list of rows with text.
6. **A synchronous Tauri command that waits for the page deadlocks it**: synchronous commands run on the main thread.
7. **A new window took 3 to 4 s to start drawing frames** in this setup, with or without a terminal.
8. **A real zsh sends `OSC 7` with the host name and `OSC 133`**, even over SSH; the host must be checked.

## Not verified

- **Windows at run time** (ConPTY, `OpenConsole.exe`, WebView2 rendering, Narrator, shell integration for PowerShell): only `cargo xwin check` of `portable-pty` and `conpty`.
- **Orca, Narrator and any other screen reader**: only the AT-SPI tree of a running window. **IME**, hardware keyboards, key encodings (kitty protocol, `modifyOtherKeys`), mouse reporting and bracketed paste were not exercised.
- **`htop`, `vim` and full-screen programs**, and resizing under load. A resize was verified only for the `SIGWINCH` and the size the child saw.
- **Flatpak** (`flatpak-spawn --host`), **KDE**, **X11 sessions**, **fractional scaling**, and a second monitor; the numbers are one 60 Hz Wayland display at device pixel ratio 2.
- **Packaged-build environment** (AppImage and Flatpak variables) and `$SHELL` handling for fish, nushell and PowerShell.
- **SSH shells over a real network**, with password logins, a jump host, or a server that closes idle connections; only loopback `sshd`.
- **A pointer drag selection, copy and paste**, links, and search in the buffer.
- **hterm** (paper only) and the **Rust-side renderer** (the parse speed only).

## Method

The harnesses were small enough to describe:

- **PTY harness.** A Rust program that opens each of the four PTY flavours, spawns `cat`, `sh -c 'yes | head -c N'`, `find / -xdev` or `timeout 5 yes`, reads into a 64 KiB buffer until end of file, and counts bytes, reads and chunks ending inside a UTF-8 sequence. Latency: write one byte to `cat` 3,000 times and time the echo (200 warm-up iterations dropped). Resize and signals: `sh` with `trap WINCH`, `0x03`, and `kill(-pgid, SIGHUP)` on a pipeline.
- **Parser harness.** The 500 MB file read into memory, then `vte::Parser::advance` or `alacritty_terminal`'s `Processor::advance` over 64 KiB slices on an 80 by 24 `Term`; peak RSS from `/proc/{pid}/status` `VmHWM`.
- **Webview harness.** A Tauri 2 release build (`custom-protocol`) with a plain page: `xterm.js` 5.5.0 and 6.0.0 and their WebGL (and, for 5.5, canvas) addons copied from npm, a command each for the PTY (`portable-pty`, the reader, sender and acknowledgement design above), a synthetic `Channel` or `emit` stream, and a command that returns CPU ticks and RSS for the app and its WebKit children from `/proc`. The page ran each case in a fresh process, waited for 20 animation frames before measuring (the window needs 3 to 4 s to start drawing), counted `requestAnimationFrame` calls and gaps over 50 ms, and wrote its result to a file.
- **SSH harness.** A `russh` 0.63.3 client with a public key against `sshd -D -f` as the current user (`UsePAM no`, no root, port 2299).
- **Windows.** `cargo xwin check --target x86_64-pc-windows-msvc` of a crate using `portable-pty` and `conpty`.
- **Accessibility.** The running page's tree read through `pyatspi` (`Registry.getDesktop`) while it was held open.

Delete the target directory, the 500 MB fixture and the `sshd` keys when done.
