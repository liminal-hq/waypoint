# Elevated operations

Status: **design for milestone 9, reviewed and accepted by the owner (2026-10-08)** (#436, #437) · decisions D217, D218, A154 and A155 · nothing here is built yet; the issues that build it are #438 (Linux helper), #439 (Windows helper) and #440 (the command and the marker).

Open as Administrator lets a person browse and change files that need more privilege than they have, through the system's own prompt (polkit on Linux, UAC on Windows). It never runs Waypoint as root or as an administrator. This page is the threat model and the design the helpers are built from.

## 1. Goals and non-goals

**Goals**

- A tab can be elevated and then browsed and used like any other tab: listing, selecting, creating, renaming, deleting, copying and moving (the job queue, conflicts, verification and undo all work), properties, split view, pairs and tear-off, with live updates.
- Nothing runs elevated without an explicit prompt from the system, and a restored session never re-prompts by itself.
- An elevated tab and window are unmistakable, and a person can leave elevation in one step.
- The unprivileged app keeps its shape: one writer per piece of state, Rust owns state, the page renders.

**Non-goals**

- Running the whole app, a window, or the webview as root or as an administrator.
- Running programs as root through the helper. The helper serves file operations only (§5). Running one program as an administrator is a separate Windows-only feature (D218).
- Elevated Trash. The Trash belongs to the person, so Delete in an elevated tab is permanent (D217).
- Elevated search indexing, NTFS MFT or USN reading, or disk tools (`docs/architecture/milestone-7-spikes.md`).
- A helper that is installed by the app itself, runs from a user-writable folder, or stays up with no elevated tab open.

## 2. The shape of it

```
 webview (unprivileged)
    │  ordinary Tauri commands, the same as any location
 waypoint-vfs / waypoint-ops (unprivileged, in the app process)
    │  Provider calls on `admin:///…`
 ElevatedProvider  (crates/waypoint-elevated)
    │  length-prefixed frames over a private duplex stream
 waypoint-elevate-helper  (a small command-line program, no UI, privileged)
    │  the same LocalProvider the app uses
 the file system
```

- **A location, not a mode (A155).** Elevation is the URI scheme `admin`: `admin:///etc` is `/etc` as seen by the helper. `VfsPath` gains an `Elevated` variant beside `Trash`, `Remote`, `Archive` and `Git`. The tab's location says it is elevated, so the marker is derived from it, nothing is added to the session store and `session.json` keeps its version. Leaving elevation is navigating the same tab to the `file:` form of the same path, so history is kept.
- **The helper is the privileged side of a provider (A154).** `ElevatedProvider` implements `waypoint_vfs::Provider` by sending each call to the helper, which performs it with `LocalProvider`. Behaviour is therefore the same as in an ordinary tab, and the operations engine, the conflict resolver, verification, the journal and cross-provider copy need no change.
- **Authenticating is the provider's `connect`.** The provider reports a connection state like a remote one. A restored or dropped `admin:` location is disconnected, so it shows the "authenticate to continue" state and nothing prompts until the person chooses to. Closing the last elevated tab, or leaving elevation in it, disconnects, and the helper exits.

## 3. Starting the helper

The helper is started by the system's own mechanism, from a fixed, protected location, and talks to the app over a stream only the two of them can reach.

### Linux (polkit)

- One polkit action, `ca.liminalhq.waypoint.admin`, with the default `auth_admin_keep` and the `org.freedesktop.policykit.exec.path` annotation set to `/usr/libexec/waypoint/waypoint-elevate-helper`. The policy file and the helper are installed, root-owned, by the `.deb` and `.rpm`.
- The app starts `pkexec` on that path with the helper's standard input and output as the stream. There is no socket and no system D-Bus service, so no other user or process on the machine can reach the root process, and the helper ends when its input does.
- The prompt says exactly what is being asked: the action's message is "Waypoint wants to read and change files as an administrator", not a generic "authentication is required", so the person knows the scope of what they approve (the one weakness of `kio-admin`'s single generic action, §9).
- The Services panel reports elevation unavailable, and every entry is hidden, unless `pkexec`, the policy and the helper are all present and the helper is in the protected path. The AppImage cannot install root-owned files, so it reports unavailable with that reason.

### Windows (UAC)

- The app starts the helper with `ShellExecuteExW` and the `runas` verb. The prompt is the system's; Waypoint never sees credentials.
- The stream is a named pipe with a random name, created so that its DACL admits only the invoking user's SID and nothing remote. The launch carries a random per-launch token which the helper presents first. The app checks the helper's process (`GetNamedPipeClientProcessId`, then its image path) against the installed helper, and the helper checks the same of the app.
- Elevation is offered only when the helper sits under Program Files, where a standard user cannot replace it. The portable zip cannot satisfy that, so it reports elevation unavailable with that reason. The NSIS installer places the helper beside `waypoint.exe`.

### Both

- Availability uses the standard shape (`PluginStatus`, `get_status`, the Services panel); entries are hidden where it is off (`docs/os-integrations.md` principle 3).
- The feature is behind Settings → Experimental → Administrator access, off by default, until the verification passes are recorded (D217).

## 4. The wire protocol (outline)

The detail is written with the crate (`crates/waypoint-elevated`); the rules are fixed here.

- Frames are a four-byte length, then a request or response body, with a hard size cap (a few MiB); a frame over the cap, or one that does not parse, closes the connection.
- Every request has an id and gets exactly one response. Paths are absolute and normalised, in the helper's own form (`file:` paths), and the client maps `admin:` to them; anything relative, containing `..`, or of another scheme is refused before it reaches the file system.
- Open files are handles in a table of bounded size with a per-handle mode; a handle is closed by the client, or when the connection drops.
- `watch` is a request. The helper runs `LocalProvider`'s watcher as the elevated user and sends each event (`Changes`, `Rescan`, `Degraded`, `Lost`) as an unsolicited frame tagged with the watch id; the client rewrites its paths to `admin:` and refuses an event outside the watched folder. Dropping the client's watch sends an unwatch. The send queue is bounded: when it overflows, the helper sends one `Rescan` instead. The helper ending, or the connection dropping, turns every watch into `Lost`, and the tab drops into the authenticate state instead of showing stale rows.
- The helper keeps no state beyond the connection. It exits on end of input, on a protocol error, and after an idle period with no requests, no open handles and no watches.

## 5. What the helper may do

An allow-list; anything else is not in the protocol, so there is nothing to refuse:

`stat`, `list` (and batched listing), `open_read` and ranged reads, `create_dir`, `create_file`, `create_write` and `resume_write`, `rename`, `remove_file`, `remove_dir`, `copy_file_within`, `permissions` and `set_permissions`, `set_times`, `symlink`, `read_link`, `resolve_link`, `canonicalize`, `volume_id`, `free_space`, `folder_size` and `details`, `watch`.

There is no request that starts a program, runs a shell, loads code, changes an owner, mounts anything or reads the environment. The set is the `Provider` trait's, so adding to it is a change to this page first.

## 6. Threat model

| Asset                | Attacker                                                              | Threat                                                                       | Mitigation                                                                                                                                                                                                                                                                                                                                                                                          |
| -------------------- | --------------------------------------------------------------------- | ---------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| The elevated session | Script in the webview (XSS, a malicious file name, a hostile preview) | Uses the helper to read or change files as the administrator                 | The page cannot talk to the helper at all: it calls the same Tauri commands as for any location, and only while the person has a connected elevated tab. The most it can do is what the person could do in that tab; it cannot start programs (§5).                                                                                                                                                 |
| The elevated session | Another local user                                                    | Connects to the root process                                                 | Linux: no socket, the stream is the helper's own pipes. Windows: a pipe whose DACL admits only the invoking SID, a random name and the launch token.                                                                                                                                                                                                                                                |
| The helper binary    | A process running as the same user                                    | Replaces a user-writable helper before the prompt, so the prompt elevates it | Only a helper in a protected location is ever launched (polkit binds the exact path; Windows requires Program Files). A copy elsewhere (AppImage, portable zip) is reported unavailable.                                                                                                                                                                                                            |
| The prompt           | Any program                                                           | Triggers a prompt to wear the person down                                    | A prompt starts only from an explicit command, never from restore, never from a script path of its own; a cancelled or refused prompt leaves nothing running and shows the state in the tab.                                                                                                                                                                                                        |
| The stream           | A same-user process that can inject into the app                      | Speaks to the helper as the app (Windows)                                    | **Residual, accepted by the owner (2026-10-08).** The token, the pipe DACL and the image check raise the cost, but code running inside the app's own process as the same user can do what the app can. UAC consent protects against processes that were not already running as the person; it does not defend against code injected into Waypoint. Stated in the Services panel text and `SPEC.md`. |
| Files                | A symlink or rename race                                              | A user-writable folder is changed between a check and the operation          | The helper opens the final path component without following links where the platform allows, and treats `canonicalize` and `resolve_link` as reads only; the paths it receives are not trusted to stay as they were. Reviewed again with the code in #438.                                                                                                                                          |
| Memory and disk      | A hostile frame                                                       | Exhausts the helper                                                          | Frame and handle caps, bounded watch queues, bounded requests in flight, an idle exit.                                                                                                                                                                                                                                                                                                              |
| Logs                 | A reader of the log                                                   | Reads paths and names from an elevated log                                   | The helper logs the operation kind and an id, never a path or a name; the app's own log follows the same rule for elevated locations.                                                                                                                                                                                                                                                               |
| The person           | Confusion                                                             | Acts as an administrator without knowing                                     | The marker (tab badge, title-bar badge, window title, frame accent), none of it colour alone; Delete states that it is permanent; leaving elevation is one step.                                                                                                                                                                                                                                    |
| Session and settings | A copied `session.json`                                               | Restores into a prompt that looks routine                                    | An `admin:` tab restores disconnected into the authenticate state, with the path shown and a button; it never prompts by itself.                                                                                                                                                                                                                                                                    |

**What an attacker with the webview can do, in one line:** act as the person in a connected elevated tab, through the allow-listed file operations, until that tab is closed or left. They cannot start a program, reach the helper without a connected tab, or make a prompt appear without a command.

## 7. What an elevated tab can and cannot do

It works like a regular tab, except where a file is handed to another program, which runs as the person:

- Opening a file with its default application, and Open With, are hidden for `admin:` files until the open-through-a-local-copy mechanism of #580 to #582 exists. Folders open as tabs as usual.
- Drag out to other applications is not offered (it needs real paths). Drag within Waypoint works.
- Thumbnails and the preview use the provider where they can; where they cannot, the type icon shows. Settled with the code in #438.
- Delete is permanent, with its own confirmation even when confirmation is off for the Trash (D217).

## 8. Verification

- Headless: the shared conformance suite against `ElevatedProvider` over an in-memory duplex stream to a `LocalProvider` in a temporary folder, plus tests for the path policy, the frame limits, handle exhaustion, the idle exit, and watching (events carry `admin:` paths, overflow becomes `Rescan`, the connection ending becomes `Lost`). The helper program is also started over its real standard input and output as the current user.
- Windows: written alongside the Linux code and type-checked and linted in the Windows jobs (A11).
- By hand, recorded in the milestone 9 verification page: the real polkit and UAC prompts, a refused and a cancelled prompt, a copy into a protected folder succeeding only after the prompt, a change made from another program appearing in the tab, and the AppImage and portable zip reporting unavailable.

## 9. Prior art

How other Linux file managers give access to files the person cannot reach, and what this design takes from them:

| File manager | Mechanism                                                                                                                                                                                                                                                      | What it means here                                                                                                                                                                                                                               |
| ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Nemo         | "Open as Root" runs the whole file manager as root with `pkexec nemo <path>` (polkit action `org.nemo.root`). It breaks on Wayland, and the project has an open issue to move to GVFS's `admin://`.                                                            | The pattern this design rejects: a root webview and the whole app's attack surface.                                                                                                                                                              |
| Nautilus     | An `admin:///` location served by a privileged GVFS backend (`gvfsd-admin`, started through `pkexec`, with polkit actions and a prompt per client application).                                                                                                | The location-as-elevation idea (A155) and a helper started through `pkexec` (A154). Part of why the backend exists is that GUI programs under `pkexec` do not work on Wayland; a command-line helper has no display to need.                     |
| Dolphin      | An `admin:///` location through `kio-admin`: a root D-Bus service, activated on demand, doing the file operations under one polkit action (`org.kde.kio.admin.commands`, `auth_admin_keep`). Dolphin refuses to run as root and shows a red bar in admin mode. | The same shape, with a system service instead of `pkexec`. Not chosen: it installs a system service and bus policy, and the service outlives the tab. Taken: the visible mode marker, and a more specific prompt than its single generic action. |

The two mature implementations agree with this design's shape: an unprivileged app, a small helper and an `admin:` location. The choice of `pkexec` over pipes rather than a D-Bus service is deliberate (smallest attack surface, simplest packaging) and can be revisited without changing anything above the transport.
