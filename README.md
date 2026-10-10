# Questro 2.3.1

Portable Windows game-process simulator with a public game catalog, local timing and public desktop-play quest cards.

## Run

Close the old instance completely and open Questro.exe in the project root. Keep dist/Questro together; it is the maintained portable build. Games, history, catalog, artwork, quest states, Electron profile/cache, logs, crash dumps and temporary files stay inside the application folder. Builds preserve saved data.

## Simulator

Select from List: choose a game and a duration, then Start. If another timed game is running, selecting a different game changes the same action to Play next. Each game's chosen time is stored in its entry. There is no Add to queue button.

Queue: view the running and upcoming games. Upcoming entries can be reordered, removed and edited in hours/minutes without changing the current game. The next starts automatically after a random 5–10 second gap. Stop on the running game's configuration stops that game and advances after the gap; Stop in Queue stops the sequence. Continuous sessions are standalone and require a manual Stop. Saved entries never resume automatically after reopening.

Custom Process: enter a relative Windows executable name and start the owned helper with that name. No application ID is invented for Discord RPC. Mode switching preserves the custom filename. File controls are visible without an Advanced settings accordion. A refresh icon beside Select Game updates the catalog and spins during loading; the mouse cursor does not turn into a busy spinner.

## Native detection

The former fully hidden window prevented the tested detection path. The runner now retains a normal minimized window owned by a hidden host, without an independent taskbar entry or a normal foreground game window. A user-authorized live comparison displayed Counter-Strike 2 - Now playing! in Discord Registered Games with RPC disabled. The reference application also displayed Overwatch - Now playing! and retained a minimized window. See docs/QUESTRO_NATIVE_DETECTION.json and docs/RUNNER_WINDOW_VERIFICATION.json.

This verifies native detection for the tested game, not every catalog entry or account quest credit. Known Windows filenames are selected automatically. Missing executable metadata can still use activity-only RPC, but that is not native process detection. The helper is Questro's code, not the real game's code.

Local Discord IPC remains available for activity publication. It uses READY and matching SET_ACTIVITY replies, ping handling, reconnect and cancellation; it does not use user tokens, client injection, private quest heartbeats or automatic enrollment. RPC acknowledgement messages are not shown in the UI. Protocol: https://docs.discord.com/developers/topics/rpc

## Quests

Only 900-second PLAY_ON_DESKTOP tasks are displayed; video, activity and mixed Watch/Activity tasks are excluded. The public community feed https://api.discordquest.com/api/quests refreshes every five minutes while open. Account eligibility and progress are not available from that feed.

Start Quest runs from its card. Further quests wait their turn. Active quests remain first with a neutral countdown and red Stop inside the card. Local runtime is 930 seconds, while the requirement text remains 15 minutes. Cancelled and locally finished cards move below unfinished cards; Reopen returns an available campaign to the unfinished group. These local states survive refresh and restart. Local completion never claims verified Discord completion.

## UI and catalog

The reference's Blurple palette, compact navbar/cards and circular theme reveal are adapted with MIT attribution. The header is outside the scrolling region and remains stationary. The scrollbar is at the application edge. English and Arabic are supported, with one artwork image per slot and a neutral fallback.

The catalog is public metadata from https://discord.com/api/applications/detectable, not installed or downloaded games. Catalog refresh is manual with a valid saved cache; startup downloads only missing/invalid caches. Settings keeps the green catalog count and update time beneath Refresh.

## Licenses and development

Reference UI adaptations: Discord Quest Helper 0.10.9, copyright 2025 Masterain, MIT License, bundled as public/assets/DISCORD-QUEST-HELPER-LICENSE.txt. Lucide logo license: public/assets/LUCIDE-LICENSE.txt. The supplied reference source folder is preserved.

npm start opens the portable build; npm run build reuses the existing Electron runtime. npm test checks backend, scheduling, IPC, filtering, paths and localization. tools/verify-ui-v2.js tests the packaged interface using Playwright and isolated data/pipes. tools/verify-runner-window.js checks minimized ownership and taskbar eligibility. tools/verify-portable-storage.js checks actual Electron paths without opening a window. Reports and previews are in docs.

The Queue tab is enabled for at least two unfinished games. Stopping a simulator game removes its saved queue entry. The first-run configuration stays empty until a catalog game is selected. Executables use an in-app dropdown with keyboard navigation, selected checkmarks and animated opening. New public quests receive a persistent New badge until started or cancelled.

The current desktop shell is Electron (bundled Chromium and Node); the reference uses Tauri and system WebView2. Comparable small binaries would require a desktop-shell migration.
