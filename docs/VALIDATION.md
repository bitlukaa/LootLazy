# Validation notes for 3.6.12

Local checks cover 69 Rust regressions, 17 frontend tests, isolated renderer tests, native WebView2 UI/recovery, update replacement and signing, fixed game-count presets, explicit duration drafts, official download links, and stable game-list rows during timer updates. Sleep accounting, disconnected Discord state, parallel Steam timers, and restart recovery are exercised with fixtures; this is not a physical sleep test or a live platform reward test.

Idle Steam provides manual game selection, timers, and Active Sessions. It does not collect Steam web sessions or automatically track card drops. External clients no longer inherit an application-wide temporary directory override.

A 126-second fresh-data tray sample (normal release, no diagnostic port, no game sessions) recorded a summed host plus WebView2 working set of 165–172 MiB in its last six samples. The final host working set was 33.1 MiB, total private memory 172.0 MiB, and average CPU 1.04% of one core. Early startup working sets were 366–368 MiB. These are short environment-specific samples with a one-game fixture; they do not predict a large real library or simultaneous Steam sessions. Summed working sets include shared pages and differ from Task Manager grouped measurements.

The installer icon is explicitly configured and the extracted installer and application icon images were byte-identical. Portable contents and signed executable hashes are inspected separately. A clean Windows machine and long live Steam/Discord sessions have not been tested; Windows Sandbox is unavailable in this environment.

Windows Authenticode signing is not configured. Ed25519 signatures independently authenticate update payloads and do not remove SmartScreen warnings. Private signing material stays outside the repository and release assets.

A separate native diagnostic sample used an idle interface and then one synthetic local helper. Idle working sets initially measured 379–381 MiB, then fell to 129–172 MiB as the runtime reclaimed memory. During the 63-second active helper sample, total working sets were 226–243 MiB, average CPU 4.48% of one core. This diagnostic run uses CDP and does not measure live simultaneous Steam games.

A clean-install Windows Sandbox plan is prepared by tools/prepare-clean-install.ps1. Its PowerShell syntax was checked, but the guest test has not executed here.

The update startup path also repairs only an inherited TEMP/TMP/TMPDIR value matching this installation's data/temp directory. Other user-configured temporary paths remain unchanged.
