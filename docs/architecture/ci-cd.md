# CI/CD

Status: **partly live** — `format`, `licence-headers`, `frontend-tests`, `rust-tests` and `bindings-drift` run today, plus `publish-test-results.yml`; the remaining jobs activate as their parts are scaffolded (table in §3) · decision A8

Waypoint follows the Liminal HQ house pipeline. **Jar** is the closest reference (Bun workspace, the fullest `release.yml`, `publish-test-results.yml`, generated-bindings drift check); **Cadence** contributes the licence-header job, step summaries and the Android-free desktop shape; **Spindle** contributes the Rust toolchain setup action pattern; **Threshold** contributes the release TUI ideas. Shared container images (`ghcr.io/liminal-hq/tauri-ci-desktop`, `tauri-dev-desktop`) are defined in `liminal-hq/.github`.

## 1. Files

| File                                         | Purpose                                                                                                                                                 | Status                                     |
| -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------ |
| `.github/workflows/ci.yml`                   | Quality gate on every pull request, push to `main` and manual dispatch                                                                                  | Live (format, licence headers)             |
| `.github/workflows/publish-test-results.yml` | `workflow_run` workflow that publishes JUnit check runs, so Dependabot and fork PRs (read-only tokens) still get results                                | Live                                       |
| `.github/workflows/release.yml`              | Tag-driven cross-platform bundles and GitHub Release                                                                                                    | Milestone 9                                |
| `.github/dependabot.yml`                     | Weekly cargo, npm and Actions updates; `tauri*` and `@tauri-apps/*` grouped; labels `build` / `ci`                                                      | Live                                       |
| `.github/release.yml`                        | Generated release-notes categories (Enhancements, Fixes, Documentation, Testing, CI and Build, Other Changes); `skip-changelog` and `internal` excluded | Live                                       |
| `scripts/check-headers.sh`                   | Licence-header enforcement                                                                                                                              | Live                                       |
| `scripts/check-release-versions.sh`          | Requires every manifest version to be synchronised before release; `--current-version` prints it                                                        | Milestone 9 (added with the first release) |
| `scripts/generate-protocol-bindings.sh`      | Regenerates `ts-rs` output                                                                                                                              | Live                                       |
| `nextest.toml`                               | `default` and `ci` profiles; `ci` retries twice and writes JUnit                                                                                        | Live                                       |

## 2. Conventions carried over

- `concurrency: ci-${{ github.ref }}` with `cancel-in-progress: true`; minimal `permissions` (`contents: read`, `packages: read`).
- Rust jobs run in the **`ghcr.io/liminal-hq/tauri-ci-desktop:latest`** container (credentials from `github.actor` and `GITHUB_TOKEN`), with `git config --global --add safe.directory "${GITHUB_WORKSPACE}"`, `Swatinem/rust-cache@v2`, and `taiki-e/install-action@nextest`.
- JS jobs use `oven-sh/setup-bun@v2` and `bun install --frozen-lockfile`.
- Test jobs upload JUnit XML as artefacts plus the `event-file`, always (`if: always()`), for `publish-test-results.yml`.
- Job steps end with a step summary where the result is worth reading (Cadence style).
- Actions are pinned by major version and kept current by Dependabot.
- Canadian English in workflow names, step names and summaries ("artefacts", "synchronised").

## 3. Target `ci.yml` jobs

| Job                    | Runs                                                                                                                                                                                                                                                                    | Activates                   |
| ---------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------- |
| `format`               | `bun run format:check`                                                                                                                                                                                                                                                  | Live                        |
| `licence-headers`      | `bash scripts/check-headers.sh`                                                                                                                                                                                                                                         | Live                        |
| `frontend-tests`       | `bun run test:js:ci`, `bun run build` (`tsc` + Vite), JUnit + event-file artefacts                                                                                                                                                                                      | Live                        |
| `rust-tests`           | `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace --profile ci`, JUnit artefact                                                                                                                     | Live                        |
| `bindings-drift`       | `scripts/check-bindings-drift.sh`: snapshot every generated directory (the protocol `generated` folder and each plugin's `guest-js/bindings`), delete them, regenerate with `cargo test`, and `diff`, deleting first so a removed type cannot leave a stale file behind | Live                        |
| `plugin-permissions`   | For each plugin, every `COMMANDS` entry has a `permissions/default.toml` entry (the Threshold ACL pitfall)                                                                                                                                                              | With the first plugin       |
| `dependency-direction` | A small script asserting the rules in `crates-and-plugins.md` §6 (no plugin-to-plugin deps; reusable plugins import no `waypoint-*` crate)                                                                                                                              | With the second plugin      |
| `windows-check`        | `cargo check` / `clippy` on `windows-latest` for crates and plugins with a Windows module; the frontend build                                                                                                                                                           | Milestone 2                 |
| `webview-smoke`        | Launches the app under a virtual display on the CI container and runs the MCP-driven smoke script against WebKitGTK                                                                                                                                                     | Milestone 2 (evaluate cost) |

`bun run validate` is the local mirror of this gate and grows in step with it.

## 4. Release

`release.yml` follows Jar's shape:

1. **Trigger:** push of a tag `vX.Y.Z` or `vX.Y.Z-beta.N`, or `workflow_dispatch` with an optional `release_tag` and `release_draft`.
2. **`prepare-release`:** resolves the tag, verifies the commit is an ancestor of `origin/main`, runs `check-release-versions.sh --current-version` and requires it to equal the tag, marks pre-releases (`-` in the tag), and creates or reuses the GitHub Release.
3. **Build matrix** (all staged as artefacts): Linux x64 and arm64 in the CI container (`deb`, `rpm`), AppImage packaging, Windows x64 and arm64 (installer) plus a portable zip. macOS is not a target and is omitted.
4. **`publish-release`:** downloads the staged artefacts, tags each asset with its OS (`Waypoint_Linux_…`, `Waypoint_Win_…`), writes `SHA256SUMS`, and uploads with `gh release upload --clobber`.
5. **Versioning:** one synchronised version across the root `package.json`, `apps/waypoint/package.json`, `tauri.conf.json`, and the Cargo workspace/crate versions; reusable plugins that have graduated are versioned in the shared repo by `covector`, not here.
6. **Signing and Windows packaging** (sparse MSIX for the modern Explorer menu, code signing) is unresolved (`docs/open-questions.md` #13) and gets its own decision before the first public Windows release.

Flatpak (D45) is a packaging follow-up: portals-first code is written from day one so the Flatpak manifest is packaging work, not a rewrite.

## 5. Automation and agents

- The debug-only MCP bridge is compiled out of release builds (`#[cfg(debug_assertions)]`), so release artefacts never ship it.
- Agents never push, tag, dispatch workflows or edit workflow files without an explicit request (`AGENTS.md`).
