# UI mockups (reference only)

Waypoint's product design was prototyped in a Claude Design project: `Waypoint.dc.html` (main window), `Waypoint Settings.dc.html`, `Waypoint Desktops.dc.html`, `Waypoint Assets.dc.html`, `Waypoint Design System.dc.html`, plus `support.js` and `wp-icons.js`. That project is `https://claude.ai/design/p/c04895a2-5376-4c31-b41a-b313c0352f48`.

The prototype's code is **not** the source of truth and is not to be ported, adapted or structurally mirrored: it uses inline styles, mock data and a single-file component model that the real app deliberately does not share. Only the _behaviour_ it demonstrates is authoritative, and only as captured in `SPEC.md`, `docs/decisions.md` and `docs/interactions.md`.

The prototype files are not vendored into this repository. If an offline copy is wanted for review, export them into this directory; `.prettierignore` already excludes `docs/ui-mockups/`.

When a prototype behaviour disagrees with `SPEC.md`, fix the spec (with a `docs/decisions.md` entry) rather than treating the prototype as correct.
