# @liminal-hq/plugin-elevate

Starts a helper program with administrator rights through the system's own prompt, and reports whether it can. On Linux the prompt is polkit's, through `pkexec`. Windows (a UAC prompt) is not implemented yet and reports itself unavailable.

The plugin only starts the helper and hands back the pipes to talk to it. What the helper does, and the protocol spoken over the pipes, belong to the app. The plugin knows nothing about the app around it: the app tells it which helper to start, where its polkit policy is, and the one line the helper writes when it is running.

## Installation

### Rust

```toml
[dependencies]
tauri-plugin-elevate = "0.1"

# Alternatively with Git:
tauri-plugin-elevate = { git = "https://github.com/liminal-hq/tauri-plugins-workspace", branch = "main" }
```

```rust
use tauri_plugin_elevate::{Config, ElevateExt};

fn main() {
    let config = Config::new(
        "/usr/libexec/my-app/my-helper",
        "/usr/share/polkit-1/actions/com.example.my-app.admin.policy",
        "my-helper ready",
    );
    tauri::Builder::default()
        .plugin(tauri_plugin_elevate::init_with_config(config))
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Grant the windows that show the status `elevate:default` (or `elevate:allow-get-status`) in a capability file.

### JavaScript

```bash
pnpm add @liminal-hq/plugin-elevate
```

## Usage

### JavaScript

The page can only read the status:

```typescript
import { featureReason, getStatus, hasFeature } from '@liminal-hq/plugin-elevate';

const status = await getStatus();
if (!hasFeature(status, 'elevate')) {
	// Hide the entry points; show `featureReason(status, 'elevate')` where the person looks for why.
}
```

### Rust

Starting the helper is Rust code only:

```rust
use tauri_plugin_elevate::{ElevateExt, LaunchError};

// On a thread of its own: it blocks until the person has answered the prompt.
let result = app.elevate().launch(&|| user_cancelled());
match result {
    Ok(stream) => {
        // `stream.reader` is the helper's standard output, `stream.writer` its standard input.
        // Dropping the stream closes both, which ends the helper.
    }
    Err(LaunchError::Dismissed) => { /* the dialog was closed */ }
    Err(LaunchError::NotAuthorised) => { /* wrong password, or not an administrator */ }
    Err(LaunchError::Cancelled) => { /* `user_cancelled()` returned true */ }
    Err(other) => { /* `other` is a message for the person; it never names a path */ }
}
```

## API

| Item                                                      | What it does                                                                                                                                      |
| --------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| `getStatus()`                                             | `{ available, reason, flavour, features }`; see below.                                                                                            |
| `hasFeature(status, name)`, `featureReason(status, name)` | Read the status.                                                                                                                                  |
| `Config { helper, policy, ready_line }` (Rust)            | The helper's absolute path, the installed polkit policy's absolute path, and the ready line. There are no defaults.                               |
| `Elevator::status()` (Rust)                               | The same status, read again on every call.                                                                                                        |
| `Elevator::launch(&cancelled)` (Rust)                     | Starts the helper and blocks until it is running. Returns an `ElevatedStream { reader, writer }` that owns the process.                           |
| `launch::launch_with`, `Spawner`, `Spawned` (Rust)        | The launching loop over a trait for the child process, so it can be tested with a script. `polkit::check` and `Probe` do the same for the status. |

## Features and status

`getStatus()` returns `{ available, reason, flavour, features }`, where each feature is `{ name, available, reason }`. There is one feature, `elevate`. `flavour` is `polkit` or `unsupported`.

On Linux, `elevate` is available only when all of these hold; the first that fails gives the reason:

| Check                                                                                                 | Reason                                                                             |
| ----------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| Not in a Flatpak (`/.flatpak-info` absent, `container` is not `flatpak`)                              | Not available in Flatpak                                                           |
| Not in an AppImage (neither `APPIMAGE` nor `APPDIR` set)                                              | Elevation is not available in the AppImage; install the .deb or .rpm               |
| `pkexec` is an executable file at `/usr/bin/pkexec`, or else found on `PATH`                          | pkexec (polkit) was not found                                                      |
| The helper exists and is a regular file at an absolute path                                           | The administrator helper is not installed                                          |
| The helper is owned by root                                                                           | The administrator helper is not owned by root                                      |
| The helper is not writable by its group or by others                                                  | The helper is writable by other users                                              |
| Every folder above the helper, up to `/`, is owned by root and not writable by its group or by others | A folder that holds the helper is not owned by root, or is writable by other users |
| The polkit policy file exists                                                                         | The polkit policy is not installed                                                 |

On every other platform the reason is "Not implemented on this platform yet".

## The launch contract

`launch` runs the status check first and refuses with `Unavailable { reason }` when it fails. It then starts `pkexec <helper>` with the helper's standard input and output piped as the protocol stream, and its standard error piped for the ready line.

Polkit's dialog can wait for the person for as long as they like, so `launch` blocks until the helper is really running. The helper's first act must be to write one line, exactly the configured `ready_line`, to its standard error. That line is the wire contract between the helper and the app; the plugin compares whole lines and ignores everything else `pkexec` writes. Only when it arrives does `launch` return, so a timeout the caller starts for its own protocol begins after the prompt, not during it. `launch` polls `cancelled` about every 50 ms.

| Outcome                  | When                                                                                                   |
| ------------------------ | ------------------------------------------------------------------------------------------------------ |
| `Ok(ElevatedStream)`     | The ready line arrived.                                                                                |
| `Cancelled`              | `cancelled()` returned true first. The process is signalled, its pipes are closed and it is released.  |
| `Dismissed`              | `pkexec` exited with 126: the person closed the authentication dialog.                                 |
| `NotAuthorised`          | `pkexec` exited with 127: authentication failed, the person is not allowed, or `pkexec` itself failed. |
| `Failed { code }`        | The process ended some other way before the ready line, or closed its error stream and lingered.       |
| `Unavailable { reason }` | The status check failed.                                                                               |
| `Io { kind }`            | The process could not be started or watched.                                                           |

The exit codes are those documented for `pkexec`: 127 for "not authorised, authentication could not be obtained, or an error", and 126 for "the user dismissed the authentication dialog". No error message names a path.

Dropping an `ElevatedStream` closes the helper's input and output first, which ends a helper that has become root's (a process the person cannot signal once `pkexec` has handed over to it), then signals the process and releases it on a thread, so dropping never blocks.

## Security notes

- **No page-callable launch.** The only command is `get_status`. A command that started an elevated process from the webview would let script in the page cause a prompt and an elevated session; launching exists only as a Rust API for the app to compose behind its own explicit actions.
- **The helper path is protected.** The helper is offered only when it and every folder above it belong to root and cannot be changed by anyone else, so a program a user planted cannot be what the prompt elevates. The polkit action binds `pkexec` to that exact path as well.
- **One polkit action, with no GUI.** The policy file the app installs names the action, its message, `auth_admin_keep` for an active session and `no` otherwise, and the helper path (`org.freedesktop.policykit.exec.path`). `allow_gui` is `false`, so the helper cannot open windows as root.
- **`pkexec` is trusted as found.** `/usr/bin/pkexec` is used when present; otherwise the first executable `pkexec` on an absolute `PATH` entry. A `pkexec` that is not the system's can only run as the person, not as root, but it could show a prompt of its own; systems that keep `pkexec` elsewhere (NixOS) rely on `PATH`.
- **Never logs paths.** The plugin logs and reports no path or name. The words `pkexec` prints on the error stream before the ready line are discarded unread.
- **The helper's own safety is out of scope.** What the helper does as root, and how it treats what the app sends, is the app's to design and review.

## Platform notes

- **Linux:** full, as above. Flatpak and AppImage report unavailable, because neither can install root-owned files.
- **Windows:** a compiling stub that reports unavailable. A UAC launcher is planned.
- **macOS, Android and iOS:** report unavailable.
