# Questro native comparison verified

After explicit user approval, a 60-second Questro test ran Counter-Strike 2 with RPC disabled and a minimized owned native window. Discord Registered Games displayed Counter-Strike 2 and Now playing! The test ended automatically. Native window inspection found one minimized native game window and zero taskbar-eligible independent windows. This supersedes the earlier blocked/unexecuted comparison below.

# Local reference comparison — Questro 2.2

Source: discord-quest-helper/discord-quest-helper-0.10.9 (MIT, copyright 2025 Masterain).

The concrete omission was RPC. GameSimulator.vue handleRunGame starts the renamed runner, then calls connectToDiscordRpc. stores/quests.ts start-game flow does the same. src-tauri/src/runner.rs builds the Playing activity, connects with the application ID and calls update_activity. Questro previously only spawned its local helper. Window changes alone could not reproduce this activity publication path.

Questro now implements local IPC handshake, framed SET_ACTIVITY requests, acknowledgement matching, ping responses, error handling, retry and cancellation. No account data is retained from READY. The helper remains hidden per the user's requirement. Applications without executable metadata can use activity-only mode, creating no fake filename or helper.

The local reference's App.vue, AppNavigation.vue, TitleBar.vue, QuestCard.vue and style.css informed the navbar, palette, compact quest cards, header artwork fade, status styling and circular theme reveal. Attribution is bundled at public/assets/DISCORD-QUEST-HELPER-LICENSE.txt. Account menus, CDP spoofing, automatic quest enrollment and heartbeat automation were not copied.

Live evidence: LIVE_DISCORD_PRESENCE.json records actual Discord SET_ACTIVITY acknowledgements for Counter-Strike 2 and Deadzone Rogue 2. This proves accepted activity requests, not Registered Games detection or account quest progression.

## Direct test of the running reference application

Overwatch was started with Run Game in Discord Quest Helper 0.10.9. A new overwatch.exe process and a window titled overwatch appeared. The Windows inspection API reported that window as minimized; it was not closed or fully hidden. The launch command contained the executable path only. Discord Registered Games displayed Overwatch and Now playing!, with the reference GameRuntime path. Stop Game was then used; the process and window disappeared. This verifies actual native detection for the reference application. Questro's current fully hidden helper still differs, and no Questro native-window comparison was executed by this test.
