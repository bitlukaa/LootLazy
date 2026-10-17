# Home and update indicator preview 3.11.5

Unpublished local preview. Home retains its header tab and removes the in-page Home heading, subtitle and statistic tiles, as clarified by the user.

Home shows actual Steam/Discord desktop status and the live connected-account count without starting the account engine. It imports the existing bounded backend session history and keeps at most twelve recent-game records locally, rendering up to eight cards. Records contain game metadata, section, time and duration, not account identifiers, login names, passwords or executable paths. Legacy Steam Idle items with elapsed play time can appear as Previous session without inventing a last-played date. Previously stopped Multi-Account sessions without any recorded history cannot be reconstructed retroactively.

Recent game cards navigate/select without launching a game. Quest cards reuse the existing available feed and New tracking, exclude finished/expired quests, and navigate without starting a quest. Direct live session controls remain available.

Update checks already ran at startup and hourly; the interval is now five minutes, with throttled checks on focus/visibility return and a reconnect retry. A temporary offline/rate-limit result preserves an already known available update. The header indicator intentionally remains hidden when the installed version is current.

The Steam mark is the original application icon extracted from a Valve Corp. signed local Steam desktop executable, with colours preserved; attribution is in platform-branding.txt.

Verification includes scheduling/reconnect/transient-failure unit cases, renderer history persistence/privacy and quest navigation cases, and native WebView2 history/session checks. No real reports, achievements or account sign-ins are performed during verification.
