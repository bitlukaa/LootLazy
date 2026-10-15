# Integrated Steam accounts: source and implementation review

## Current status

ASF 6.3.10.6 is preserved as editable source under `third_party/ArchiSteamFarm`,
including its original license and notices. It is pinned to
`c156c7ba6a2ebbfce5bdf70d9f1a69ac77e23ee9` rather than upstream main.
No account feature or engine has been shipped. No real Steam credentials have been
requested, read, or used. The existing desktop-client idling remains unchanged.

## Product placement

Within Idle Steam, the tab order will be **Steam Library / Steam Accounts / Active sessions**.
`public/index.html` holds the sub-navigation, and `setSteamSubMode()` in
`public/js/app.js` currently accepts only `library` and `active`; add `accounts`
with its own view rather than changing the library's existing behavior.
English and Arabic labels belong in `public/js/i18n.js`.

Steam Accounts will list accounts by a user-chosen label and show Signed out,
Signing in, Needs Steam Guard, Ready, Running, or Playing elsewhere. Each account
has Add, Sign in, Select games, Start, Stop, and Remove as applicable. Game selection
includes search; use text rows and existing compact controls. Show a separate error
per account. Passwords and codes must never be rendered back from status responses.
Removing an account disconnects it and removes its local authentication data.

Steam desktop Running/Not running remains a desktop-client status; network account
states must not imply that the Steam desktop program is running. The desktop client
is unnecessary for network accounts. Existing Active sessions gains an account label
for network sessions, with IDs namespaced away from desktop sessions.

## Build and runtime decision

ASF targets .NET 10 and uses SteamKit2; LootLazy uses Rust/Tauri. ASF cannot be copied
into Rust and run directly. The least disruptive implementation is one internal,
headless engine process built from this source and started by LootLazy on demand.
The user installs only LootLazy, uses only LootLazy's window, and does not install or
configure ASF separately. This still creates an additional operating-system process;
it must not be described as a single-process implementation.

An absolute single-process requirement would instead need native .NET hosting and
explicit initialization/shutdown changes, or a substantial port. That is a separate
engineering decision, not a rename or build switch. Do not promise it is equivalent
to the internal-engine design.

The engine must be optional at runtime: no account engine for users who only use
Game Time, Game Variety, Quests, or the existing Steam library. Build a self-contained
Windows engine so users do not have to install a .NET SDK/runtime. Measure published
package sizes during the build, not runtime RAM/CPU benchmarks. The upstream Windows
archive is about 20 MB compressed; this is an upstream artifact size, not a prediction
of LootLazy's eventual package size or memory consumption.

## Source interfaces reviewed

| Concern | Existing ASF source | Integration consequence |
| --- | --- | --- |
| Initialization | `ArchiSteamFarm/Program.cs`: private `Init`, private `Main` | Add a reviewed engine entry point; it is not currently an embeddable SDK. |
| Account registration | `Steam/Bot.cs`: internal `RegisterBot`, `Start`, `Stop` | An adapter in the same engine assembly can call these without reflection. |
| Login prompts | `Steam/Bot.cs`: `RequiredInput`, `SetUserInput`; `Core/ASF.cs`: `EUserInputType` | Support password, email code, authenticator code, device confirmation, retries and expired sessions. |
| Session state | `Steam/Bot.cs`: `IsConnectedAndLoggedOn`, `IsPlayingPossible`, `PlayingBlocked` | Distinguish connection from successful idling and pause when the same account plays elsewhere. |
| Game idling | `Steam/Storage/BotConfig.cs`: `GamesPlayedWhileIdle`; `Bot.cs`: `ResetGamesPlayed` | Configure owned App IDs; do not launch game files or require a desktop client. |
| Account safety | `Steam/Storage/BotConfig.cs`: farming, remote communication, trading settings | Explicitly disable excluded ASF features; no card-farming UI or background card runs. |
| Tokens | `Steam/Storage/BotDatabase.cs`, `Bot.cs`: `UpdateTokens`, `Helpers/ArchiCryptoHelper.cs` | Defaults can store tokens as plain text; secure persistence is a prerequisite, not an optional setting. |
| Desktop app integration | `src-tauri/src/main.rs`, `backend.rs`: API routes, update busy checks and shutdown | Add account engine lifecycle and include its sessions in update/shutdown decisions. |

## Adapter boundary

Use inherited anonymous stdin/stdout pipes between the Rust parent and the owned engine
child. No exposed HTTP control server or web dashboard. Use correlated JSON messages
with a protocol version, bounded message length, request timeout, and a strict set of
commands: status, add_account, sign_in, submit_input, list_games, set_games, start,
stop, and remove_account. Use generated internal account IDs, not user-supplied file
paths or ASF bot selectors. Do not forward arbitrary ASF commands.

Responses expose only label, account ID, Steam ID (as a string), connection state,
required input kind, chosen game IDs, running/blocked state, and sanitized error codes.
Do not serialize a Bot/BotConfig/BotDatabase object directly. Pipe errors must not
include request bodies, credentials, tokens, or raw exception details in UI or logs.

On EOF or parent exit, stop the owned engine. On app Exit, stop every network session.
Use an owned Windows job object or equivalent parent-lifetime guarantee, so a failed
UI cannot leave silent idling behind. A stopped engine can restart explicitly;
never automatically replay Start after a crash. Engine initialization must be lazy.

## Required restrictions and persistence changes

* Disable ASF's IPC HTTP server, standalone web UI, remote chat commands, plugin loading,
  plugin updates, engine auto-updates/restarts, telemetry/remote communication,
  automatic gifts, trading, key redemption, and card farming. Do not rely only on
  paused farming: deny unrelated operations in the adapter and keep priority farming
  empty. Selected-game idling is the only enabled automation.
* Preserve ASF/SteamKit connection limiting and reconnect behavior; do not run parallel
  login retries that bypass them. Map account-in-use to a visible waiting state.
* Store engine data only beneath LootLazy's application data directory. It must be
  included in full uninstall deletion, retained during updates, and excluded from Git.
* Passwords and Steam Guard codes remain in memory and are cleared from forms after
  submission. Never pass them in process arguments, environment variables, log files,
  or plaintext account JSON. Do not request them in chat.
* Windows DPAPI CurrentUser must protect persisted refresh/access tokens and other
  reusable login secrets, including SteamGuardData. `PasswordFormat=ProtectedDataForCurrentUser`
  is not by itself enough: `SetUserInput(Password, ...)` resets the format to PlainText,
  and the upstream guard-data path also needs review/patching. Add round-trip and
  plaintext-absence tests before enabling login.
* No remote key custody. Explain that copying a DPAPI-protected data directory to a
  different Windows user/install does not guarantee that login can be restored.
* Keep upstream copyright and Apache-2.0 notices. Include engine/dependency notices in
  the distributed notices file when the engine is actually bundled. The original
  author's work does not become exclusive LootLazy authorship.

## Packaging and updates

`tools/build-tauri.ps1`, `tools/package-tauri.ps1`, and `src-tauri/tauri.setup.conf.json`
currently ship the app plus documents. The in-app updater replaces only the signed app
executable. Adding a loose engine binary without changing that updater would leave
users with missing or stale engines.

Prefer one signed app payload containing a versioned, hashed engine archive, safely
extracted into the app data directory on first use. Protect against traversal/reparse
points, atomically replace completed extraction, and never execute a user-selected
path. Verify the engine hash before launch. Pin an engine protocol version and refuse
mismatches. Keep account data separate from engine extraction. Do not silently download
and execute the newest upstream engine at runtime.

## Validation and release gates

The source-import check verifies all 500 blobs against the upstream commit. A separate
Windows CI job builds the copied engine project from source with .NET 10; it does not
start ASF or authenticate accounts. This proves source preservation/buildability,
not the completed feature or compatibility with live Steam authentication.

Before publishing the feature, require adapter protocol tests, malformed/oversized
message tests, multi-account isolation, secret persistence round trips, no plaintext
secrets, Steam Guard failure/retry, playing-elsewhere handling, disconnect/reconnect,
Stop/Remove/Exit cleanup, update busy checks, portable and Setup packaging, extraction
integrity, English/Arabic UI regressions, and account-tab placement tests. Live login
needs a user-authorized test account entered locally, never a password posted in chat.
Until those pass, keep this work on a development branch and keep the public release
unchanged. Do not add a decorative accounts tab that cannot perform these operations.

## Build and test the implemented preview

Requirements: Windows x64, .NET SDK 10, Node.js 22, Rust/MSVC and the usual Tauri
Windows build prerequisites. Users of the built application do not need the SDK.

```powershell
npm ci --ignore-scripts
./tools/build-steam-engine.ps1
npm run test:accounts
npm run test:unit
npx playwright install chromium
npm run test:renderer
./tools/test-tauri.ps1
./tools/build-tauri.ps1
./tools/build-setup.ps1
```

The engine adapter lives in `engine/LootLazyBridge.cs`; the explicit patches are
in `tools/prepare-steam-engine.cjs`. Native ownership/extraction is implemented in
`src-tauri/src/steam_accounts.rs` and the UI in `public/js/steam-accounts.js`.
Engine data lives under `data/steam-accounts`; executable/runtime extraction is
under `data/steam-engine`. Both are covered by the full-data uninstall cleanup.

The source-built, self-contained engine is embedded in the application executable.
It is extracted only when the account tab is first opened. No separate ASF install,
web dashboard or unauthenticated localhost control port is used. One hidden owned
engine process manages all accounts. The same signed executable carries its engine
through normal app updates and portable/Setup packaging.

To modify SteamKit, unpack the preserved source archive and build the SteamKit2
NuGet project into a local package directory. Supply that directory during restore
(or configure a local NuGet source), then rebuild the engine and LootLazy. Rebuild
`LootLazy.exe` after changing the engine because its hash is embedded in the app.
There is no signing-key requirement for local development/relinking. Distributed
update signing remains separate from local builds.

Offline tests exercise actual DPAPI-protected database writes/reads and reject
plaintext database files; protocol fixtures exercise validation, account isolation,
persistence and shutdown; actual initialization fixtures register/remove disabled
bots without login. Browser fixtures exercise the UI and simulated Guard failures.
These do not establish real Steam sign-in success or server-credited game hours.
Do not advertise the preview as live-tested until those have been checked locally.

Account search now uses `ArchiHandler.GetOwnedGames` for the signed-in account,
not the public store catalog. Library loading runs in a bounded background task,
so a slow Steam response cannot block other account controls. Selection and Start
both validate ownership against that account’s authenticated library.

## Authentication and account lifecycle

One centered card presents credentials and the Steam mobile QR together. A new
account starts as an in-memory draft. Drafts are excluded from accounts.json and
from the visible saved list; commit requires an authenticated connection. Cancel
removes the draft and its generated config/database. Startup removes orphaned
generated configs left by a crash before commit. Previously saved profiles remain
by label and in insertion order when signed out; the games panel requires a
connected account. Row removal uses an explicit confirmation for the target ID.

Stop and timed completion clear played games and retain the authenticated
connection/library. Sign out is a separate disconnect command. Ready accounts
alone do not block an update; games, drafts and pending authentication do. Exit
still terminates the owned engine and every connection.

The bridge copies the exact Steam QR challenge payload without URI reserialization.
A per-attempt identity guards rotating challenge updates and late callbacks.
Connection scopes cancel credential polling, and a new attempt waits for previous
connection callbacks to finish before starting. Failed or cancelled QR logins do
not silently restart. New code explicitly ends the prior attempt and starts fresh.
QR data stays on the private pipe; the locally generated SVG uses black/white, a
four-module quiet zone, integer device-pixel sizing and an explicit LTR direction.
No image service receives the challenge and no QR URL is persisted in metadata.

Offline validation includes protected database persistence, drafts/abandoned
config cleanup, account protocol/startup, connection cancellation and the generated
QR fixture. The renderer decodes actual screenshots against that exact fixture in
English/light and Arabic/dark at 125% scaling, and checks cancellation, labels,
Guard retry, Stop versus Sign out, and removal of a non-selected account.

The user confirmed password sign-in and a game session on 3.7.2. Anonymous live
Steam challenge generation/refresh can be tested in an isolated profile without
approving or signing into an account. Mobile QR approval and credited playtime
are not established by those tests and still need user verification.
