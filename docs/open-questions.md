# Open Questions & Assumptions

## To confirm

1. Is **Waypoint** the final name? Are other candidates still open?
2. Should the unified title bar ship as a shared package (for example `@liminal-hq/titlebar`) that the other apps adopt, or be copied into each app?
3. Plugin runtime: WASM only, or JS as well? Is a native sidecar ever allowed?
4. Is there a public plugin index hosted by Liminal HQ, or is sideloading enough?
5. Flow integration: what exactly gets logged (paths, or only workspace names)?
6. Is Windows 11 a real target, or only a portability showcase?
7. Default checksum algorithm: BLAKE3 or SHA-256?
8. Should the Shelf persist items across reboots by default?

9. Should groups be able to span windows in a later version?

10. Windows: who owns the sparse MSIX package and code signing for the modern Explorer menu?
11. Should Waypoint ship its own FileChooser portal backend, or leave that to a later phase?
12. Which Windows version is the minimum (Windows 10 support)?

## Assumptions made

- Stack: Tauri v2, React and Rust, like the family.
- Brand tokens and type come from the liminal-hq.github.io site.
- The desktop frames in the prototype are illustrative, not pixel copies of the DE themes.
- File data in the prototype is mocked. No real filesystem access.
