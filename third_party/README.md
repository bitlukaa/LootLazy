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

## SteamKit corresponding source

`SteamKit2-3.4.0-source.zip` is the complete GitHub source archive at commit
`1c7bc9c41a529e8fbb1e6890f1e4dbcdc5200cb7`, matching the pinned NuGet package.
Archive SHA-256: `c045860464c17627b1f6b4469f659df2672e37b00e08860c1db036182019aaf0`.
It retains SteamRE's original copyright and LGPL license. The complete license
and dependency notices are included in the application's notices file.
You may modify and rebuild/relink the library and application; LootLazy adds no
restriction on debugging modifications to LGPL components. See the engine build
instructions for replacing the package with a locally rebuilt version.
