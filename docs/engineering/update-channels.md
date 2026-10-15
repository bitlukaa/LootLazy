# Update channels

Stable is the normal release. Beta receives new features first and includes stable fixes. GitHub prereleases are the beta distribution; no separate application or extra installation is needed after the channel-capable version is installed.

Settings > Updates lets users opt in to Beta or return to Stable. Confirmation changes discovery only; installing is a separate action. Beta selects the highest valid semantic version among the latest 50 published releases (including stable); drafts and untrusted downloads are excluded. Stable uses GitHub latest and rejects prereleases.

Only an explicit Stable selection enables a lower-version installation. Its authorization is tied to the source application version and expires when that version changes. Hash, size, repository URL and Ed25519 signature checks apply equally to upgrades and returning to stable. Switching channels clears discovery cache. Active sessions still block installation.

The choice is stored separately in data/update-channel.json. A stable package defaults to Stable and a beta package defaults to Beta; saved choices take precedence. Replacing the executable keeps settings, saved queues and encrypted account state. Stable ignores the beta-only Steam Accounts directory. The current beta introduces no incompatible changes to shared data. Future beta migrations must retain stable-compatible shared files or supply a tested reverse migration before release. No automatic deletion or restoration of account data occurs on a channel switch.

Release policy: publish feature releases as GitHub prereleases first, then promote tested changes to stable. Never mark beta as latest. Existing versions without this selector need one channel-capable update (or one manual Beta installation) to bootstrap it.

Shared updater UI and fixes must be carried into both distribution branches before publication. A beta release must not retain an older confirmation flow when Stable changes it.
