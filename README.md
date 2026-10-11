# Questro 🎮

[![Release](https://img.shields.io/github/v/release/bitlukaa/Questro?color=blue&label=Latest%20Version)](https://github.com/bitlukaa/Questro/releases)
[![Platform](https://img.shields.io/badge/Platform-Windows%2010%20%2F%2011%20(64--bit)-0078D6?logo=windows)](https://github.com/bitlukaa/Questro/releases)
[![License](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Safety](https://img.shields.io/badge/Discord%20Account-100%25%20Safe-success?logo=discord)](#why-questro-is-100-account-safe)
[![Engine](https://img.shields.io/badge/Built%20With-Rust%20%2B%20Tauri%20v2-orange?logo=rust)](https://tauri.app)

> **The ultra-lightweight, 100% account-safe Discord game activity & quest companion.**  
> Complete Discord Quests and grind playtime hours without downloading massive 50–100 GB games, overheating your GPU, or risking your Discord account.

---

## ✨ Why Questro?

Downloading a 100 GB game just to complete a 15-minute Discord Quest or keeping resource-heavy games running in the background for hours eats up disk space, burns electricity, and stresses your computer. 

**Questro solves this gracefully.** It launches a tiny, lightweight native helper process that simulates local game activity, allowing Discord to recognize the game legitimately—with virtually **0% CPU** and under **15 MB of RAM**.

---

## 🛡️ Why Questro is 100% Account Safe

Unlike risky selfbots or automated Discord API tools that violate Discord Terms of Service and get accounts banned, **Questro never touches your Discord credentials or API**:

* 🔒 **Zero Credentials or Tokens**: Questro never asks for your email, password, or Discord user token.
* 🛡️ **Zero Client Tampering**: No DLL injection, memory hooking, or modified Discord clients. Your Discord installation remains completely untouched.
* 🤝 **Official Rich Presence Channel**: Questro communicates game status through Discord's official local IPC socket—the exact same mechanism used by verified Steam and Epic Games titles.
* 👤 **Manual Quest Workflow**: You accept quests, select "Desktop Play", and claim rewards directly inside your official Discord app. Questro strictly runs the local game session process.

---

## 🚀 Key Features

* 🎯 **Discord Quests Explorer**: Instantly browses and syncs available Discord desktop quests. Start any quest with a single click.
* 📋 **Multi-Quest & Game Queue**: Queue multiple games or quests back-to-back. Questro runs them in order automatically so you can finish all your quests hands-free.
* ⏱️ **Game Time & Badge Grinder**: Want to log game hours on your profile or grind long-term playtime badges (including 5,000+ hour badges)? Run continuous or timed sessions without heating up your PC.
* 🍃 **Near-Zero Resource Footprint**: Built with native Rust and Tauri v2. Uses virtually no CPU and minimal memory—perfect for low-end PCs, laptops, and multitasking while working or studying.
* 📦 **100% Portable & Self-Contained**: No installation required and no Steam dependency. Runs out of the box with zero setup.
* 🔔 **Native Windows Integration**: Windows Toast notifications alert you when sessions finish, with taskbar flashing attention and minimize-to-tray on close.
* 🌍 **Bilingual & Modern Interface**: Smooth Dark and Light themes with complete English and Arabic (RTL) localization.

---

## ⚡ How It Works (Quick Start)

1. **Accept the Quest**: Open the official Discord app, go to your Quests tab, and click **Accept Quest** (choose **Desktop**).
2. **Launch Questro**: Download and run `Questro.exe`.
3. **Start Playing**: Select your quest from the **Quests** tab (or choose any game in **Game Time**) and click **Start Session**.
4. **Claim Your Reward**: When the session completes, Questro alerts you with a desktop notification. Head back to Discord and claim your reward!

---

## 📥 Download & Installation

Questro is a portable Windows executable—no installer needed.

1. Head to the **[Latest Release](https://github.com/bitlukaa/Questro/releases)** page.
2. Download `Questro.exe`.
3. Double-click to run!

### System Requirements
* **OS:** Windows 10 or Windows 11 (64-bit)
* **Runtime:** [Microsoft Edge WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/) (pre-installed on almost all modern Windows systems)

---

## 🛠️ Built for Developers

Questro is fully open-source and engineered for performance:
* **Backend:** Rust (Tauri v2) for native execution, strict security boundaries, and minimal memory usage.
* **Frontend:** Clean vanilla HTML5, CSS3, and JavaScript—fast, dependency-free, and accessible.

```bash
# Clone the repository
git clone https://github.com/bitlukaa/Questro.git
cd Questro

# Build the release executable
powershell -ExecutionPolicy Bypass -File .\tools\build-tauri.ps1
```

---

## 📄 License

Questro is licensed under the [MIT License](LICENSE).
