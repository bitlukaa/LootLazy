# Questro 2.3.0

- Verified native Counter-Strike 2 detection in Discord Registered Games, with RPC disabled.
- Retain a minimized owned runner window without an independent taskbar entry.
- Replace Add to queue with a single Start / Play next action and Selected Games mode.
- Add Custom Process mode and always-visible executable controls.
- Edit future durations/order while the current game continues; Stop current advances after the normal gap.
- Add animated catalog refresh beside the game picker; keep the mouse cursor normal.

# Questro 2.2.0

- Add local Discord RPC activity publication and acknowledgement status, the missing path found in the supplied reference.
- Live Discord accepted Counter-Strike 2 and Deadzone Rogue 2 activities; quest credit remains unverified.
- Enable activity-only sessions when no game executable is listed, without generating guessed filenames or launching helpers.
- Adapt the reference navbar, Blurple palette, compact quest cards, artwork fades, transitions and maximize/restore icon.
- Preserve hidden helper behavior, per-quest queue, cancellation, reopening and local completion semantics.
- 32 backend tests and exact portable UI regression pass. Reference MIT attribution is included.

# Questro 2.1.0

- Start Quest directly on cards; append further quests without interrupting the current session.
- Separate quest queue with a 5–10 second interval, deduplication and persistent stop control while waiting.
- Only 900-second desktop play quests; exclude video and activity tasks. Five-minute public refresh.
- 930-second local runtime with a visible buffer and truthful local completion status.
- Native minimized helper window replaces invisible frameless helper; Discord detection remains unverified.
- Sidebar, SVG window controls, stable game selection and removal of category and unused settings.
- 24 backend tests and packaged UI regression pass.

# Questro 1.1.0

- Standardized the visible product name to Questro without a simulator subtitle.
- Unified SVG, PNG, executable, tray, taskbar, and shortcut branding using Lucide gamepad-2 in the application palette; retained the ISC license.
- Added a searchable quest shortcut form with duration and optional expiry. Bulk JSON import remains an advanced option.
- Added a custom Steam installation setting and installed-game scan on the home page.
- Added an offline artwork cache with a 64 MB disk limit, source validation, and four concurrent downloads.
- Cached resolver JSON parsing and installed executable scans; reduced idle/background status polling.
- Preserved English and Arabic localization, selected executable, and unsaved input while switching languages.
- Kept one maintained portable application; removed legacy builds, obsolete patches, old logo assets, test outputs, and redundant development runtimes.
- Preserved source files, test cases, build tools, and user data. Development dependencies can be restored with npm ci.

Verification: nine automated tests passed. Packaged language/form regression, native process lifecycle and preload verification, and actual packaged startup/title checks passed. Details are in VERIFICATION.json.

Discord game detection and quest progress still need to be checked in Discord; no live quest feed is connected. Removing duplicate files saves disk space and does not by itself reduce the runtime memory of Electron.

# Questro 1.2.0

- Added a persistent, ordered queue of up to 30 games, each with its own executable and timed duration.
- Waits 5–10 random whole seconds after a process stops before starting the next entry.
- Shows current entry, per-entry state, wait countdown, ordering, duration editing, removal, clear, start, and stop controls.
- Quest shortcuts can be added directly to the queue.
- Cancellation invalidates pending transitions, including during process startup; early exits halt the queue.
- Blocks manual starts and editing while queued execution is active.
- Application shutdown clears queue transitions and owned running or starting processes. Reopening restores the draft without automatic execution.
- Build now reuses the existing portable Electron runtime, preserving the cleaned project layout.

# Questro 1.3.0

- Simplified home instructions and Discord status; removed implementation-only connection descriptions.
- Collapsed executable customization and process details under Advanced settings.
- Added an explicit Open Discord action without account access.
- Added user-triggered Steam launches for detected installed catalog games; launch URIs contain only the verified Steam application ID. Real game processes are not managed or stopped by the simulator.
- Manual next-game progression is now the default. Retained local automatic progression as an explicit optional checkbox; Discord enrollment and account actions remain manual.
- Reviewed official OAuth2 and Social SDK documentation: no public quest-list scope or generic quest-enrollment event was found. Did not add an identity-only login that would misrepresent account-quest access.
- Eighteen tests passed, including manual queue handoff and restricted Steam URI resolution. Simplified packaged UI and both languages passed browser verification. External Steam/Discord opens were mocked for UI checks; no account access or real game launch was performed during testing.

# Questro 2.0.0 — interface rebuild

- Replaced the layered legacy stylesheet and controller with one coherent layout and palette.
- Open directly to Simulator; removed home, duplicated libraries, Discord status, account/link buttons and quest shortcut forms.
- Put the queue in a collapsible panel, preserving queue ordering and hour/minute duration editing.
- Added real anonymous quest refresh and clear through the documented Discord Quest Community endpoint. Preserves cache on error, excludes unavailable date ranges and consolidates equivalent variants.
- Fixed image fallback layering; successful images and neutral placeholders are mutually exclusive. Removed text initials and hardcoded image substitutions.
- Added separate custom Hours and Minutes fields and rejected invalid or zero custom durations.
- Restored and verified circular theme transitions and refresh-icon rotation.
- Catalog count is green, with last-updated text beneath the refresh button. Kept essential settings and collapsed advanced filename/path controls.
- Removed generated executable guesses. Missing Windows executable data is explicitly unresolved; community candidates are unverified, not automatically solved.
- Local process startup was verified. External Discord detection was not verified and is not claimed as fixed; the runner is a renamed local helper, not the actual game binary.
