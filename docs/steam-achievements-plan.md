# Steam Achievements — proposed implementation

Status: plan only; no Achievements tab or unlock implementation is shipped in this change.

## Product placement and scope

Add Achievements as a secondary Steam Tools service alongside Library, Accounts and Active sessions. Reuse LootLazy panels, small game artwork, inline status badges and centered confirmation cards. Keep the main header unchanged. No platform registration, paid plan or extra standalone application is required.

The first implementation targets the account currently signed into the Steam desktop client. SAM itself requires that client and an authenticated account. Steam Accounts uses a separate protocol connection; do not imply that its accounts automatically work with the native UserStats API. Multi-account achievement support requires a separate feasibility investigation.

## Delivery stages

1. Read-only discovery: select an owned game; request current stats; show achievement name, description, icon, locked/unlocked state, unlock time and total completion. Handle games with no achievements and games whose stats cannot be loaded. Refresh confirms server state; cached data is labeled stale while offline.
2. Review and apply: stage individual unlock/relock changes locally. Display a change count and an explicit Apply confirmation. Take a local snapshot before writes, use SetAchievement/ClearAchievement followed by StoreStats, wait for the completion callback and read back actual state. Partial rejection stays visible per item; never mark a write successful optimistically. No automatic changes or bulk changes across accounts.
3. Optional later work: filtered batch selection and stat editing only after type/range checks and partial-failure recovery are demonstrated. Preserve existing values and distinguish unsupported/server-controlled data. Do not promise that every game permits every write.

## Technical approach

Use an internal worker to isolate each game's Steam API context from the main UI and from the existing idle sessions. Serialize writes per game/account, validate owned AppIDs and compare the active Steam account before applying changes. Changing accounts, closing a card or a worker timeout must cancel pending UI work and cannot apply it to another account. Reconnect and restart recover the last observed state, without automatically replaying staged writes.

Evaluate SAM.API bindings and the installed Steam runtime before selecting a concrete worker implementation. If source is reused, keep its zlib notice, clearly mark modifications, and audit the separate icon attribution. Do not bundle the external SAM program as a second user-facing app.

## Validation before Beta

Test read-only and no-achievement games, active-account changes, offline/timeout recovery, unsupported writes, partial failure, worker exit and accurate readback after StoreStats. Use isolated fixtures for protocol tests, then user-authorized live tests with a chosen owned game. Publish an optional Beta only after the read-only stage is verified; keep write controls absent until the apply flow works.

## Primary references

- SAM repository and README: https://github.com/gibbed/SteamAchievementManager
- SAM license: https://github.com/gibbed/SteamAchievementManager/blob/master/LICENSE.txt
- Valve ISteamUserStats interface: https://partner.steamgames.com/doc/api/ISteamUserStats
- Valve achievement guide: https://partner.steamgames.com/doc/features/achievements
