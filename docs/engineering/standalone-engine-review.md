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

Live QR approval, live password/Guard authentication and transport/session behavior against Steam still require a dedicated test account. Synthetic tests do not certify those live scenarios or establish the absence of all bugs. SteamKit/protobuf publish trimming warnings remain; their root assemblies are explicitly preserved by the project. No release was published and no installed application/account data was changed by this repair.

The candidate is version 3.10.1 with the Beta default channel. The published Stable 3.10.0 is unchanged.
