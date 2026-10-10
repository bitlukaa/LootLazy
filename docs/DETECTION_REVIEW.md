# Detection review

The current process manager copies `runner/src-win.exe` into `games/<application ID>/<selected executable path>` and starts that helper. Windows binary metadata reports its original filename as `src-win.exe`; product/company metadata are blank. The game itself is not installed or executed by this operation.

This explains the key limitation: local launch success is not evidence of Discord detecting the genuine game. The public catalog supplies filenames, not a guarantee that a replacement binary will satisfy a particular client's detection rules. No exact rejection condition inside the user's Discord client was observed.

The previous generated-name fallback could create arbitrary names for missing registry entries. Version 2.0 removes that behavior, blocks unresolved starts, and retains explicit custom names, valid Windows catalog names, discovered local executables and unverified community candidates as distinct cases.

A game-specific investigation needs the game name and the executable selected by the app. Requested this information from the user. No client token, private Discord APIs, spoofed progress, fake Steam manifests or Discord code modifications were used.

Official activity settings reference: https://support.discord.com/hc/en-us/articles/7931156448919-Activity-Sharing-on-Discord-FAQ
