# Plugins

> **Terminology:** this document describes user-installable, sandboxed **extensions** (shown as "Plugins" in the UI, and called extensions in code). They are unrelated to the build-time **Tauri plugins** (Rust crates plus npm packages) described in `docs/architecture/`.

## Model

- Package: a `waypoint-plugin.toml` manifest plus a WASM or JS module, with an optional native sidecar (needs extra consent).
- Sandboxed. The permissions a plugin declares are shown at install time and can be revoked later.
- Permissions: `fs.read`, `fs.write`, `net`, `exec`, `clipboard`, `secrets`, `ui.panel`.
- Lifecycle: install → grant permissions → enable → (update) → disable or uninstall. A crashed plugin is disabled and flagged, and the rest of the app keeps running.
- Sources: the Liminal plugin index, a URL, or a local folder (dev mode).

## Extension points

| Point         | What it adds                             | Example                     |
| ------------- | ---------------------------------------- | --------------------------- |
| `column`      | A list view column plus a sortable value | Git status, media duration  |
| `action`      | Context menu and palette commands        | "Resize image…"             |
| `dropAction`  | An entry in the action picker            | "Convert to WebP here"      |
| `provider`    | A location scheme                        | `gdrive://`, `nextcloud://` |
| `thumbnailer` | Thumbnails for MIME types                | `.blend`, `.kra`            |
| `previewer`   | Quick Look or Inspector rendering        | Markdown, CSV table         |
| `panel`       | An Inspector tab or sidebar section      | Git log                     |
| `statusItem`  | A status bar widget                      | Current branch              |
| `renameToken` | A batch rename token                     | `{exif.date}`               |
| `emblem`      | An icon overlay                          | Sync state                  |

## Sample plugins (for the prototype)

| Plugin                                 | Points                                    | Permissions        | Status in the mock           |
| -------------------------------------- | ----------------------------------------- | ------------------ | ---------------------------- |
| Git Status                             | column, emblem, statusItem, action, panel | fs.read, exec(git) | Enabled                      |
| Image Tools                            | action, dropAction                        | fs.read, fs.write  | Enabled                      |
| Cloud Drive (Nextcloud / Google Drive) | provider, emblem                          | net, secrets       | Enabled; Drive needs sign-in |
| Media Info                             | column, previewer                         | fs.read            | Enabled                      |
| Archive Handler (7z, rar)              | provider, thumbnailer                     | fs.read, fs.write  | Update available             |
| Rename Rules                           | renameToken                               | none               | Enabled                      |
| Custom Columns Demo                    | column                                    | fs.read            | Disabled, dev-loaded         |

## Plugin manager UI

- Tabs: Installed, Browse and Updates.
- A row shows the icon, name, author, version, an enable switch, extension point chips and a health dot.
- The detail pane has the description, permissions (each revocable), settings (rendered from the manifest schema), a changelog and logs.
- A warning banner appears for plugins that ask for `exec` or a native sidecar.

## Bundled plugins

Some features ship as plugins so they can be switched off, list their permissions, and stay out of the core. They appear in Settings → Plugins with a “Bundled” label. When a plugin is off, its menu items, palette commands and dialogs disappear.

| Plugin            | Adds                                                                 | Permissions                         | Default                                           |
| ----------------- | -------------------------------------------------------------------- | ----------------------------------- | ------------------------------------------------- |
| Find Duplicates   | “Find Duplicates in Here…”, palette command, duplicate finder dialog | fs.read, trash                      | On                                                |
| Compare and Sync  | “Compare and Sync…” in the pair menu                                 | fs.read, fs.write                   | On                                                |
| Previous Versions | A “Previous versions” list in Properties                             | fs.read, fs.write, exec (snapshots) | On; hides itself when no snapshots exist          |
| Network Sharing   | “Share Over Network…”, sharing state in Properties                   | net, exec (net usershare)           | On; unavailable without Samba or NFS              |
| Nearby Devices    | “Nearby Devices…” in the Share menu                                  | net, fs.read                        | On; unavailable without Avahi                     |
| Disk Tools        | “Format…” on drives, “Mount Disk Image” on .iso files                | privileged                          | On; nothing runs and no prompt appears until used |
| Encrypted Vaults  | “New Encrypted Vault…”, unlock prompt                                | exec (gocryptfs), secrets, fs.write | Off; needs gocryptfs or CryFS                     |

**Stay in the core:** files, tabs, panes, drag and drop, search, operations queue, Trash, sidebar, Properties, Settings, and the terminal drawer and tabs.

**Unavailable:** a plugin whose system dependency is missing shows as “Unavailable: needs Samba” (for example) and stays off. Per the integration rule, options for unavailable features are hidden.
