# Clean Windows installer check

Build and package the release, then run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File tools/prepare-clean-install.ps1
```

This prepares data/clean-install-check/LootLazy.wsb and a test payload. It does not install anything on the host. The sandbox reads only the staged installer and expected executable hash; its result folder is the only writable host mapping. Networking allows the installer to obtain Microsoft's WebView2 runtime when needed. No account data or private keys are mapped.

On a Windows computer with Windows Sandbox available, use the same command with -Run or open the generated .wsb file. The guest installs LootLazy silently, compares the installed executable hash, checks its version and backend in a fresh profile, and checks that normal startup remains running. The result is written to data/clean-install-check/results/result.json. A passing report is required before claiming a clean Windows installation was tested.

Windows Sandbox is unavailable on the development machine; this plan has only been prepared and syntax-checked. Live Discord or Steam account behavior is outside this test.
