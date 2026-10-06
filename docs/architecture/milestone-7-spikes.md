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
