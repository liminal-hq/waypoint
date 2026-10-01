# Crates

Pure Rust crates that hold Waypoint's domain logic. They have **no `tauri` dependency**, so they can be built and tested headlessly, and they are what the thin Tauri plugins in [`plugins/`](../plugins) adapt to the app. The reasoning is in [`docs/architecture/`](../docs/architecture/README.md).

## Rules

- **No `tauri`.** A crate here never depends on `tauri` or on a Tauri plugin. If it needs something from the platform (Trash, secrets, the file system), it defines a trait and the app injects the implementation.
- **Traits, not other plugins.** Crates depend on one another only through `waypoint-protocol`, `waypoint-path` and traits, never on a plugin's Tauri API.
- **Types are generated.** Wire types are authored once in `waypoint-protocol` and generated to TypeScript with `ts-rs` (`bun run bindings`). Never hand-edit the generated files; CI fails on drift.
- **Every source file carries the licence header** described in [`AGENTS.md`](../AGENTS.md).

## Crates

| Crate                                                | Status  | What it is                                                                                                                                                                              |
| ---------------------------------------------------- | ------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [`waypoint-protocol`](waypoint-protocol)             | Built   | Shared wire-format types (`PluginStatus`, `WindowKind`) and the `ts-rs` generation that feeds the front end                                                                             |
| [`waypoint-path`](waypoint-path)                     | Built   | The common path and URI type: local paths (including Windows drive letters and long paths) are built; `sftp://`, `smb://`, `davs://`, `s3://`, `git+file://` and archive URIs           |
| [`waypoint-vfs`](waypoint-vfs)                       | Partial | The `Provider` trait, the local provider and listing handles with sorted and filtered indexes, and the local watcher are built; the provider registry, and tags and comments follow     |
| `waypoint-provider-{sftp,smb,webdav,s3,archive,git}` | Planned | One crate per protocol behind Cargo features, so heavy dependencies stay optional                                                                                                       |
| [`waypoint-ops`](waypoint-ops)                       | Partial | The operations engine: the model, planner, queue and the executors for create, rename, duplicate, trash, restore and delete are built; the journal, the plugin and copy and move follow |
| `waypoint-session`                                   | Planned | Windows, tabs, groups, split layouts, saved layouts, session restore and the tab hand-off used by tear-off                                                                              |
| `waypoint-search`                                    | Planned | Scoped search, filters, content and regular-expression search, and smart folders                                                                                                        |
| `waypoint-ext`                                       | Planned | The extension host: manifests, permissions, lifecycle and the extension-point registry                                                                                                  |

Only add a crate when a milestone needs it. The full split, including which concerns become reusable plugins instead, is in [`docs/architecture/crates-and-plugins.md`](../docs/architecture/crates-and-plugins.md).

## Working on the crates

```bash
cargo test --workspace                   # unit tests, and regenerates the TypeScript bindings
cargo nextest run --workspace            # the runner CI uses
cargo clippy --workspace --all-targets -- -D warnings
bun run bindings                         # regenerate the TypeScript types on their own
```
