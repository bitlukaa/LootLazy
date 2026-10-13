# Vencord integration assessment

LootLazy remains a standalone desktop application. No Vencord plugin is included in this release.

Vencord supports browser-side plugins and optional native Node.js entry points. A plugin could present Discord-specific controls, but Steam sessions, the Windows helper, and update installation require native application functionality; moving them into a plugin is not a direct UI conversion.

A future companion would need a separately designed authenticated local bridge, an explicit user opt-in, narrow commands, and independent testing. LootLazy's existing native-window API restrictions must not be relaxed to expose privileged commands to arbitrary browser or Discord pages. A companion should not request Discord passwords or account tokens.

Reference: https://docs.vencord.dev/plugins/
