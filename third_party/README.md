# ASF source snapshot

This directory contains the unmodified, editable ASF 6.3.10.6 source at commit
`c156c7ba6a2ebbfce5bdf70d9f1a69ac77e23ee9`, imported from
https://github.com/JustArchiNET/ArchiSteamFarm.

The source is stored in LootLazy itself, not a Git submodule or a download at runtime.
It remains available if upstream stops development. It retains the original Apache-2.0
license, copyright notices, and authorship. The separate ASF-ui and wiki submodules
are deliberately not fetched: LootLazy will supply its own interface and documentation.
The engine project builds without those submodules.

`asf-source.lock.json` records every upstream source blob. Run
`node tools/verify-asf-source.cjs` after staging to check the imported snapshot.
Keep integration-specific changes as reviewed patches outside the snapshot initially;
record patch provenance if the fork is later edited directly.

This import is a development foundation. It does not enable accounts, bundle an ASF
binary, change LootLazy's current runtime, or publish a new application release.
See [integration review](../docs/engineering/steam-accounts.md) for the implementation
contract and remaining work. Do not publish the unmodified engine as a finished feature.
