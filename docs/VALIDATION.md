# Validation notes for 3.6.11

Local regression coverage includes helper path confinement, update signatures, Steam runtime publisher checks, queue persistence, timer suspension accounting, and synchronized-session failure handling. Frontend checks cover localization, settings rollback, refresh cooldowns and custom game-count controls. Native WebView checks use synthetic game identities and isolated data.

Idle Steam uses manual game selection, timers, and an Active Sessions view. It does not collect Steam web sessions or automatically track card drops.

Performance benchmarks include the native host and owned WebView2 processes. Summed working sets include shared pages and differ from private memory or Windows Task Manager's grouped figure. Short samples cannot establish long-term stability. Run `node tools/benchmark-normal-tray.cjs` without an existing LootLazy instance for a normal-release tray measurement; it uses isolated data and does not open a game session. The benchmark terminates only the process tree it launched.

Windows Authenticode signing is not configured. Ed25519 update signatures authenticate the update payload independently and do not eliminate SmartScreen warnings. The private signing key must stay outside the repository and is never included in release assets.

Real account eligibility, rewards, platform enforcement, prolonged Steam/Discord sessions and fresh-machine installation remain outside the automated regression evidence. No platform account-safety guarantee is made.

On the development computer, a 126-second normal tray sample before the memory policy recorded a 532–542 MiB summed working set. After the policy, early samples were 514–517 MiB, then 265–309 MiB. Private memory remained approximately 324–331 MiB after warm-up; this is a resident-memory reduction, not proof that allocated memory was eliminated. Observed CPU averaged 0.69% of one core in the latter short sample. Results are environment-specific.
