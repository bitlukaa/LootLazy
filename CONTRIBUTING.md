# Contributing to Questro

Bug reports, usability feedback, and focused improvements are welcome.

## Report an issue

Include your Questro version, Windows version, steps to reproduce the problem, and what you expected to happen. Screenshots are helpful. Remove personal information before sharing logs or screenshots.

Never include passwords, account tokens, payment details, or private account data. For security issues, follow [SECURITY.md](SECURITY.md).

## Suggest a feature

Describe the problem it solves and how it fits the existing interface. Questro focuses on local game sessions and manual quest controls; proposals should preserve that approach.

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
