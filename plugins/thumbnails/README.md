# @liminal-hq/plugin-thumbnails

Makes thumbnails for files, with a queue that keeps up with a scroll. On Linux it works on the freedesktop.org thumbnail cache itself, so a thumbnail made here is reused by Nemo, Nautilus and Dolphin, and the other way round. On Windows it asks the shell for the thumbnails Explorer shows.

The plugin knows nothing about the app around it. It reports what works through `getStatus()`, so an interface can hide what does not.

## Installation

### Rust

```toml
[dependencies]
tauri-plugin-thumbnails = "0.1"

# Alternatively with Git:
tauri-plugin-thumbnails = { git = "https://github.com/liminal-hq/tauri-plugins-workspace", branch = "main" }
```

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_thumbnails::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

`init_with_config(Config { … })` sets the number of workers, the in-memory cache size, the largest file the built-in generator decodes and the timeout of an external thumbnailer. Grant the windows that use it `thumbnails:default` in a capability file, or the individual `thumbnails:allow-request`, `thumbnails:allow-cancel`, `thumbnails:allow-prioritise` and `thumbnails:allow-get-status`.

### JavaScript

```bash
pnpm add @liminal-hq/plugin-thumbnails
```

## Usage

```typescript
import { cancel, getStatus, hasFeature, prioritise, request } from '@liminal-hq/plugin-thumbnails';

const status = await getStatus();
if (status.available) {
	const ticket = await request(
		[{ key: 'row-12', path: '/home/me/photos/a.jpg', size: 'large', mtimeMs: 1_700_000_000_000 }],
		(event) => {
			if (event.kind === 'ready') img.src = event.url; // thumb://localhost/large/{md5}.png?v=…
			if (event.kind === 'failed') console.warn(event.key, event.reason);
			if (event.kind === 'skipped') console.debug(event.key, event.why); // 'remote', 'tooLarge', 'cloud', …
		},
	);
	await prioritise(ticket, ['row-12']); // the rows now in view, first one first
	await cancel(ticket); // the user scrolled away
}
```

### Rust

```rust
use tauri_plugin_thumbnails::ThumbnailsExt;

let ticket = app.thumbnails().request(items, std::sync::Arc::new(|event| { /* … */ }));
app.thumbnails().cancel(ticket);
app.thumbnails().set_max_file_bytes(20 * 1024 * 1024);

// A file the plugin cannot open (one on a server, read by the app itself): from its bytes.
let event = app
    .thumbnails()
    .cached_from_bytes("row-7", "sftp://me@nas.lan/a.jpg", mtime_ms, size)
    .unwrap_or_else(|| app.thumbnails().from_bytes("row-7", "sftp://me@nas.lan/a.jpg", mtime_ms, size, &bytes));
```

`from_bytes(key, uri, mtime_ms, size, bytes)` makes a thumbnail from bytes the caller read (a whole image, or an image embedded in a file) with the built-in decoders, and `cached_from_bytes` answers from what it made before for the same `uri` and time, so a caller asks first and reads nothing when it is known. These thumbnails are kept in a memory cache of their own (`Config::bytes_cache_bytes`, 32 MB), never in the shared cache folder, and are served by the same `thumb://` scheme. Both are Rust only: bytes do not cross IPC.

## API

| Function                                                  | What it does                                                                                                                                                                                                                                                                                            |
| --------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `request(items, onEvent)`                                 | Queues the items (`{ key, path, size, mtimeMs }`) as one request and returns its ticket. Results arrive through `onEvent` as they are made: `ready` (with a `url`), `failed` (with a `reason`) or `skipped` (with a `why`). The newest request is served first, and a `key` requested twice is one job. |
| `cancel(ticket)`                                          | Withdraws a request: nothing more is sent to it, work nobody else wants is dropped and a running external thumbnailer is killed. Returns false when the ticket was no longer known.                                                                                                                     |
| `prioritise(ticket, keys)`                                | Moves the request's pending items for `keys` to the front, in the order given.                                                                                                                                                                                                                          |
| `getStatus()`                                             | `{ available, reason, flavour, features }`; see below.                                                                                                                                                                                                                                                  |
| `hasFeature(status, name)`, `featureReason(status, name)` | Read the status.                                                                                                                                                                                                                                                                                        |

Sizes are the standard's: `normal` (128 px), `large` (256), `x-large` (512) and `xx-large` (1024). A thumbnail fits within a square of that size and is never scaled up. `mtimeMs` is the file's modified time; a cached thumbnail made from another time is stale and is made again.

A `skipped` event is not an error: `remote` (not a local file), `tooLarge` (over the decode limit), `cloud` (a cloud placeholder with no thumbnail already cached), `noGenerator` (nothing here handles that kind of file) or `unsupported` (this system has no thumbnails). A `failed` event is remembered, so a broken file is not decoded again until it changes.

### The `thumb://` scheme

A `ready` event's `url` is `thumb://localhost/{size}/{md5}.png?v={mtime}` (on Windows `http://thumb.localhost/…`). The scheme serves bytes by **cache key only**: exactly a size folder name and 32 lowercase hexadecimal digits, and only if that entry is in the cache. It never takes a path, so it cannot read any other file, and `..`, separators, encoded separators, other names and symbolic links all answer 404. The `v` parameter is the file's modified time, which makes a newer thumbnail a different address.

## Features and status

`getStatus()` returns `{ available, reason, flavour, features }`, where each feature is `{ name, available, reason, count }` and `reason` is `{ kind, message }`. Decide behaviour from the features, never from the platform.

| Feature    | Meaning                                                                                                           |
| ---------- | ----------------------------------------------------------------------------------------------------------------- |
| `cache`    | The thumbnail cache folder can be used.                                                                           |
| `builtin`  | The built-in image generator works (Linux).                                                                       |
| `external` | At least one `*.thumbnailer` is installed; `count` says how many (Linux). Reason kind `noThumbnailers` otherwise. |
| `shell`    | The Windows shell makes thumbnails (Windows).                                                                     |

Reason kinds: `unsupported`, `otherPlatform`, `noCacheDirectory` and `noThumbnailers`. `flavour` is `freedesktop`, `windows` or `unsupported`.

## Platform notes

### Linux (`freedesktop`)

- **The cache** is the [Thumbnail Managing Standard](https://specifications.freedesktop.org/thumbnail-spec/latest/): `$XDG_CACHE_HOME/thumbnails/{normal,large,x-large,xx-large}/{md5-of-uri}.png`, where the URI is the file's `file://` URI percent-encoded the way GLib does it (so the names match the ones other file managers write, even for names that are not UTF-8). Each PNG carries `Thumb::URI`, `Thumb::MTime` and `Thumb::Size` text chunks, written before the image data and read without decoding it; a thumbnail whose time differs from the file's is stale. Files are written atomically with mode 0600 in folders of mode 0700. Failures are recorded under `fail/{app}-{version}/`.
- **The built-in generator** decodes PNG, JPEG (at a reduced scale, so a 12 megapixel photograph is never decoded in full), GIF, WebP, BMP, TIFF and ICO, found by content and not by name, applies the Exif orientation and resizes with `fast_image_resize`. A file larger than `max_file_bytes` (50 MiB) is never read.
- **External thumbnailers** are the `*.thumbnailer` files in `thumbnailers/` of `$XDG_DATA_HOME` and `$XDG_DATA_DIRS`: `TryExec` must exist, `Exec` is expanded (`%i` input path, `%u` input URI, `%o` output path, `%s` size, `%%`) and run with a 10 second timeout, `nice` 10, in its own process group, which is killed on timeout and on cancel. The file's MIME type comes from the shared MIME database's name globs (`mime/globs2`, merged from every data directory, so `~/.local/share/mime` adds the user's own types to the system's rather than replacing them) and never from reading the file. Local files only.
- **The queue** serves the newest request first, with one job per key, `min(4, cores / 2)` workers, and a 64 MB in-memory cache of encoded thumbnails in front of the files.

### Windows (`windows`)

Each worker thread enters a single-threaded apartment and asks `IShellItemImageFactory::GetImage` with `SIIGBF_THUMBNAILONLY`; the bitmap is converted to a PNG and cached under the app's local cache folder in the same layout. A cloud placeholder (`FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS`, `RECALL_ON_OPEN` or offline) is only asked for a thumbnail the shell already has (`SIIGBF_INCACHEONLY`), so a thumbnail never downloads a file; with none cached the request is `skipped` as `cloud`. Network shares are `remote`. The path is cross-checked with `cargo xwin` and exercised through the plugin's ignored `live_shell_thumbnail` test.

### Elsewhere

Every feature is reported unavailable with the reason `unsupported`, and each request is `skipped` as `unsupported`.

## Tests

`cargo nextest run -p tauri-plugin-thumbnails` runs everything headless in temporary directories; the real home and cache are never touched. `cargo test --release -p tauri-plugin-thumbnails --test throughput -- --ignored --nocapture --test-threads=1` measures thumbnails per second, peak memory and cache-hit latency (`THUMB_BENCH_COUNT` sets the number of images, 2000 by default).
