# Questro 3.1 audit

Reviewed on 10 October 2026: the native Rust backend, Tauri bridge and window configuration, renderer state and localization, public feed normalization, process ownership, timers, persistence, artwork downloads and portable build scripts.

## Confirmed issues repaired

- Custom Process previously polluted the Library selection. The modes now maintain separate selection state. An untouched Library remains empty after mode or tab changes, while a real game and its chosen executable are preserved.
- Completed and cancelled games could return in the saved queue. Persistence now excludes them. Failure to save history cannot leave a stopped queue running; setting and executable edits roll back when writing fails.
- Wall-clock changes could affect timed transitions. Duration and queue-gap deadlines now use a monotonic clock.
- Corrupt JSON shapes, malformed artwork metadata and invalid executable names could break rendering or startup. Data is validated and Windows reserved names and unsafe paths are rejected.
- Artwork requests previously created unbounded workers. Downloads now use four workers, coalesce duplicate URLs and replace cached files atomically.
- Missing executable metadata could produce an activity session without a native game process. A public catalog lookup is attempted; an unresolved executable produces an actionable error instead of a false running state.
- Refreshing a feed could remove the Stop control for an already running quest. Its card now remains available until the session ends. Quest variants are matched when displaying active and queued state.
- Game pagination recreated existing rows. Additional pages now append without rebuilding the preceding list.
- The native bridge, navigation restrictions and content security policy have been tightened; artwork listeners no longer require inline script handlers.

## Interface reference

Adapted the MIT-licensed Discord Quest Helper 0.10.9 component styling: light/dark palette, typography, card spacing and corners, 600-pixel game picker, 40-pixel inputs, executable menu and selection checkmarks, segmented controls, navigation and theme transition. The supplied license remains bundled. Questro's queue and quest controls retain the user's requested behavior and omissions.

## Verification

22 native regression tests pass, covering timing, cancellation, queue edits, save errors, path confinement, helper ownership, feed filtering, IPC framing and bounded downloads. English/localization checks pass. The actual WebView2 regression results and screenshots are recorded in TAURI_UI_VERIFICATION.json and the accompanying PNG files.

The native UI test runs an isolated profile with Discord presence disabled. It verifies native process ownership and window behavior without accessing the user's account.

## Practical limits

- A helper process is Questro's code, not the actual game. Earlier live testing verified Counter-Strike 2 detection; it does not establish detection for every catalog entry or Discord quest credit. RPC acknowledgement alone is not proof of either.
- Quests come from a public community feed. Availability, upstream changes and account eligibility cannot be guaranteed. Local completion and Reopen affect Questro's state only.
- WebView2 is an installed Windows dependency. The executable size excludes its shared runtime and subsequently downloaded application data.
- The portable directory must be writable. Saved preferences and history are preserved during the final move; build artifacts, old programs, temporary files and regenerable artwork caches are excluded.

## Follow-up opportunities

Signed releases and an explicit update mechanism would improve distribution. Broader live detection testing would establish which catalog games support Questro's helper approach. Neither is represented as already implemented or verified.
