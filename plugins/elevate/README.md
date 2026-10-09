# @liminal-hq/plugin-elevate

Starts a helper program with administrator rights through the system's own prompt, and reports whether it can. On Linux the prompt is polkit's, through `pkexec`; on Windows it is UAC's, through `ShellExecuteExW` with the `runas` verb.

The plugin only starts the helper and hands back the pipes to talk to it. What the helper does, and the protocol spoken over the pipes, belong to the app. The plugin knows nothing about the app around it: the app tells it which helper to start, where its polkit policy is (Linux), the one line the helper writes when it is running, and the start of the pipe names (Windows).

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
    )
    // Windows only: the start of the names of the pipes of a launch.
    .with_pipe_prefix("my-app-elevate");
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
        // `stream.reader` is what the helper writes (its standard output on Linux, a pipe on
        // Windows) and `stream.writer` what it reads.
        // Dropping the stream closes both, which ends the helper.
    }
    Err(LaunchError::Dismissed) => { /* the dialog was closed */ }
    Err(LaunchError::NotAuthorised) => { /* wrong password, or not an administrator */ }
    Err(LaunchError::Cancelled) => { /* `user_cancelled()` returned true */ }
    Err(other) => { /* `other` is a message for the person; it never names a path */ }
}
```

## API

| Item                                                        | What it does                                                                                                                                                                          |
| ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `getStatus()`                                               | `{ available, reason, flavour, features }`; see below.                                                                                                                                |
| `hasFeature(status, name)`, `featureReason(status, name)`   | Read the status.                                                                                                                                                                      |
| `Config { helper, policy, ready_line, pipe_prefix }` (Rust) | The helper's absolute path, the installed polkit policy's absolute path (Linux), the ready line, and the pipe-name prefix set by `with_pipe_prefix` (Windows). There are no defaults. |
| `Elevator::status()` (Rust)                                 | The same status, read again on every call.                                                                                                                                            |
| `Elevator::launch(&cancelled)` (Rust)                       | Starts the helper and blocks until it is running. Returns an `ElevatedStream { reader, writer }` that holds the process.                                                              |
| `launch::launch_with`, `Spawner`, `Spawned` (Rust)          | The launching loop over a trait for the child process, so it can be tested with a script. `polkit::check` and `Probe` do the same for the status.                                     |

## Features and status

`getStatus()` returns `{ available, reason, flavour, features }`, where each feature is `{ name, available, reason }`. There is one feature, `elevate`. `flavour` is `polkit`, `uac` or `unsupported`.

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

On Windows, `elevate` is available only when all of these hold; the first that fails gives the reason:

| Check                                                                                                                                     | Reason                                                                                 |
| ----------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| The app's token is not already elevated (when it can be read)                                                                             | The app is already running as an administrator                                         |
| The app configured a pipe-name prefix                                                                                                     | Elevation is not set up in this build                                                  |
| The Program Files folders are known (`SHGetKnownFolderPath` for `FOLDERID_ProgramFiles`, `...X86` and `...X64`)                           | The Program Files folder could not be found                                            |
| The helper and the running app are both strictly under one of those folders (whole components, any case, no `..`, no UNC or device paths) | Elevation is not available in the portable version; install the app with its installer |
| The helper exists and is a regular file                                                                                                   | The administrator helper is not installed                                              |
| The helper is not a link or other reparse point                                                                                           | The administrator helper is not a regular file                                         |

On every other platform the reason is "Not implemented on this platform yet".

## The launch contract (Linux)

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

## The launch contract (Windows)

`launch` runs the status check first, then:

1. Makes two byte-mode pipes with random names, `\\.\pipe\<prefix>-<128 random bits>-to` (the app writes, the helper reads) and `...-from` (the helper writes, the app reads). Two one-direction pipes, because a pending read on one synchronous pipe handle would block a write on the same handle from another thread. Each is the first instance of its name, allows one instance, rejects remote clients, is not inheritable, and has a protected DACL that grants read and write to the invoking user's SID only (built from SDDL; nothing for Everyone, the anonymous account or the network).
2. Makes a per-launch token of 256 bits from `BCryptGenRandom`.
3. Starts the helper with `ShellExecuteExW` and the `runas` verb (hidden window) on a thread of its own, because the call blocks while the UAC prompt is open. The command line is `--pipe-to <name> --pipe-from <name> --token <hex> --parent <app pid>`. `launch` polls `cancelled` about every 50 ms while it waits; a declined prompt (`ERROR_CANCELLED`) is `Dismissed`; a launch that returns no process handle fails closed.
4. Once the prompt is answered, the helper has 15 seconds to connect to both pipes. Waiting for it is interruptible: `cancelled`, the helper's exit and the time limit all end the wait. An elevated process cannot be killed from the app; the pipes are closed instead, so a helper that starts late finds nothing to connect to and exits.
5. After the helper connects, `GetNamedPipeClientProcessId` of each pipe must equal the launched process's id, and that process's image path must equal the configured helper path (compared by whole components, ignoring case). Then the helper's first line on the `-from` pipe must be exactly `<ready line> <token>`, the token compared in constant time. Any mismatch disconnects both pipes and fails closed.

| Outcome                  | When                                                                                                                                                     |
| ------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Ok(ElevatedStream)`     | Every check above passed.                                                                                                                                |
| `Cancelled`              | `cancelled()` returned true first.                                                                                                                       |
| `Dismissed`              | The person declined the UAC prompt.                                                                                                                      |
| `Failed { code }`        | No process handle, the helper ended before it connected (`code` is its exit status), it did not connect or present the token in time, or a check failed. |
| `Unavailable { reason }` | The status check failed.                                                                                                                                 |
| `Io { kind }`            | A pipe, a thread or the launch could not be made.                                                                                                        |

Dropping the stream closes the `-to` pipe first, which the helper sees as the end of its input, then the `-from` pipe, then the process handle (which is only released: an elevated process cannot be signalled).

## Security notes

- **No page-callable launch.** The only command is `get_status`. A command that started an elevated process from the webview would let script in the page cause a prompt and an elevated session; launching exists only as a Rust API for the app to compose behind its own explicit actions.
- **The helper path is protected.** The helper is offered only when it and every folder above it belong to root and cannot be changed by anyone else, so a program a user planted cannot be what the prompt elevates. The polkit action binds `pkexec` to that exact path as well.
- **One polkit action, with no GUI.** The policy file the app installs names the action, its message, `auth_admin_keep` for an active session and `no` otherwise, and the helper path (`org.freedesktop.policykit.exec.path`). `allow_gui` is `false`, so the helper cannot open windows as root.
- **`pkexec` is trusted as found.** `/usr/bin/pkexec` is used when present; otherwise the first executable `pkexec` on an absolute `PATH` entry. A `pkexec` that is not the system's can only run as the person, not as root, but it could show a prompt of its own; systems that keep `pkexec` elsewhere (NixOS) rely on `PATH`.
- **Never logs paths.** The plugin logs and reports no path or name. The words `pkexec` prints on the error stream before the ready line are discarded unread.
- **Windows: only an installed copy.** The helper and the app must be under Program Files, where a standard user cannot replace them, so a program the person planted cannot be what UAC elevates. A portable copy reports unavailable. The helper must not be a link.
- **Windows: the pipes.** Random names, the first instance of each name, one instance, no remote clients and a DACL for the invoking user only; the helper's process is checked by id and image path, and it must present the launch token. A standard user who approves with an administrator's credentials runs the helper as a different account, which the DACL does not admit, so that case fails (as `Failed`) rather than widening the access list.
- **Windows: the same-user residual risk.** The token is on the helper's command line, which other processes of the same user can read, and code running inside the app's own process as the same user can do what the app can. UAC consent protects against processes that were not already running as the person; it does not defend against code injected into the app.
- **The helper's own safety is out of scope.** What the helper does as root, and how it treats what the app sends, is the app's to design and review.

## Platform notes

- **Linux:** full, as above. Flatpak and AppImage report unavailable, because neither can install root-owned files.
- **Windows:** partial. Written against the Win32 API and type-checked and linted for `x86_64-pc-windows-msvc`, with the pure logic (pipe names, token, SDDL, command line, path comparison, availability) tested on Linux; the launch itself has not yet been run on Windows. `tests/live_windows.rs` is the manual check.
- **macOS, Android and iOS:** report unavailable.
