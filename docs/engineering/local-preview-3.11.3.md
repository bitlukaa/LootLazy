# 3.11.3 local preview

Local verification record before Stable publication. The preview used separate data; ten saved account records and their encrypted config files were copied byte-for-byte without opening them or changing the installed data. Publication was subsequently authorized by the user.

## Implemented

Independent QR and password authentication attempts share one connected Steam transport. The first successful attempt claims token persistence/logon atomically and cancels the other. A failing password attempt does not stop QR; refreshing QR does not clear credential errors. A live anonymous probe exercised exactly one synthetic nonexistent account login and verified QR continuity, independent refresh and cancellation without approving or logging in a real account. Valid email/Guard completion still needs a user-driven trial.

Long-press pointer dragging and Alt+Arrow keyboard movement persist account and Variety order. Active/current/completed games cannot be moved across the current session boundary. Steam Idle pins persist locally and never start games. A fresh desktop account ID from Steam's ActiveProcess registry key is checked against the existing engine's live play intent before desktop starts. If the desktop identity cannot be read, the guard cannot prove absence of a conflict; this does not certify that Steam can never request sign-in.

Username privacy is device-wide and remains visual masking, with hover/focus reveal. The pencil is removed and successful Steam profile updates replace local display labels. Updates have their own header indicator and do not enter session history.

## Achievements list comparison

SAM's picker downloads its maintained games.xml, checks IsSubscribedApp, and then filters normal/demo/mod/junk categories according to its menu choices. LootLazy reads bounded local Steam app metadata and then validates current account subscription. Its 106 entries and SAM's roughly 80 can differ because candidate lists and filters differ. SAM does not dynamically verify that each listed game currently has unlockable achievements in this picker path. Do not hide arbitrary games just to reproduce a count, and do not claim that the missing 26 titles have been identified. No SAM source or external game catalog was incorporated.

Reference inspected: https://github.com/gibbed/SteamAchievementManager/blob/master/SAM.Picker/GamePicker.cs

## Report player limit

Read-only preparation and official profile opening are implemented, with explicit text that no reports are sent from LootLazy accounts. Automatic submission and the requested 60-second multi-account queue are not implemented. See player-report-feasibility.md.
