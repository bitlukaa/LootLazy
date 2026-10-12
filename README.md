# Questro

A lightweight Windows app for game sessions, Discord quests, and Steam timers.

[![Download](https://img.shields.io/github/v/release/bitlukaa/Questro?label=Download&color=777af8)](https://github.com/bitlukaa/Questro/releases)
[![Windows](https://img.shields.io/badge/Windows-10%20%2F%2011-0078d4)](https://github.com/bitlukaa/Questro/releases)
[![Support Questro](https://img.shields.io/badge/Support-Questro-777af8)](#support-questro)

Questro keeps things simple: choose your games, set a timer, and start a session. Queue games one after another, build a shuffled selection, or manage several Steam sessions together — all from one place.

## Features

- **Game Time:** browse games, choose preset or custom durations, and save a queue you can pause and resume after reopening the app.
- **Game Variety:** choose 10, 50, 100, or a custom number of games. Search and reorder the list, shuffle it, and apply one duration to the group.
- **Quests:** view available 15-minute desktop play quests, start them from their cards, and resume unfinished sessions. Video and activity quests stay out of the list.
- **Idle Steam:** run independent sessions together, with continuous, preset, or custom timers. Search for titles beyond your installed games.
- **Optional two-way sync:** start a matched Discord and Steam session together from either Game Time or Idle Steam.
- **A compact interface:** light and dark themes, English and Arabic, live client status, tray controls, and optional completion notifications.

## Get started

1. Download the latest archive from [Releases](https://github.com/bitlukaa/Questro/releases), extract it, and open **Questro.exe**.
2. Choose a game or quest and set your duration.
3. Start the session. You can pause, resume, or clear a saved queue whenever you need.

For quests, accept the quest in Discord and choose **Desktop** before starting it in Questro. Check progress and claim any reward in Discord itself.

For Steam sessions, the game must be in your Steam library. Steam needs to be signed in, and a compatible Steam runtime must be available from an installed game. The selected title does not have to be installed.

## Latest update · 3.5.4

- Corrected Settings layout.
- Two-way Discord and Steam sync for matched titles.
- Short badge guidance and Steam library search tips.
- New Game Variety and Idle Steam pages.
- Saved queue recovery and optional Steam sync.
- Game Variety duration presets, custom hours and minutes, search, and reordering.
- Simpler quest controls, with a resume action for unfinished sessions.
- A dedicated Donation page and optional completion notifications.

See the [release history](CHANGELOG.md) for updates.

## Privacy & limitations

Questro does not request your Discord password or account token, and does not automatically accept quests or claim rewards. However, we cannot guarantee that using it will not result in account restrictions or a ban. Use it at your own discretion.

A completed local timer does not confirm quest completion or recorded Steam playtime. Compatibility and reward eligibility are not guaranteed; follow each platform's rules.

Windows 10 or 11, 64-bit, and Microsoft Edge WebView2 are required. The current executable is approximately **4.2 MB**; saved data and cached artwork use additional space.

## Support Questro

If Questro has been useful to you, thanks for using it. Feedback, bug reports, and sharing the project help a lot.

Optional coffee and crypto support are planned. The in-app Donation page is ready; payment links will be added once they are available.

## Build from source

Requires Rust with the MSVC toolchain and the Windows C++ build tools.

```powershell
 git clone https://github.com/bitlukaa/Questro.git
 cd Questro
 powershell -ExecutionPolicy Bypass -File .\tools\build-tauri.ps1
```

Questro is an independent project and is not affiliated with Discord or Valve.
