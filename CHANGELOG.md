# Release history

## 3.7.1 — Beta update channel

- Opt into Beta from Settings > Updates; new features arrive here first.
- Return to the signed stable release, including a lower version, while keeping settings and account data.
- Steam Accounts remains a beta feature. Live account sign-in still requires real-world testing.

## 3.7.0 — Steam Accounts preview

- Added Steam Accounts between Steam Library and Active sessions.
- Manage separate Steam sign-ins, Steam Guard prompts, selected games, and timed or continuous sessions.
- Account sessions use a source-built internal engine; the desktop Steam client is optional.
- Protect saved account authentication data with Windows DPAPI; passwords are not saved.
- Stop idling when an account disconnects or plays elsewhere. No automatic session start after reopening.
- Keep card farming, remote commands, plugin loading, and engine self-updates disabled.
- Embed the engine in the signed application payload so Setup, portable downloads, and app updates carry the same version.
- Preview validation uses synthetic accounts and authentication fixtures; live Steam sign-in and credited playtime still require local user testing before a stable promotion.


## 3.6.22 — October 15, 2026

- Clean portable data and generated helpers when application data deletion is selected during uninstall.
- Select data deletion by default while honoring explicit retention and update mode.
- Preserve unrelated files and junction targets, handle long WebView cache paths, and report files that remain locked.
- Test cleanup, retention, updates, junctions, readonly and locked files, repeated runs, spaces and large directory trees.
- Run the new Setup over an existing installation once to refresh its uninstaller.

## 3.6.21 — October 15, 2026

- Default Game Time to Continuous in both the initial interface and session state.
- Verify fresh launches and reopening after selecting a timed duration.

## 3.6.20 — October 15, 2026

- Avoid rewriting unchanged quest DOM attributes and labels during polling; verify idle button pixels across automatic status checks.

- Recognize native string errors as well as Error objects during update installation.
- Explain that all active sessions must be stopped before installing an update.

## 3.6.19 — October 15, 2026

- Override the shared pressed-button transform for quest actions in the final stylesheet.
- Preserve closed quest text nodes during unchanged status polls.
- Report initial quest loading failures instead of presenting a successful empty feed.
- Preserve the active quest Pause icon during status updates.
- Exercise pressed and polled quest controls in both languages and themes.

## 3.6.18 — October 14, 2026

- Include Discord: and Steam: status labels in the downloadable application.
- Keep quest action buttons still during hover and repeated status updates.
- Match the screenshot gallery to the navigation order and show real desktop quest artwork.
- Rotate the update signing key after loss of its backup; install 3.6.18 manually once before using future in-app updates.

## 3.6.10 — October 13, 2026

- Use LootLazy build paths, interface names and launchers exclusively.
- Correct the updater helper executable-name check and exercise replacement with LootLazy.exe.
- Copy legacy browser preferences to renamed keys and read legacy settings files when new files are absent.
- Migrate the protected signing key to LootLazySigning without rotating it; retain the original as a backup.

## 3.6.9 — October 12, 2026

- Rename application, native window, tray controls, package metadata and distribution assets to LootLazy.
- Accept the renamed release repository and legacy asset names while preserving signature verification.
- Mark remaining-card tracking as an inactive preview and display unknown counts without implying zero.
- Refresh the four interface screenshots and publication instructions.

## 3.6.8 — October 12, 2026

- Confine helper writes to the games subtree, including dot and reserved game names.
- Require detached Ed25519 update signatures bound to the version, binary hash and size.
- Verify Valve publisher signatures on Steam runtimes and hold read-only file locks during validation/loading.
- Rebuild the session helper from C# source, pin frontend test dependencies, and include frontend tests in CI.
- Filter replenished Game Variety pools to Discord artwork and expand regression coverage.
- Lower WebView2 memory pressure while hidden in the tray and restore normal mode when shown.

## 3.6.7 — Included in 3.6.8

- Start Game Variety with 10 games and explicitly save or cancel custom counts; saved custom presets can be removed.
- Remove next-action glow rings and keep completed quest buttons neutral during polling and card recreation.
- Refresh the games catalog every 24 hours, with a 15-minute cooldown for manual refresh. Quests refresh every 5 minutes, with a 1-minute manual cooldown.
- Refresh the Game Variety screenshot, remove unused guidance state, and expand regression coverage.

## 3.6.6 — October 12, 2026

- Game Variety starts empty until a game count or Shuffle is selected.
- Unstarted saved lineups are cleared on restart; paused sessions with playtime retain progress.
- Updated the crypto donation address and explained eligible Steam Trading Card drops in the README.
- Refreshed the screenshot gallery with directly viewable PNG captures and ready quest buttons.

## 3.6.5 — October 12, 2026

- Removed the unused Buy Me a Coffee option and centered the crypto support card.

## 3.6.4 — October 12, 2026

- Refined support icons and removed duplicate branding.
- Added crypto support with explicit currency/network choices and address copying.
- Refreshed the README around benefits, downloads, manual controls, and community support.

## 3.6.3 — October 12, 2026

- Replaced custom graphics with published Lucide and Simple Icons assets.
- Added page-specific wording for game sessions, game lineups, Steam, preferences, and support.

## 3.6.2 — October 12, 2026

- Centered the navigation and moved compact Discord/Steam statuses into the titlebar.
- Matched the Steam icon to the app palette.
- Refined empty session states, game rows, button interactions, and light-theme colors.

## 3.6.1 — October 12, 2026

- Added matching icons to all main navigation tabs.
- Optimized the Steam navigation image while keeping it clear at small sizes.

## 3.6.0 — October 12, 2026

- Added Setup and portable download options.
- Added in-app update downloads, verification, installation, and restart.
- Simplified the Updates panel and clarified the Discord activity setting.

## 3.5.4 — October 12, 2026

- Removed Sound Alerts and its preview control.
- Kept visual and desktop completion notifications.

## 3.5.3 — October 12, 2026

- Fixed the Settings layout and placed the sound preview alongside Sound Alerts.
- Added optional Discord sync from Idle Steam for matched games.
- Improved paired session timers and completion alerts.
- Added brief Game Time and Game Variety badge guidance.
- Added a Steam search tip for owned games that are not installed.

## 3.5.2 — October 12, 2026

- Added Game Variety with stable list resizing, shuffled selections, search, and reordering.
- Added independent Steam sessions and optional Steam sync for supported Game Time titles.
- Added saved queue recovery with pause, resume, and clear controls.
- Added 5-, 10-, 15-, and 20-minute Game Variety presets and custom hours and minutes.
- Simplified quest controls and improved the unfinished-session resume action.
- Added the Donation page to the main navigation; payment options are coming later.
- Improved completion sounds and notifications, including a sound preview in Settings.
- Refined connection badges, layouts, and Arabic support.
