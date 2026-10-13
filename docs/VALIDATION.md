# Validation notes for 3.6.16

Current frontend checks cover 21 unit/localization cases and isolated renderer scenarios for the literal Running badge, notification entry navigation, safe details rendering, 30-event retention, five-entry dropdown, filtering of manual refresh/update-check toasts, Clear size, and a single custom-input focus border. Catalog refresh fixtures exercise one transient retry, validation-error rejection, and persistent-failure reporting without removing saved games. Rust application logic is unchanged from the 70-test 3.6.15 validation except version metadata. No memory benchmark was performed.

# Validation notes for 3.6.15

This release adds a new Variety startup/reset policy, independent current-game selection, compact session controls, platform status labels, local notification history, simplified donation/update feedback, and navigation polish. Current checks cover 70 Rust regressions, 18 frontend tests, and isolated renderer scenarios for empty startup, count selection, active controls, returning between tabs, notification retention/bounds/safe text, both languages, and themes. Physical sleep, long live sessions, and clean Windows installation remain untested. No new memory measurements were run.

## Earlier validation history

# Validation notes for 3.6.12

Local checks cover 69 Rust regressions, 17 frontend tests, isolated renderer tests, native WebView2 UI/recovery, update replacement and signing, fixed game-count presets, explicit duration drafts, official download links, and stable game-list rows during timer updates. Sleep accounting, disconnected Discord state, parallel Steam timers, and restart recovery are exercised with fixtures; this is not a physical sleep test or a live platform reward test.

Idle Steam provides manual game selection, timers, and Active Sessions. It does not collect Steam web sessions or automatically track card drops. External clients no longer inherit an application-wide temporary directory override.

A 126-second fresh-data tray sample (normal release, no diagnostic port, no game sessions) recorded a summed host plus WebView2 working set of 165–172 MiB in its last six samples. The final host working set was 33.1 MiB, total private memory 172.0 MiB, and average CPU 1.04% of one core. Early startup working sets were 366–368 MiB. These are short environment-specific samples with a one-game fixture; they do not predict a large real library or simultaneous Steam sessions. Summed working sets include shared pages and differ from Task Manager grouped measurements.

The installer icon is explicitly configured and the extracted installer and application icon images were byte-identical. Portable contents and signed executable hashes are inspected separately. A clean Windows machine and long live Steam/Discord sessions have not been tested; Windows Sandbox is unavailable in this environment.

Windows Authenticode signing is not configured. Ed25519 signatures independently authenticate update payloads and do not remove SmartScreen warnings. Private signing material stays outside the repository and release assets.

A separate native diagnostic sample used an idle interface and then one synthetic local helper. Idle working sets initially measured 379–381 MiB, then fell to 129–172 MiB as the runtime reclaimed memory. During the 63-second active helper sample, total working sets were 226–243 MiB, average CPU 4.48% of one core. This diagnostic run uses CDP and does not measure live simultaneous Steam games.

A clean-install Windows Sandbox plan is prepared by tools/prepare-clean-install.ps1. Its PowerShell syntax was checked, but the guest test has not executed here.

The update startup path also repairs only an inherited TEMP/TMP/TMPDIR value matching this installation's data/temp directory. Other user-configured temporary paths remain unchanged.

A more representative normal tray sample copied the existing public catalog (24,636 games) into an otherwise fresh profile, without credentials or user settings. After 63 seconds, the total working set was 312 MiB and total private memory 329 MiB; the native host accounted for 178 MiB of working set and 157 MiB private memory. Startup totals were approximately 524–527 MiB. This exposes the catalog's contribution in addition to WebView2.

Redundant full-catalog copies during load and refresh were removed without dropping fields or changing the stored public catalog. These short before/after runs did not demonstrate a meaningful reduction in steady working sets (321 versus 312 MiB). Do not generalize the small one-game fixture's 165–172 MiB tray result to a full library. Further sustained memory reduction needs a more compact catalog representation and longer measurements.
