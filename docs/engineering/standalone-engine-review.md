# Standalone Steam engine repair review

The standalone engine continues to depend on SteamKit2 for the Steam protocol and Codecrete for QR encoding. Removing ASF does not remove those dependencies or their notices.

## Repairs

- Both QR and credential authentication wait for a connected Steam client. Credentials use SteamKit authentication sessions with an IPC-backed email/device Guard authenticator instead of direct password logon.
- Successful authentication saves the refresh token and new Guard data before logging on. Persistence failures are surfaced and preserve the previous in-memory and on-disk login.
- Authentication flows are registered before returning to the UI, bounded to three minutes and canceled on stop/removal. Generations prevent stale authentication from starting a later session. A new connection waits for an intentional disconnect to finish.
- Each client instance uses a cryptographically generated nonzero LoginID. Session-conflict logoffs stop reconnecting; an explicit retry may use the saved token.
- PlayingSessionState is authoritative, including when another client plays the same app. Persona updates only supply display/avatar information. Sending a nonempty game list checks the current block flag again.
- Unreadable protected login files are retained until successful reauthentication saves replacement credentials. Their profiles remain available for sign-in or removal.

## Verification

- Built and self-tested the trimmed, self-contained Windows engine.
- Self-tests exercise device/email challenges, cancellation cleanup, unique LoginIDs, same-game occupation, disconnected play rejection, transactional token-write failure and persisted Guard data.
- Account protocol and startup tests use isolated synthetic profiles and protected tokens. No real Steam authentication is performed.
- All four renderer suites and 26 unit tests passed.
- Native tests: 85 passed, one explicitly local-Steam-cache test ignored; includes the embedded engine round-trip.
- Rust formatting checked after repairing three pre-existing formatting failures.

## Limits

Live QR approval, live password/Guard authentication and transport/session behavior against Steam still require a dedicated test account. Synthetic tests do not certify those live scenarios or establish the absence of all bugs. SteamKit/protobuf publish trimming warnings remain; their root assemblies are explicitly preserved by the project. No installed application/account data is changed by verification. Live Steam sign-in is not certified by synthetic tests.

## Reliability and privacy follow-up

The approved promotion candidate is 3.11.0 with a Stable default channel. Login attempts share a cancelable one-second pacing gate; repeated transport failures back off from three to sixty seconds. Steam throttle responses enforce a fifteen-minute shared cooldown. Saved-login connection attempts have a thirty-second watchdog. Known expired tokens require explicit sign-in; opaque legacy tokens are left for Steam to validate. This is not local token signature verification or a claim of ASF feature parity.

Temporary disconnection preserves requested games and freezes elapsed time until the game list is successfully re-sent after connection/occupation checks. Explicit Stop and session conflict still cancel play intent. A small SteamKit packet handler tracks the protocol's own-library lock field in addition to the native playing-state callback.

Privacy remains a device-wide visual blur on the account detail Steam login. Mouse hover and keyboard focus reveal it. The plaintext remains in application state/DOM; this is not a security boundary. Storage read failure defaults to masking; storage write failure keeps the prior choice and shows an error.

Regression coverage includes token expiry, retry classification/backoff, login-gate cancellation, session suspension/resumption and Stop while suspended; renderer coverage includes privacy persistence after reload, hover/focus reveal and denied storage. Final release CI and signature/feed verification are required before deleting Beta releases.
