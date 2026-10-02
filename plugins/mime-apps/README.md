# @liminal-hq/plugin-mime-apps

The type of a file and the applications that open it: the type, a phrase and an icon name for a path or URI, the default application and the other applications for it, Open With, the system's own chooser where it has one, the default handler, and application icons. On Linux it uses gio's `AppInfo`, which follows `mimeapps.list` and the desktop files; inside a Flatpak sandbox it uses the OpenURI portal. On Windows it uses the shell's association handlers.

The plugin knows nothing about the app around it. It reports what works through `getStatus()`, so an interface can hide what does not.

## Installation

### Rust

```toml
[dependencies]
tauri-plugin-mime-apps = "0.1"

# Alternatively with Git:
tauri-plugin-mime-apps = { git = "https://github.com/liminal-hq/tauri-plugins-workspace", branch = "main" }
```

```rust
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_mime_apps::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Grant the windows that use it `mime-apps:default` in a capability file, or the individual `mime-apps:allow-get-status`, `mime-apps:allow-type-info`, `mime-apps:allow-handlers`, `mime-apps:allow-open-with`, `mime-apps:allow-open-default`, `mime-apps:allow-choose`, `mime-apps:allow-set-default` and `mime-apps:allow-open-default-apps-settings`. The `appicon://` scheme needs `appicon:` (and `http://appicon.localhost` on Windows) in the `img-src` of the page's content security policy.

### JavaScript

```bash
pnpm add @liminal-hq/plugin-mime-apps
```

## Usage

### JavaScript

```typescript
import {
	appIconUrl,
	choose,
	getStatus,
	handlers,
	hasFeature,
	isMimeAppsError,
	openDefault,
	openWith,
	setDefault,
	typeInfo,
} from '@liminal-hq/plugin-mime-apps';

const status = await getStatus();

const info = await typeInfo('file:///home/me/cat.png'); // { mime: 'image/png', description: 'PNG image', icon: 'image-png' }
const kinds = await typeInfo('/home/me/Pictures/'); // inode/directory

if (hasFeature(status, 'handlers')) {
	const { default: preferred, recommended, others } = await handlers(['/home/me/cat.png']);
	// preferred: { id, name, icon, execHint } | null; show appIconUrl(app.id, 24) as an <img>
	await openWith(['/home/me/cat.png'], recommended[0].id);
}

try {
	await openDefault(['/home/me/cat.png', '/home/me/notes.txt']);
} catch (error) {
	if (isMimeAppsError(error) && error.kind === 'noHandler') {
		// error.mime has no application: offer the list instead
	}
}

if (hasFeature(status, 'chooser')) {
	await choose(['/home/me/cat.png'], 'main-1'); // rejects with { kind: 'cancelled' } when dismissed
}
if (hasFeature(status, 'setDefault')) {
	await setDefault('image/png', 'org.gnome.eog.desktop');
}
```

### Rust

The same operations are on the handle:

```rust
use tauri_plugin_mime_apps::MimeAppsExt;

let handlers = app.mime_apps().handlers(&["/home/me/cat.png".into()]).await?;
app.mime_apps().open_default(&["/home/me/cat.png".into()]).await?;
```

## API

| Function                                                      | What it does                                                                                                                                                                                                                                                |
| ------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `getStatus()`                                                 | `{ available, reason, message, flavour, features, associationFiles }`; see below.                                                                                                                                                                           |
| `typeInfo(uri, sniff?)`                                       | `{ mime, description, icon }` of a path or URI. By name; with `sniff` the start of a local file is read too. A trailing slash or a local directory is `inode/directory`.                                                                                    |
| `handlers(uris)`                                              | `{ mime, mixed, default, recommended, others }`. The default is first and not repeated; hidden applications (`NoDisplay`) are left out and duplicates removed. For several types only the applications that open all of them are listed and `mixed` is set. |
| `openWith(uris, appId)`                                       | Opens the locations in that application, in one start.                                                                                                                                                                                                      |
| `openDefault(uris)`                                           | Opens each location in its default application, one start per application. Nothing starts when one location has no handler (`noHandler`).                                                                                                                   |
| `choose(uris, parentLabel?)`                                  | The system's chooser, as a child of the window with that label: the Open With dialog on Windows, the portal's chooser in a Flatpak. `unsupported` where there is none: draw a list from `handlers`.                                                         |
| `setDefault(mime, appId)`                                     | Makes an application the default for a type (writes the user's `mimeapps.list` through gio).                                                                                                                                                                |
| `openDefaultAppsSettings()`                                   | Opens Windows Settings at Default apps. `unsupported` elsewhere.                                                                                                                                                                                            |
| `appIconUrl(appId, size?)`                                    | The `appicon://` address of an application's icon, a PNG of 16 to 256 pixels.                                                                                                                                                                               |
| `hasFeature(status, name)`, `featureReason`, `featureMessage` | Read the status.                                                                                                                                                                                                                                            |

A location is a path (`/home/me/a.txt`, `C:\Users\me\a.txt`) or a URI (`file:///…`, `smb://…`, `sftp://…`); a relative path is rejected. An `App` is `{ id, name, icon, execHint }`: `id` is the desktop-file id on Linux (`org.gnome.eog.desktop`; `.desktop` may be left off when passing it back) and the shell's handler name on Windows, and is otherwise opaque. `icon` is an icon-theme name for a tooltip or a fallback; the picture comes from `appIconUrl`. `execHint` is the command line, for a tooltip, and is never run by the plugin.

Errors are `MimeAppsError` objects with a `kind`: `appNotFound`, `noHandler` (with `mime`), `invalidUri` (with `uri`), `empty`, `cancelled`, `unsupported` and `failed` (with `message`).

### Icons by app id only

`appicon://localhost/{app id}?size=32` serves an application's icon as a PNG, resolved through the icon theme and kept in memory per size. The scheme takes an application id and nothing else: it is a 404 for an id the system does not know, for anything that is not a single path segment, for `.`, `..` and for any encoded slash, so it cannot be used to read a file. Only `GET` and `HEAD` are answered.

## Features and status

`getStatus()` returns `{ available, reason, message, flavour, features, associationFiles }`, where each feature is `{ name, available, reason, message }`. Decide behaviour from the features, never from the platform.

| Feature       | Meaning                                                                   |
| ------------- | ------------------------------------------------------------------------- |
| `typeInfo`    | The type of a location can be read.                                       |
| `handlers`    | The applications for a type can be listed.                                |
| `openWith`    | A location can be opened in an application chosen by id.                  |
| `openDefault` | A location can be opened in its default application.                      |
| `setDefault`  | The default application for a type can be changed.                        |
| `chooser`     | The system has a chooser to call (otherwise draw a list from `handlers`). |
| `appIcons`    | `appicon://` serves icons.                                                |

`flavour` is `gio`, `portal`, `windows` or `unsupported`. `associationFiles` lists the `mimeapps.list` files that exist, from the one that wins to the one that loses (Linux; empty elsewhere), which says where a default comes from. A `reason` is a code to branch on and `message` a sentence for people:

| Reason                 | Meaning                                                                  |
| ---------------------- | ------------------------------------------------------------------------ |
| `flatpak-sandbox`      | A Flatpak sandbox sees neither the host's applications nor its defaults. |
| `no-system-chooser`    | The system has no chooser to call; the front end draws its own list.     |
| `managed-by-system`    | The system does not allow a silent change of the default (Windows).      |
| `no-display`           | There is no display, so there is no icon theme to draw from.             |
| `not-implemented`      | The plugin does not do this on this operating system yet.                |
| `unsupported-platform` | This operating system has no support in the plugin.                      |

## Platform notes

### Linux (`gio`)

- Every gio call is behind a small trait, `AppDirectory`, so the logic that builds the lists and starts applications is tested against a fake. Applications are copied into plain data inside each call; no gio object outlives it.
- Types come from gio's content-type guess by file name (the shared MIME database's globs). A local file's first 4 KiB is read only when `sniff` is asked for.
- The handlers are `AppInfo::default_for_type`, `recommended_for_type`, then `all_for_type`, `fallback_for_type` and every other installed application (what an "Other application…" list shows). A default whose desktop file is hidden is still shown as the default.
- When gio cannot name a default, or lists nothing, the plugin reads the `mimeapps.list` files itself, in the order the freedesktop.org specification gives (`$XDG_CONFIG_HOME`, `$XDG_CONFIG_DIRS`, `$XDG_DATA_HOME/applications`, `$XDG_DATA_DIRS/applications`, each with the `$desktop-mimeapps.list` first) and the rules for `[Added Associations]`, `[Removed Associations]` and `[Default Applications]`. It only ever reads them; `setDefault` is gio's.
- An application starts through the main thread with the toolkit's launch context, so the display's activation token is passed and the window is raised on Wayland.
- Inside a Flatpak sandbox (`/.flatpak-info`) the OpenURI portal opens each location (`ask` for `choose`). The portal cannot list applications, open in a chosen one or change a default, so `handlers`, `openWith`, `setDefault` and `appIcons` are reported unavailable with `flatpak-sandbox`.
- Outside a sandbox there is no system chooser to call: `choose` is unavailable (`no-system-chooser`) and the front end draws its own list from `handlers`.

### Windows (`windows`)

- Handlers belong to an extension. `SHAssocEnumHandlers` lists them (the recommended ones with `ASSOC_FILTER_RECOMMENDED`), `AssocQueryStringW` reads the default and the registered content type, and `IAssocHandler::Invoke` opens files in a chosen handler. The type reported is the extension's content type, or the extension itself (`.png`) when it has none.
- `choose` is `SHOpenWithDialog`, as a child of the window whose label is passed, for one local file; for several files it is `unsupported` and the front end draws a list.
- `openDefault` is the shell's `open` verb.
- Windows does not let an application change the default silently: `setDefault` is unavailable (`managed-by-system`) and `openDefaultAppsSettings()` opens `ms-settings:defaultapps`.
- Every shell call runs on a thread of its own with COM initialised as a single-threaded apartment.
- Application icons are not served yet (`appIcons` is `not-implemented`).
- Cross-checked with `cargo xwin` and exercised in a Windows 11 virtual machine through the ignored `live_handlers_windows` test.

### Other systems

Every feature is reported unavailable with `unsupported-platform`.

## Testing

`cargo nextest run -p tauri-plugin-mime-apps` runs headless: the `mimeapps.list` parser and precedence, the mapping of a fake directory to the handler lists, locations to names and types, the `appicon://` guard, the status in a simulated Flatpak and the commands through Tauri's mock runtime over a fake backend. Nothing starts an application, opens a dialog or changes a default, and the owner's `mimeapps.list` is never read.

`cargo test -p tauri-plugin-mime-apps --test live -- --ignored --nocapture` prints this machine's answer for `image/png`, `text/plain` and `inode/directory` (Linux `live_handlers`; Windows `live_handlers_windows`). It only asks.
