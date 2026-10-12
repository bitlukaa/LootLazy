<div align="center">

<img src="public/assets/lootlazy-logo.svg" width="100" height="100" alt="LootLazy">

# LootLazy

### Discord quests. Gaming badges. Steam hours.

**Lightweight game activity, without downloading every full game.**

[![Download](https://img.shields.io/github/v/release/bitlukaa/LootLazy?label=Download&color=5865f2)](https://github.com/bitlukaa/LootLazy/releases/latest)
[![Windows](https://img.shields.io/badge/Windows-10%20%2F%2011-0078d4)](https://github.com/bitlukaa/LootLazy/releases/latest)
[![License](https://img.shields.io/badge/License-MIT-16a34a)](LICENSE)
[![Support](https://img.shields.io/badge/Support-LootLazy-5865f2)](#support-lootlazy)

**Manual quest controls. Local game sessions. No Discord login, account tokens, or injected client scripts.**

[Download](https://github.com/bitlukaa/LootLazy/releases/latest) · [Features](#features) · [Report a bug](https://github.com/bitlukaa/LootLazy/issues) · [Support](#support-lootlazy)

</div>

LootLazy brings Discord game activity, desktop play quests, and Steam idling into one app. Choose your games, set the time, and start — whether you're working toward gaming badges, lining up several quests, or keeping your owned Steam games active.

If LootLazy helps you, consider leaving a star!

## Features

- **Game Time** — Run lightweight game activity with continuous, preset, or custom timers. Manage one game or a saved queue.
- **Game Variety** — Begin with 10 games and choose 10, 50, 100, or save your own custom counts. Remove saved custom counts, shuffle, search, reorder, and run the group with one duration per game. Paused sessions with playtime can be resumed.
- **Desktop play quests** — Browse available 15-minute quests and start directly from their cards. Accept the quest in Discord first; unfinished local sessions can be resumed.
- **Idle Steam** — Keep owned games active for playtime and eligible Steam Trading Card drops. Run several games together with separate timers, and search for titles even when they aren't installed.
- **Two-way sync** — Start matched Discord and Steam sessions together from either page.
- **Pick up where you left off** — Pause, resume, clear, and reorder saved queues after reopening LootLazy.
- **Make it yours** — Light and dark themes, English and Arabic, compact client indicators, desktop notifications, and tray controls.
- **Keep it light** — A native Windows app with an executable of approximately 4.3 MB.

## Screenshots

The card-drop panel is a preview; remaining-card tracking and automatic farming are not enabled.

| Game Time | Game Variety |
|:---:|:---:|
| [![Game Time](https://raw.githubusercontent.com/bitlukaa/LootLazy/main/docs/screenshots/game-time.png)](https://raw.githubusercontent.com/bitlukaa/LootLazy/main/docs/screenshots/game-time.png) | [![Game Variety](https://raw.githubusercontent.com/bitlukaa/LootLazy/main/docs/screenshots/game-variety.png)](https://raw.githubusercontent.com/bitlukaa/LootLazy/main/docs/screenshots/game-variety.png) |

| Idle Steam | Quests |
|:---:|:---:|
| [![Idle Steam](https://raw.githubusercontent.com/bitlukaa/LootLazy/main/docs/screenshots/idle-steam.png)](https://raw.githubusercontent.com/bitlukaa/LootLazy/main/docs/screenshots/idle-steam.png) | [![Quests](https://raw.githubusercontent.com/bitlukaa/LootLazy/main/docs/screenshots/quests.png)](https://raw.githubusercontent.com/bitlukaa/LootLazy/main/docs/screenshots/quests.png) |

Click a screenshot to view it at full size.

## Download & run

Choose your version from [GitHub Releases](https://github.com/bitlukaa/LootLazy/releases/latest).

| Download | Instructions |
|---|---|
| **Windows x64 · Setup** | Run the installer and choose your installation folder. |
| **Windows x64 · Portable ZIP** | Extract the archive and run `LootLazy.exe`. |
| **Windows x64 · Portable EXE** | Download and run the executable directly. |

Windows 10/11 (64-bit) and Microsoft Edge WebView2 are required. Setup can download WebView2 if needed.

Download 3.6.10 manually if upgrading from Questro or LootLazy 3.6.9: earlier update helpers do not handle the renamed executable correctly. Keep your existing portable folder to retain its data. Version 3.6.10 fixes the installer helper and preserves legacy settings and signing verification.

## Getting started

**Discord activity and badges:** keep Discord open, select a game, set its duration, and start. Use Game Variety for a lineup of different games. Discord must detect eligible games for badge progress to count.

**Quests:** accept the quest in Discord and choose **Desktop**, then click **Start Quest** in LootLazy. Check progress and claim your reward in Discord yourself.

**Steam:** sign in to Steam, choose a game you own, and start. A compatible Valve-signed runtime from an installed Steam game is required; the selected title does not have to be installed.

**Trading Cards:** choose an eligible game with card drops remaining to idle while Steam awards available drops. Check remaining drops on your Steam Badges page. LootLazy does not track remaining cards or stop automatically when drops run out; Steam controls eligibility and drop timing. See the [Steam Trading Cards FAQ](https://steamcommunity.com/tradingcards/faq/).

**Updates:** use **Check updates** in Settings, then **Download & install** when a newer release is available. Stop active sessions first; saved settings and queues are preserved. Starting with version 3.6.8, updates also require a publisher signature.

## Privacy & account safety

Local session helpers run separately from the game. LootLazy does not inject code into game processes or modify game files or memory.

LootLazy does not request Discord passwords or account tokens, and does not automatically accept quests or claim rewards. Sessions use LootLazy's local timer; the app does not read your account's quest progress.

> [!WARNING]
> We cannot guarantee that using LootLazy will not result in account restrictions or a ban. Use it at your own discretion and follow each platform's rules. Local timers do not guarantee badge progress, recorded Steam hours, or quest rewards.

<a id="support-lootlazy"></a>

## Support LootLazy

If LootLazy makes things easier for you, your support helps make time for improvements, fixes, and new features. No pressure — feedback and sharing the project help too.

**Crypto support**

| Currency | Network |
|---|---|
| USDT | Ethereum (ERC-20) or BNB Smart Chain (BEP-20) |
| ETH | Ethereum |
| BNB | BNB Smart Chain |

Donation address:

`0xcbd6fec2fb4acb31dc3dacac133ca364ccbe58d8`

Make sure your sending network matches the network you intend to use.

## Community

[Contributing](CONTRIBUTING.md) · [Community guidelines](CODE_OF_CONDUCT.md) · [Security policy](SECURITY.md)

## License

LootLazy is available under the [MIT License](LICENSE). Third-party notices are included with the downloads.

*LootLazy is an independent project and is not affiliated with Discord or Valve.*
