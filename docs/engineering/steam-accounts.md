# Integrated Steam accounts: Architecture and Implementation

## Current status

LootLazy integrates an independent, proprietary Steam micro-client engine (`LootLazy.Steam`), written in C# on .NET 9 using `SteamKit2`. All legacy third-party idler code has been completely replaced. The engine is self-contained, trimmed, and compiled into a single executable (~14.6 MB) that is launched on demand by the Tauri Rust backend over an anonymous bidirectional pipe with strict protocol framing.

## Product placement

Within Idle Steam, the tab order is **Steam Library / Steam Accounts / Active sessions**.
`public/index.html` holds the sub-navigation, and `setSteamSubMode()` in
`public/js/app.js` manages switching between `library`, `accounts`, and `active` sessions.

Steam Accounts lists accounts by a user-chosen label and shows Signed out,
Signing in, Needs Steam Guard, Ready, Running, or Playing elsewhere. Each account
has Add, Sign in, Select games, Start, Stop, and Remove as applicable. Game selection
includes search; using text rows and existing compact controls. Each account displays
its separate error status. Passwords and codes are never echoed back in status responses.
Removing an account disconnects it and removes its local authentication data.

Steam desktop Running/Not running remains a desktop-client status; network account
states do not imply that the Steam desktop program is running. The desktop client
is unnecessary for network accounts. Active sessions displays the account label
for network sessions, with IDs namespaced away from desktop sessions.

## Build and runtime architecture

LootLazy uses Rust/Tauri for the main application and desktop windowing. The Multi-Account
idle engine is implemented as `engine/Program.cs` (`LootLazy.Steam.csproj`) targeting `net9.0`.
It provides direct Steam network idling using `SteamKit2` and local DPAPI encrypted storage.

Key architecture points:
1. **Direct Protocol**: Uses `SteamKit2.SteamClient`, `SteamUser`, and `SteamApps` directly.
2. **IPC Bridge**: Standard input/output anonymous pipe using single-line JSON commands (`status`, `add`, `remove`, `games`, `signin`, `input`, `start`, `stop`, `library`, `qr`, `commit`, `disconnect`, `presence`, `resume`, `rename`).
3. **Encrypted Storage**: Refresh tokens and account secrets are encrypted with Windows DPAPI (`DataProtectionScope.CurrentUser`) inside `config/<account-id>.db`. Plaintext passwords and tokens are never stored on disk.
4. **Self-Contained & Trimmed**: The engine is compiled as a self-contained trimmed single file (`PublishSingleFile=true`, `PublishTrimmed=true`, `_DefaultValueAttributeSupport=true`), eliminating external runtime dependencies.
5. **No Desktop Steam Dependency**: Runs headlessly in the background without needing the Steam client or hooks.

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
terminates the owned engine process and every active connection.

The engine generates the exact Steam QR challenge payload without URI reserialization.
A per-attempt identity guards rotating challenge updates and late callbacks.
Connection scopes cancel credential polling, and a new attempt waits for previous
connection callbacks to finish before starting. Failed or cancelled QR logins do
not silently restart. New code explicitly ends the prior attempt and starts fresh.
QR data stays on the private pipe; the locally generated SVG uses black/white, a
four-module quiet zone, integer device-pixel sizing and an explicit LTR direction.
No image service receives the challenge and no QR URL is persisted in metadata.

Offline validation includes protected database persistence, drafts/abandoned
config cleanup, account protocol/startup, connection cancellation and the generated
QR fixture. The renderer validates screenshots against that exact fixture, and
checks cancellation, labels, Guard retry, Stop versus Sign out, and account removal.
