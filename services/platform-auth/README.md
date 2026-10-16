# LootLazy platform accounts — staged integration

This service is separate from Steam Accounts. No live domain, hosting or OAuth applications are configured yet. The desktop source defaults to an unlocked preview; mandatory platform login is disabled at compile time. Do not publish an enforced build until the live checks below pass.

## Hosting

Use Node.js 24.4 or newer on a persistent server. Keep this directory and a private persistent data directory; never use ephemeral storage for accounts.sqlite and its WAL files. Copy .env.example to .env, supply the real HTTPS origin and provider credentials, and run `node --env-file=.env server.mjs`. Run under an unprivileged service account, restart on failure, and back up the SQLite database using SQLite's backup facilities. Protect backups like account credentials.

Bind to 127.0.0.1 behind an HTTPS reverse proxy, forwarding to port 8787. Preserve Cookie headers and do not cache any route. Apply additional request limits at the proxy. The built-in limit uses socket IP, so requests through one proxy share its 60 requests/minute and 15 sign-in starts/minute limits; tune deployment capacity before a public rollout. Never expose the private port directly or trust arbitrary forwarded headers.

Google requires a Web application OAuth client and redirect URI `https://YOUR_ORIGIN/v1/callback/google`, scopes openid and profile. Discord requires redirect URI `https://YOUR_ORIGIN/v1/callback/discord`, scope identify. Put client secrets only on the server. Steam uses OpenID at `/v1/callback/steam`; no desktop Steam credentials are sent to this service. Disabled Google/Discord configuration disables those choices. Use the same provider to return to the same account: provider linking and billing are not implemented, and matching email addresses are never silently merged.

## Validation and activation

Run `npm test` here. These tests use simulated providers and do not prove live OAuth works. Check `/health`, then test all configured providers from the desktop using a staging service: successful and cancelled login, wrong state, restart, expired access refresh, offline startup, sign-out, and persistence across a signed app update. Verify secrets and tokens never appear in logs or renderer responses. Have the public authentication service reviewed before production.

Only after live validation, set `serviceUrl` in `src-tauri/platform-auth.json` to the HTTPS origin. Build the desktop with Cargo feature `platform-login` and environment `LOOTLAZY_PLATFORM_SERVICE_VERIFIED=1`. The build refuses an empty URL or missing verification flag. Ensure the release build actually enables the feature before publishing. The setting is compile-time; editing a local file cannot unlock an enforced binary. The service URL must remain stable across updates because Windows session protection is bound to it.

Windows protects platform tokens with DPAPI for the current Windows user. Platform sign-out stops local sessions and removes platform credentials; Steam account profiles remain separate. A revoked or expired platform session requires login. The service currently exposes only Free; paid plans, checkout and cloud idle are intentionally unavailable.
