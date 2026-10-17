# Player reporting: feasibility and integration boundary

Status: researched, not implemented. No report is sent by this change.

## Requested behavior

A Report player view beside Steam Accounts and Active sessions inside Multi-Account Idle. The user supplies a Steam profile URL, reviews the resolved target, chooses from currently connected accounts, edits a common report description, and explicitly starts a sequential queue. Requested minimum spacing is 60 seconds between submissions. Disconnected accounts are skipped; Stop cancels remaining submissions. Never create or sign in additional accounts automatically.

## Verified public integration

Valve documents ISteamCommunity/ReportAbuse as a restricted publisher API. It requires a Steamworks publisher key and a secure server; the documentation explicitly forbids calling it directly from clients. LootLazy's connected SteamKit accounts are not publisher credentials. Adding a timed queue does not supply that missing authorization.

- https://partner.steamgames.com/doc/webapi/ISteamCommunity#ReportAbuse
- https://help.steampowered.com/en/faqs/view/2F3F-25EE-2AC6-30E7

SteamKit 3.4.0's current documented public client interface was inspected; no general Steam Community report submission method was identified. Game-specific GC report message types are not a verified replacement for reporting an arbitrary Steam profile.

A separate authenticated Steam Community web-session integration would require independent investigation. Anonymous requests to read Steam's profile/shared JavaScript assets were unavailable in this environment. This does not establish that browser automation or a private Community endpoint is impossible; their authentication, request format and acceptance semantics have not been validated. Do not invent an endpoint, expose refresh/access tokens to the renderer, or claim a request succeeded on HTTP status alone.

## Before implementing an automatic queue

Validate a permitted submission path and its authentication requirements. Keep credentials in the engine, validate profile hosts and target SteamID server-side, freeze the reviewed target/accounts/description when starting, and enforce global sequential timing in the engine rather than a frontend timer. Honor server throttling even if longer than 60 seconds. Prevent concurrent or duplicate queues and self-reporting. Distinguish confirmed acceptance, rejection, skipped, canceled and unknown outcome; an ambiguous timeout must not automatically resend a report. Test through a mock transport without reporting real users.

An official manual reporting helper is a separate possible scope and must not be presented as completed automatic multi-account submission.
