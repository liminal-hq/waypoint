# Waypoint

A tabbed, extensible file manager for Linux and Windows 11, from Liminal HQ. Inspired by Nemo; not bound by it.

Waypoint treats tabs, drag and drop, and remote locations as one connected system: tear tabs off and merge them, join tabs into split views, drop files onto tabs and servers, and browse SFTP, SMB, WebDAV, S3, Git repositories, archives and cloud drives as if they were local folders. It follows the desktop it runs on (colour scheme, accent, icon theme, optional window transparency) and fits the Liminal HQ family (Jar, Threshold, Cadence, Spindle).

Built with Tauri v2, React/TypeScript and Rust.

## Status

Early scaffolding (Milestone 1). The product design and structural architecture are written; the workspaces, the shared protocol crate, the window chrome package and a placeholder app shell exist. No file-manager features yet. Start with `docs/architecture/README.md`.

## Documentation

| File                                      | Purpose                                                               |
| ----------------------------------------- | --------------------------------------------------------------------- |
| `SPEC.md`                                 | Product and design spec (start here for behaviour)                    |
| `docs/architecture/README.md`             | Structural architecture proposal: layers, crates, plugins, milestones |
| `docs/architecture/crates-and-plugins.md` | The concern-by-concern split, reusable vs domain plugins              |
| `docs/architecture/frontend.md`           | Front-end framework evaluation and stack                              |
| `docs/architecture/ci-cd.md`              | CI/CD pipeline following the Liminal HQ house conventions             |
| `docs/architecture/decisions.md`          | Architecture decision log (A-numbers)                                 |
| `docs/decisions.md`                       | Product and design decision log (D-numbers)                           |
| `docs/goals-and-principles.md`            | Why it exists, principles, non-goals                                  |
| `docs/interactions.md`                    | Mouse, keyboard and drag-and-drop rules                               |
| `docs/plugins.md`                         | The extension model (sandboxed, user-installable)                     |
| `docs/theming-and-platforms.md`           | Light/dark, OS theming, transparency, per-desktop frames              |
| `docs/os-integrations.md`                 | Linux and Windows integrations and fallbacks                          |
| `docs/accessibility.md`                   | Accessibility and focus review                                        |
| `docs/shared-components.md`               | Title bar, context menu and settings shell shared across Liminal apps |
| `docs/tauri-tear-off.md`                  | Implementation reference for tab tear-off                             |
| `docs/prototype-plan.md`                  | History of the design prototype                                       |
| `docs/open-questions.md`                  | Unresolved items and assumptions                                      |
| `docs/ui-mockups/README.md`               | Pointer to the reference-only design prototype                        |

## Development

```bash
bun install
bun run validate
```

Contributor conventions are in `AGENTS.md` (mirrored for Claude Code in `CLAUDE.md`).

## Licence

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT licence](LICENSE-MIT) at your option.
