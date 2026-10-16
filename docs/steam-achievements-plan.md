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
