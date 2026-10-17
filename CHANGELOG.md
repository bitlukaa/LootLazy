# Release history

## 3.11.1

- Fix QR authentication configuration that immediately rejected mobile confirmation; verify creation and refresh against Steam without approving an account.
- Prevent stale QR failures from appearing in password/Guard sign-in; clear prior Guard errors when submitting a new code.
- Bound authentication polling by cancellation and distinguish timeout/unexpected failures from transport errors. Add bounded local diagnostics containing stage, exception type and Steam result only.
- Read generated dependency notices as UTF-8 on Windows PowerShell.


## 3.11.0

- Promote the standalone SteamKit2 account engine and English interface to Stable.
- Restore games after temporary transport loss without counting offline time; Stop cancels pending recovery.
- Pace logins across accounts, back off repeated connection failures, observe Steam sign-in throttling and bound saved-login connection attempts.
- Check saved-token expiry and shared-library lock state in addition to playing-session blocks.
- Harden Steam username visual privacy persistence; add keyboard reveal and visible storage errors. This is visual masking, not encryption.


## 3.10.1 Beta (unpublished)

- Repair standalone Steam credentials and QR login with connected, cancelable authentication and Steam Guard challenges.
- Persist refresh tokens and remembered Guard data transactionally; report write failures and retain previous saved credentials.
- Use unique logon identifiers and Steam playing-session callbacks to avoid competing with Steam Desktop, including the same game.
- Preserve explicit sign-out, recover unreadable saved-login profiles through reauthentication, and allow explicit retry after session conflicts.
- Keep this engine rewrite on a separate Beta branch; published Stable is unchanged.


## 3.10.0

- Promoted Beta 3.9.4 features to Stable: independent game achievement management, Steam tools with multi-account idling, and refined session controls.
- Achievement library cards open games individually without cross-game operations.
- New Game Time and Steam Idle sessions default to Continuous, with duration and optional sync under Advanced settings.
- Streamlined session setup with unified Executable dropdown, matching SVG chevrons, and full-width Start actions.
- Fresh account logons verify connection and playing-elsewhere state before starting activity.
- Preserves all saved accounts, encrypted credentials, settings, and update channel choices.

## 3.9.4 Beta

- Achievement library cards open games individually. Removed game checkboxes, Select all games, Clear selection and cross-game achievement operations.
- Fixed the duplicate Executable control by keeping its existing dedicated dropdown. Removed Custom from the game executable choices and aligned the label beside the control.
- Restored matching SVG dropdown chevrons and added clear hover/focus feedback to account Status options.
- Game Time and Steam Idle default new sessions to Continuous. Run duration and optional sync controls are inside collapsed Advanced settings.
- Shuffle games sits beside 10 / 50 / 100 / 200. Game Variety Start and Multi-Account Idle Start span their action area. Game Variety stops after the final game as requested.
- Fresh account logons wait briefly before starting play activity, then re-check the connection and playing-elsewhere state. This addresses a startup race; it does not prove the cause of an external Steam desktop sign-out.

## 3.9.3 Beta

- Achievements opens with a full-width game gallery using LootLazy cards, Steam artwork, search, refresh and game counts.
- Open a game on its own page inside LootLazy. Back to games restores the library search and scroll position; unsaved changes prompt before discarding.
- Achievement details include icons, descriptions, unlock times, filters, Unlock all / Lock all and reviewed Commit changes. Library bulk game selection remains available.
- Removed Risky badges, exclusion controls and native risk classification.
- Confirmed write batches, encrypted saved logins and the optional Beta channel remain unchanged.

## 3.9.2 Beta

- The All accounts Status control is a compact themed dropdown beside Start all and Stop all. Individual accounts show Set Status above their four options.
- Library pages append automatically while scrolling. All achievements are available in a scrolling list without Load more or Previous.
- Achievements adds game counts, Select all games, Exclude Risky, and reviewed bulk unlock/lock actions. Changes use bounded batches and retain unfinished work after failures. Unmarked games are not guaranteed safe.
- Achievement icons load through Steam with bounded thumbnails. Games without available icons keep a fallback. Older runtimes use the local-user stats request; unavailable stats and denied access have clearer messages.
- All select popups use the application theme, with keyboard navigation and English/Arabic layouts.
- Steam-reported session conflicts now pause automatic account reconnection while keeping the saved login. This prevents reconnection contention; it does not establish the source of an external Steam sign-out.

## 3.9.1 Beta

- Fixed achievement loading with modern Steam runtimes and the native callback layout. Confirmed read-only loading with CS2 and Wallpaper Engine on a live Steam client.
- Achievements now reads owned games from the current desktop account and Steam library metadata instead of showing only installed games. Each game has a small thumbnail. Open Steam Library and refresh if its metadata is not synced yet.
- Risky badges identify available multiplayer or anti-cheat metadata. Use at your own risk; unmarked games are not guaranteed safe.
- Restored Steam Accounts labels and navigation icons. Set Status now uses the four themed buttons for all accounts.
- Multi-account library lists show three rows with vertical scrolling, without horizontal wrapping or duplicate add icons.
- Steam artwork uses cached headers and Steam metadata fallbacks. Discord service tabs now match Steam tab selection and inactive icon colors.
- Fixed native diagnostic wrappers hiding the actual achievement and sign-in errors.

## 3.9.0 Beta

- Steam Tools now contains Steam Idle, Multi-Account Idle and Achievements. Library and Active sessions remain inside their own service.
- Compact All accounts controls add Start all, Stop all and a shared Status selector. Account sessions always run continuously.
- Browse your account library or add games directly from Steam catalog search. Steam determines whether activity can be registered for each AppID.
- Account activity appears beside the label; edit labels using the pencil beside the name. Improved scrollbar spacing and clearer incorrect-password/Steam Guard/connection errors.
- Achievements Beta adds game selection, filters, staged unlock/relock changes and a review before saving. Requires Steam Desktop; game/runtime support varies.
- Removed Back to Settings from Donate and added a heart to the thank-you message.

- Native achievement bindings are verified with an isolated fixture DLL; no live achievement writes were performed during verification.

## 3.8.1 Beta

- Remove platform sign-in and Plans; place Free beside the app name and Donate beside the edge-aligned utility controls.
- Initialize each account library before Start/Start all validates ownership, independently of the selected account.
- Show cached Steam profile avatars, a separate game-activity indicator and a Sign out icon.
- Display Steam play-session locks inside accounts and on the reported game when known; preserve all saved selections and respect account-wide restrictions.
- Document the future Achievements service plan; no achievement changes are implemented in this release.

## 3.8.0 Beta

- Group Discord and Steam services in the header; move donations into Settings and add Plans.
- Edit Steam account labels, keep Connected badges stable, and start or stop all eligible Steam Accounts together.
- Search the Steam catalog while keeping play restricted to games owned by the selected account.
- Remove Connections & Privacy and add optional on-demand desktop app launching, OFF by default.
- Platform sign-in and paid plans are still being prepared; mandatory platform login is not enabled in this Beta.

## 3.7.9 Beta

- Required sign-in fields now show an asterisk. Account labels are required for both password and QR sign-in.
- Removed the redundant Cancel footer from the sign-in card; use the close icon or Escape.

- Simplified QR renewal to one Refresh QR code action on the blurred QR area. Removed the duplicate New code button and kept retry available after connection errors.


## 3.7.8 Beta

- Add or remove Steam Account games while running, without restarting the other games or the session deadline. New games get their own elapsed timer.
- Removing the last game ends only the game session and keeps the account signed in.
- Hardened game selection validation, failed-edit recovery and session-expiry handling.


## 3.7.7 Beta

- Added small game thumbnails to Steam Accounts and its active sessions, keeping game names, Running badges and timers on one line.


## 3.7.6 — Steam status clarity

- Rename Appear as to Status while keeping all four choices visible.
- Verify actual SteamKit status packets for Online, Away, Invisible and Offline in an isolated regression, without account authentication or a running game.
- Status applies to the Steam Accounts session. Other active Steam sessions can affect the presence friends see.

## 3.7.5 — Remember Steam sign-ins

- Restore remembered Steam logins after reopening or updating LootLazy, without starting games automatically. Sign out keeps the account saved and prevents automatic login.
- Finish saving the protected login session before confirming account sign-in. Passwords are not stored.
- Refresh an expired QR code from a soft overlay instead of an alarming error. Wrong-account and connection problems remain visible.
- Show all four presence options directly, and group selected games in one compact card with inline game status and timers. Remove redundant Selected labels and the saved-login password placeholder.
- Match Stable and Beta confirmation messages to their buttons and the following update step.

## 3.7.4 — Clearer Steam sessions

- Choose Online, Away, Invisible or Offline for each Steam account. Your choice is remembered.
- Keep Sign out in the account details, with Signed in and Running badges in the account list.
- See each selected game in its own card with running status and elapsed or remaining time.
- Use familiar duration buttons and a custom timer, starting with Continuous.
- Manage Steam Library and Steam Accounts sessions in separate cards with independent controls.

## 3.7.3 — Simpler Steam sign-in

- Use your Steam name and password or scan a QR code from the same centered card.
- Save a new account only after sign-in succeeds. Saved labels keep their order, with Sign in, Sign out and a confirmed remove action in the account list.
- Stop ends the game session while keeping the Steam account signed in.
- Refresh QR codes without overlapping old sign-in attempts, with clearer expiry and retry messages.
- Show Running and Beta in compact badges.

## 3.7.2 — Consistent navigation and dialogs

- Platform status and notifications now share the main navigation header.
- Stable and Beta confirmations use the same centered cards and visible close icons.
- Sign in with a Steam mobile QR code, with password and Guard available as an alternative.
- Add and sign in to an account from one dialog; passwords and Guard prompts no longer clutter the games page.
- Choose quick picks from the signed-in account’s library. New badges disappear after first successful use.

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
