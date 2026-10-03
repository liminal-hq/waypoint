# Open Questions & Assumptions

## To confirm

1. Is **Waypoint** the final name? Are other candidates still open?
2. Should the unified title bar ship as a shared package (for example `@liminal-hq/titlebar`) that the other apps adopt, or be copied into each app?
3. Plugin runtime: WASM only, or JS as well? Is a native sidecar ever allowed? **Answered (2026-10-03): WebAssembly only (the component model); no JavaScript runtime and no native sidecar.**
4. Is there a public plugin index hosted by Liminal HQ, or is sideloading enough? **Answered (2026-10-03): sideloading only for now (a file, a URL or a Git repository); the manifest should let an index be added later.**
5. Flow integration: what exactly gets logged (paths, or only workspace names)?
6. Is Windows 11 a real target, or only a portability showcase?
7. Should groups be able to span windows in a later version? **Answered (2026-10-03): yes (reverses D36); planned in milestone 10.**

8. Windows: who owns the sparse MSIX package and code signing for the modern Explorer menu? **Deferred (2026-10-03): out of scope until an undetermined date; Windows builds are unsigned and the modern Explorer menu waits.**
9. Should Waypoint ship its own FileChooser portal backend, or leave that to a later phase? **Answered (2026-10-03): yes, in milestone 10, starting with a decision and spike.**
10. Which Windows version is the minimum (Windows 10 support)? **Deferred (2026-10-03): out of scope until an undetermined date; Windows 11 stays the target.**

## Assumptions made

- Stack: Tauri v2, React and Rust, like the family.
- Brand tokens and type come from the liminal-hq.github.io site.
- The desktop frames in the prototype are illustrative, not pixel copies of the DE themes.
- File data in the prototype is mocked. No real filesystem access.
