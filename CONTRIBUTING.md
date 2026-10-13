# Contributing to LootLazy

Bug reports, usability feedback, and focused improvements are welcome.

## Report an issue

Include your LootLazy version, Windows version, steps to reproduce the problem, and what you expected to happen. Screenshots are helpful. Remove personal information before sharing logs or screenshots.

Never include passwords, account tokens, payment details, or private account data. For security issues, follow [SECURITY.md](SECURITY.md).

## Suggest a feature

Describe the problem it solves and how it fits the existing interface. LootLazy focuses on local game sessions and manual quest controls; proposals should preserve that approach.

## Submit a change

1. Open an issue for substantial changes before starting work.
2. Keep pull requests focused on one problem.
3. Follow the existing code style. Keep application logic and comments in English, and provide English and Arabic interface text where needed.
4. Include the checks relevant to your change and explain any limitations.

For native changes, run:

```powershell
cargo fmt --manifest-path src-tauri/Cargo.toml --check
powershell -ExecutionPolicy Bypass -File .\tools\test-tauri.ps1
```

Do not commit generated builds, local settings, game helper files, credentials, or private diagnostics. Preserve required third-party notices.

Review and understand any code produced with AI assistance before submitting it. Credit actual collaborators when appropriate.

By contributing, you agree that your contribution may be distributed under the project's [MIT License](LICENSE).

## Reproduce the frontend checks

Install Node.js 22 or later and run `npm ci --ignore-scripts`, `npx playwright install chromium`, `npm run test:unit`, and `npm run test:renderer`. Set LOOTLAZY_BROWSER only to use a specific installed Chromium browser. Build Windows helpers with `tools/build-helper.ps1` before direct Cargo builds; the normal build script does this automatically.

## Release signing

Initialize the local key once with `tools/init-update-key.ps1`. It is protected by Windows DPAPI for the current user under LOCALAPPDATA/LootLazySigning, outside this repository. Only `src-tauri/update-public-key.hex` belongs in source control. Preserve the protected key and the Windows profile needed to recover it; never rotate the embedded public key casually or upload the private key.

After the final build, run `tools/sign-release.ps1 -BinaryPath releases/LootLazy-<version>/LootLazy.exe`. Publish the resulting file with the exact asset name `LootLazy.exe.sig` alongside `LootLazy.exe` and the portable ZIP. The signature binds the version, SHA256 and file size. CI artifacts are development builds and are not automatically signed or published.

Older clients do not enforce the new signature policy until they install version 3.6.8 or later. Windows Authenticode signing of LootLazy itself is separate and still requires an appropriate publisher certificate. Steam SDK verification uses the installed SDK publisher signature and does not sign or modify the game.

Keep the existing protected signing key when renaming the application. Version 3.6.11 uses the LootLazy signature domain; earlier installations require a one-time manual upgrade.
