# Documentation

Waypoint's documentation lives here and is maintained alongside the code: a change that alters behaviour, a design decision or the structure updates the matching document in the same pull request. The repository-wide rules are in [`AGENTS.md`](../AGENTS.md).

## Where to start

- **What is Waypoint supposed to do?** Read [`SPEC.md`](../SPEC.md), then [`goals-and-principles.md`](goals-and-principles.md).
- **How is it built?** Read [`architecture/README.md`](architecture/README.md).
- **Why was it decided that way?** Read the decision logs: [`decisions.md`](decisions.md) for product and design (D-numbers) and [`architecture/decisions.md`](architecture/decisions.md) for structure (A-numbers).

## Product and design

| Document                                               | What it covers                                                                       |
| ------------------------------------------------------ | ------------------------------------------------------------------------------------ |
| [`../SPEC.md`](../SPEC.md)                             | The master product and design spec                                                   |
| [`goals-and-principles.md`](goals-and-principles.md)   | Why Waypoint exists, its principles and its non-goals                                |
| [`decisions.md`](decisions.md)                         | The product and design decision log (D1 onwards)                                     |
| [`interactions.md`](interactions.md)                   | The mouse, keyboard and drag-and-drop rules                                          |
| [`plugins.md`](plugins.md)                             | The extension model: sandboxed, user-installable extensions and the bundled features |
| [`theming-and-platforms.md`](theming-and-platforms.md) | Light and dark, OS theming, transparency and the per-desktop frames                  |
| [`os-integrations.md`](os-integrations.md)             | Linux and Windows integrations, fallbacks and where they appear in the UI            |
| [`accessibility.md`](accessibility.md)                 | The accessibility and focus review: what exists and what to fix                      |
| [`shared-components.md`](shared-components.md)         | The title bar, context menu and settings shell shared across Liminal HQ apps         |
| [`tauri-tear-off.md`](tauri-tear-off.md)               | The implementation reference for dragging a tab out into a new window                |
| [`open-questions.md`](open-questions.md)               | Unresolved questions and assumptions                                                 |
| [`prototype-plan.md`](prototype-plan.md)               | The history of the design prototype (historical)                                     |
| [`ui-mockups/`](ui-mockups/README.md)                  | A pointer to the reference-only design prototype                                     |

## Architecture

| Document                                                                   | What it covers                                                                     |
| -------------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| [`architecture/README.md`](architecture/README.md)                         | Layers, the composition model, data flow, windows, milestones and risks            |
| [`architecture/crates-and-plugins.md`](architecture/crates-and-plugins.md) | The concern-by-concern split into crates and Tauri plugins, and which are reusable |
| [`architecture/frontend.md`](architecture/frontend.md)                     | The front-end framework evaluation, the chosen stack and the front-end structure   |
| [`architecture/ci-cd.md`](architecture/ci-cd.md)                           | The CI/CD pipeline and release process                                             |
| [`architecture/decisions.md`](architecture/decisions.md)                   | The architecture decision log (A1 onwards)                                         |

## Keeping the docs current

- **Behaviour changes** update `SPEC.md`. **Design decisions** add or amend an entry in `decisions.md`. **Structural changes** (crate or plugin boundaries, data flow, CI) update `architecture/` and its decision log.
- **Open questions** are resolved by moving the answer into the relevant decision log and deleting the question.
- **Two meanings of "plugin".** A _native plugin_ is a build-time Tauri plugin; an _extension_ is a user-installable, sandboxed add-on. `plugins.md` describes extensions.
- **Prose style.** Canadian English, one line per paragraph or list item (no hard wrapping), and real em dashes.
- **The prototype is reference only.** When it disagrees with `SPEC.md`, fix the spec with a decision entry rather than treating the prototype as correct.
