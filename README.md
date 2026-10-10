# Questro 3.1 — Tauri

Portable Windows desktop application with a public game catalog, owned game-process helpers, local timing and public desktop-play quest cards.

## Run

Close the previous Questro instance completely and open Questro.exe in the project root. The launcher opens dist/Questro-Tauri/Questro.exe. Copy the Questro-Tauri folder to distribute the application. The optimized executable is approximately 3.81 MiB; catalog, artwork and profile caches are additional user data.

The native runtime is Tauri and Rust with the installed Microsoft Edge WebView2 runtime. Electron, Chromium and Node are not bundled. WebView2 must be available on the destination machine: https://developer.microsoft.com/microsoft-edge/webview2/

Games, JSON data, artwork, WebView2 profile/cache and temporary files stay inside the writable application folder. Saved JSON data and artwork have been consolidated into the Tauri portable folder. Rebuilds never replace user data. New language and theme choices persist inside the portable profile.

## Simulator and quests

Select a catalog game before configuring it. Known Windows executable names are selected automatically. The executable dropdown has selection checkmarks, keyboard navigation and animated opening.

Start begins a timed game. Choosing another game changes the action to Play next and preserves each game's duration. Queue becomes available for at least two unfinished games. Upcoming entries can be reordered, removed and edited in hours/minutes. Following games start after a random 5–10 second gap. Stop on the current game removes it while retaining following entries. Stop in Queue cancels the sequence. Continuous sessions cannot have a timed successor.

Custom Process accepts a relative Windows executable filename and starts Questro's helper without inventing a Discord RPC application ID. Missing executable metadata triggers a public catalog lookup; if no Windows executable is available, starting is rejected rather than reporting a game as running. The helper is Questro's code, not the real game's code.

Only 900-second PLAY_ON_DESKTOP quests appear. Video, Activity and mixed Watch/Activity tasks are excluded. The anonymous community feed https://api.discordquest.com/api/quests refreshes every five minutes while open. It provides neither account eligibility nor account progress.

Start Quest runs from the card; further quests wait. Active cards remain first with a neutral countdown and red Stop. Local runtime is 930 seconds; the requirement text remains 15 minutes. Cancelled and locally finished cards move below unfinished cards. Reopen returns an available campaign to the unfinished group. New campaigns receive a persistent New badge until started or cancelled. Local completion never claims verified Discord credit.

## Native runtime

The renderer calls Rust through Tauri commands. There is no local HTTP server. Artwork uses a restricted custom protocol and portable cache. Timers run independently of renderer polling. Closing Questro stops its owned helper and cancels pending transitions.

The helper keeps a normal minimized window owned by a hidden host, without an independent taskbar entry. Earlier live testing confirmed Counter-Strike 2 detection in Discord Registered Games. The native 3.x test confirms minimized helper windows, zero helper taskbar entries, Play next, Stop and quest cancellation/reopen. This does not verify every catalog game or account quest credit.

Local Discord named-pipe IPC supports READY, framed SET_ACTIVITY acknowledgements, ping handling, reconnect and cancellation. No user tokens, client injection, private quest heartbeats or automatic enrollment are used. Protocol: https://docs.discord.com/developers/topics/rpc

## Build and tests

Install Rust's MSVC toolchain, Microsoft C++ Build Tools and WebView2: https://v2.tauri.app/start/prerequisites/

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/test-tauri.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File tools/build-tauri.ps1
```

npm run build, npm start and npm test wrap the native scripts. npm is optional for ordinary use and native builds. npm run test:ui requires Playwright; set QUESTRA_PLAYWRIGHT to its module directory if needed. The UI test uses an isolated hidden WebView2 profile and does not access the user's Discord account.

src-tauri contains the maintained native backend. Only the Tauri portable build is kept locally. Retired Electron code, binaries and build caches have been removed. The 2.3.1 baseline remains recoverable from GitHub history; tauri-migration contains 3.x.

The public catalog https://discord.com/api/applications/detectable contains game metadata, not installed or downloaded games. Catalog refresh is manual; startup fetches missing/invalid caches. Failed public API requests preserve saved data.

## Attribution

The UI adapts layout, palette and theme transitions from Discord Quest Helper 0.10.9 under MIT. Its license and the Lucide icon license are bundled in the portable folder and public/assets. See docs/THIRD_PARTY_NOTICES.md.

## Clean distribution

Run npm run package after building. It creates releases/Questro-3.1.0 and its ZIP, containing only the tested executable, a short README and license notices. Source code, game catalog, artwork, profiles and personal data are excluded. Use a new -OutputName when packaging the same version again; existing folders are preserved. The executable SHA256 is checked against the tested build.
