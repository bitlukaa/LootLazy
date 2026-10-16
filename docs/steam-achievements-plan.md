# Steam Achievements — Beta implementation

Achievements is a separate service under Steam Tools, beside Steam Idle and Multi-Account Idle. It uses the account signed into Steam Desktop; the independent multi-account connections are not native UserStats sessions.

## Included

- Game selection, achievement names/descriptions, completion totals, locked/unlocked filters and search.
- Original Rust flat-API bindings inside an isolated process for each AppID. No external achievement application is launched or bundled.
- Individual unlock/relock changes staged locally, with a centered review before Apply.
- AppID/ownership checks, account identity checks and stale-state checks before writes.
- Compact backups of affected original values before applying changes, retaining at most twenty generated backups.
- StoreStats completion is required before reporting success. Rejected, timed-out or uncertain saves show an error and require refreshing; staged changes are never automatically replayed.

## Verification and limits

Native tests load an original fixture DLL and exercise real dynamic symbol resolution, asynchronous API-call results, achievement reading, unlock/relock, rejected StoreStats, rejected completion callbacks, changed accounts and stale state. Renderer tests cover review/cancel, failed saves, filtering, pagination and English/Arabic layouts. Live read-only verification also confirmed CS2 and Wallpaper Engine achievement loading and account-filtered library discovery. No real achievements were changed. Live compatibility varies by installed Steam runtime and game; server-controlled achievements may reject changes. Game thumbnails use Steam cache/metadata artwork; achievement icons currently use LootLazy's trophy icon. Risky flags come from available multiplayer/anti-cheat categories and do not certify unmarked games as safe. Batch selection, stat editing and multi-account achievement changes are not included.

## API references

- https://partner.steamgames.com/doc/api/ISteamUserStats
- https://partner.steamgames.com/doc/api/steam_api
- https://partner.steamgames.com/doc/features/achievements


### 3.9.2 review and collection behavior

- Library selection supports all search results or exclusion of metadata-flagged Risky games. An unflagged game is not certified safe.
- Bulk unlock/relock stages desired values across selected games without writes. Loading is sequential, cancellable between requests, and skips unavailable games with a visible summary.
- Apply reviews every staged change, then sends at most 200 changes per request. Only acknowledged batches leave the pending set. A failure stops the sequence and retains unfinished changes; no automatic write retry occurs.
- Achievement lists are fully scrollable without pagination controls. Account library pages append on vertical scrolling and ignore responses from a previously selected account/query.
- Native icons use Steam achievement handles and ISteamUtils RGBA extraction, with bounded 32px PNG thumbnails and a fallback when Steam supplies no image. Worker output is capped at 64 MiB; input remains capped at 8 MiB.
- SteamAPI_Shutdown only closes the worker API context. No desktop login revocation, Steam credential editing, or desktop LogOff API is used. Engine session-conflict callbacks stop reconnection attempts and preserve the encrypted saved login; the source of an external session conflict cannot be inferred from that callback alone.

Validation uses an isolated native DLL fixture and browser mocks. No live achievements are changed by these checks.
