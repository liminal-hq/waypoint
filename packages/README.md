# Packages

Shared TypeScript packages, consumed through Bun workspaces. Anything here is written to be independent of the Waypoint app, so it can be extracted and shared with the other Liminal HQ apps. The front-end reasoning is in [`docs/architecture/frontend.md`](../docs/architecture/frontend.md).

## Rules

- **No Waypoint domain imports.** A package never imports from `apps/` or `crates/`, and never mentions file-manager concepts. The app depends on packages, not the other way round.
- **Deep-path exports, no barrel files.** Each public module is mapped in the package's `exports` to the file that defines it (for example `@liminal-hq/waypoint-chrome/TitleBar`).
- **Styling through tokens.** Components read the semantic `--wp-*` CSS custom properties, with fallbacks, so they work before the app defines them.
- **Accessible by default.** Roles, names, focus order and keyboard paths follow [`docs/accessibility.md`](../docs/accessibility.md).
- **Every source file carries the licence header** described in [`AGENTS.md`](../AGENTS.md).

## Packages

| Package                                 | Status | What it is                                                                                                      |
| --------------------------------------- | ------ | --------------------------------------------------------------------------------------------------------------- |
| [`@liminal-hq/waypoint-chrome`](chrome) | Built  | The unified Liminal title bar, the shared window menu and the context menu. See its [README](chrome/README.md). |

## Working on the packages

```bash
bun run typecheck    # tsc for every workspace
bun run test:js      # Vitest for every workspace
```

Each package also has its own `typecheck` and `test` scripts, for example `bun run --cwd packages/chrome test`.
