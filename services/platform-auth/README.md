# Platform account service — paused reference

Platform account sign-in was cancelled for now. The desktop authentication integration, login UI, Plans UI and its build feature have been removed. The current app is free and uses no platform login. Steam Accounts remains separate and continues to connect directly to Steam.

These server sources are retained only as an unconfigured, undeployed reference. They do not run with the desktop app. Reintroducing platform accounts requires a new implementation and live-provider validation; there is no desktop activation flag anymore. No domain, hosting or provider OAuth applications are configured.

The reference uses Node.js 24.4+, SQLite, Google/Discord OAuth and Steam OpenID. Run `npm test` here for simulated provider and HTTP tests. Provider linking, payments and paid plans are not implemented. Never put provider secrets in the desktop app or Git repository.
