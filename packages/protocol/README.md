# @liminal-hq/waypoint-protocol

The TypeScript side of Waypoint's Rust wire types. Every file in `src/generated/` is written by `ts-rs` from a `#[ts(export)]` type in `crates/waypoint-protocol` or another domain crate such as `crates/waypoint-vfs`; nothing here is hand-written and nothing here has behaviour, so the package is types only.

It exists so the app and each plugin's `guest-js` package can import the same shapes (`Location`, `EntryId`, `Entry`, `ListingEvent` and so on) by package name, instead of one of them reaching into the other's source tree.

## Using it

Import a type from its deep path, with no barrel file:

```ts
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
```

## Adding or changing a type

Annotate the Rust type with `#[ts(export, export_to = "../../../packages/protocol/src/generated/")]` (the path is relative to the crate's `src/`), then run `bun run bindings` and commit the regenerated files. CI runs `bun run check:bindings`, which deletes this directory, regenerates it and fails on any difference.

Reusable plugins that graduate to `tauri-plugins-workspace` export into their own `guest-js/bindings` instead and never import from this package.
