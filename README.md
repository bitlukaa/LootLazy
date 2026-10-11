# Questro 🎮

<div align="center">

### The Ultra-Lightweight, 100% Account-Safe Discord Quest & Game Activity Companion

[![Release](https://img.shields.io/github/v/release/bitlukaa/Questro?color=7289da&label=Download%20Latest&style=for-the-badge)](https://github.com/bitlukaa/Questro/releases)
[![Platform](https://img.shields.io/badge/Platform-Windows%2010%20%2F%2011%20(64--bit)-0078D6?style=for-the-badge&logo=windows)](https://github.com/bitlukaa/Questro/releases)
[![Safety](https://img.shields.io/badge/Account%20Safety-100%25%20Ban--Free-23a55a?style=for-the-badge&logo=discord)](#-100-account-safe--zero-ban-risk)
[![Engine](https://img.shields.io/badge/Engine-Rust%20%2B%20Tauri%20v2-orange?style=for-the-badge&logo=rust)](https://tauri.app)
[![License](https://img.shields.io/badge/License-MIT-blue?style=for-the-badge)](LICENSE)

<br/>

**Complete Discord Quests & grind 5,000+ hour game badges effortlessly — without downloading 100 GB games, overheating your GPU, or risking your Discord account.**

[📥 Download Questro (Free & Portable)](https://github.com/bitlukaa/Questro/releases) • [🛡️ Safety Architecture](#-100-account-safe--zero-ban-risk) • [⚡ Quick Start](#-quick-start-guide) • [✨ Key Features](#-key-features)

</div>

---

## 💡 The Problem Questro Solves

Have you ever:
- Had to download a **70–120 GB game** (like *Call of Duty*, *Genshin Impact*, or *Warframe*) just to finish a simple **15-minute Discord Quest** for an avatar decoration or reward?
- Wanted to grind playtime hours for Discord profile badges (like the **5,000+ hour gameplay badges**), but leaving AAA games running burns electricity, raises room temperatures, and stresses your GPU/CPU?
- Feared using untrusted automation bots or third-party web tools that ask for your Discord token and risk getting your account permanently banned?

**Questro is built specifically for you.**  
It emulates legitimate local game activity using a lightweight native helper process. Discord recognizes the game natively via local Windows IPC, allowing you to complete quests and accumulate verified playtime while consuming **virtually 0% CPU and under 15 MB RAM**.

---

## 🛡️ 100% Account Safe & Zero Ban Risk

Your Discord account's safety is our **#1 priority**. Questro was engineered from the ground up to follow Discord's official local desktop architecture:

| Security Feature | Questro | Traditional Selfbots / Exploits |
| :--- | :---: | :---: |
| **No Account Credentials** | ✅ Never asks for password, email, or token | ❌ Steals or requires user tokens |
| **No API Violations** | ✅ Zero external Discord API requests | ❌ Spams Discord REST endpoints |
| **Zero Client Tampering** | ✅ Untouched Discord files, no DLL injection | ❌ Modifies Discord client files |
| **Official Local IPC** | ✅ Uses native Windows Named Pipes (`discord-ipc-*`) | ❌ Unofficial unauthorized protocols |
| **Legitimate Workflow** | ✅ You accept & claim quests inside official Discord | ❌ Emulates illegal automated claims |

> **Bottom line:** Questro acts exactly like launching a legitimate game executable on your PC. Discord sees the process running locally through its standard Rich Presence channel. It is **100% safe, non-intrusive, and ban-free**.

---

## 🌟 Key Features

### 🎯 1. Instant Discord Quests Companion
- Automatically synchronizes available Discord desktop quests.
- Play and finish 15-minute game quests with a single click—no game installations required.
- Smart notification alerts you the moment the quest timer finishes so you can claim your reward in Discord.

### 📋 2. Automated Multi-Quest & Game Queue
- Accept multiple quests inside your Discord desktop client, add them to Questro's queue, and let Questro run them sequentially back-to-back.
- Includes automatic countdown transitions between games so you can step away and let it finish hands-free.

### ⏳ 3. Game Time & 5,000+ Hour Badge Grinder
- Build up verified playtime on your Discord profile for any game you choose.
- Perfect for grinding long-term badges (such as the prestigious 5,000+ hour playtime badges) without keeping heavy games running in the background.
- Set custom durations (15m, 1h, 5h, or continuous uninterrupted simulation).

### 🟢 4. Real-Time Discord Connection Indicator
- Live topbar status dot (**Discord Connected** / **Discord Disconnected**) monitors your Discord desktop client in real time.
- **Smart Timer Protection:** The timer automatically pauses if Discord is closed, and only counts down when Discord is actively running and connected—ensuring not a single second is wasted.

### 🍃 5. Ultra-Lightweight & Resource-Friendly
- Powered by a native Rust backend and Tauri v2.
- Uses **~0% CPU** and **<15 MB of RAM**—run it in the background while studying, working, or playing other lightweight games.
- Works flawlessly on low-end laptops and budget PCs.

### 🎨 6. Modern UI with Light & Dark Modes
- Fast, clean, responsive user interface with complete English and Arabic (RTL) support.
- Beautiful Light and Dark themes with smooth animated transitions.
- Minimizes cleanly to the system tray (`Minimize to Tray`).

---

## ⚡ Quick Start Guide

It takes less than 30 seconds to get started:

1. **Accept the Quest in Discord:**  
   Open your Discord desktop app, go to **User Settings > Gift Inventory / Quests**, and click **Accept Quest** (select **Desktop**).
2. **Download & Run Questro:**  
   Grab the latest single-file `Questro.exe` from the [Releases](https://github.com/bitlukaa/Questro/releases) page. No installation required!
3. **Start the Session:**  
   Open Questro, click on the **Quests** tab, select the accepted quest, and click **Start**.
4. **Claim Your Loot:**  
   Once the countdown ends, Questro sends you a desktop notification. Open Discord and claim your reward!

---

## 📥 Download & Compatibility

| Specification | Requirement |
| :--- | :--- |
| **Operating System** | Windows 10 / Windows 11 (64-bit) |
| **Installation** | **None** (100% Portable single `.exe` under 5 MB) |
| **Dependencies** | Microsoft Edge WebView2 (built into Windows 10/11) |
| **Hardware** | Any PC or laptop (virtually zero CPU/GPU overhead) |

👉 **[Download the Latest Questro Release](https://github.com/bitlukaa/Questro/releases)**

---

## 🛠️ For Developers & Contributors

Questro is proudly open-source. You can inspect the full source code and build it locally:

```bash
# Clone the repository
git clone https://github.com/bitlukaa/Questro.git
cd Questro/project

# Build the release binary using the automated PowerShell build script
powershell -ExecutionPolicy Bypass -File .\tools\build-tauri.ps1
```

- **Backend:** Rust 2021, Tauri v2, local Windows Named Pipes IPC.
- **Frontend:** Vanilla HTML5, modern CSS3, native ECMAScript—zero bulky frontend frameworks.

---

## 📜 License & Disclaimer

Questro is released under the [MIT License](LICENSE).  
*Disclaimer: Questro is an independent utility and is not affiliated with, endorsed by, or sponsored by Discord Inc. Discord and all related trademarks are the property of their respective owners.*
