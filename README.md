---
description: "Self-use Tauri 2 desktop shell that runs the dsh Web UI from a bundled runtime on Windows x64, with an Import Runtime update path."
---

# DeepSeek Harness Tauri shell

English | [中文](README.zh.md)

## Summary

This application is a small Tauri 2 shell around the dsh Web UI for Windows 10 and 11 x64. It starts a bundled single-file dsh runtime as a supervised sidecar, loads the tokenized `dsh web` URL in WebView2, and terminates the sidecar tree with the window. The installer is self-contained: neither this repository nor a system Node installation is required after installation.

## Build

The build scripts need PowerShell 7, a Rust toolchain, and the Tauri CLI (`cargo tauri --version` reports 2.12.6 for this application). Run from the application directory (in this repository, `apps/desktop-tauri`; in a standalone checkout, the repository root):

```powershell
pwsh scripts/build-installer.ps1
```

The script prepares the bundled payloads and then runs `cargo tauri build`:

- `prepare-runtime.ps1` uses `dist-exe/deepseek-harness-sdk-runtime-win-x64.exe` when that local build exists. Otherwise it downloads the latest public PyPI `deepseek-harness-runtime-bin` win_amd64 wheel, verifies the SHA-256 digest published by PyPI, and extracts `deepseek-harness-sdk-runtime-win-x64.exe` with its `-rg.exe` sidecar into `resources/runtime/`. Set `DSH_DESKTOP_RUNTIME_VERSION` to pin a published version.
- `prepare-node.ps1` downloads Node 24.22.3 win-x64, verifies it against the published `SHASUMS256.txt`, downloads pnpm 11.28.5, verifies its npm registry SHA-512 integrity, and creates the `pnpm.cmd` shim in `resources/pnpm/`.

The NSIS installer is written under `target/release/bundle/nsis/` in the application directory. Generated payloads, `target/`, and installer output are not committed.

## Install

Prebuilt installers are published on the [Releases page](https://github.com/sinyookkkk/deepseek-harness-desktop/releases). Building from source is only needed to produce your own installer or to refresh the seed runtime.

The per-user installer (`currentUser` mode) requires no elevation and registers an uninstall entry. It bundles the seed runtime and the Node, npm, npx, and pnpm toolchain. The WebView2 bootstrapper downloads the runtime on a machine that does not already have WebView2.

Uninstalling removes the installed application and its bundled resources. `$DSH_HOME` is not part of the installation and remains intact.

## Runtime selection and Import

Startup prefers the runtime recorded in `%LOCALAPPDATA%\ai.deepseek.harness.desktop-tauri\runtimes\current.json` and falls back to the bundled seed runtime when that record is absent or invalid. The seed is never modified.

**Import Runtime…** opens a file dialog and accepts a `deepseek-harness-sdk-runtime-win-x64.exe` whose sibling `-rg.exe` exists and whose `--version` reports a semantic version. A valid import copies both files to `runtimes/<version>/`, records that version as current, and offers a restart. The previous runtime directory stays in place. An invalid import leaves the selection unchanged. If the newly imported runtime fails its first start, the shell restores the previous runtime, or removes the record so the bundled seed is used. A successful first start marks the selection stable, so later failures are reported without changing it.

The bundled seed is refreshed by rebuilding with a newer `dist-exe` or a newer published wheel. Import is the update path for a dsh runtime the shell did not install.

## Toolchain pinning

The bundled toolchain is Node 24.22.3 with its npm and npx, plus pnpm 11.28.5. Both downloads are verified before extraction. The shell appends the bundled `resources/node` and `resources/pnpm` directories after the inherited `PATH`, so a user-installed Node keeps priority while the bundled toolchain fills the remaining entries. A `pnpm.cmd` shim runs pnpm through the bundled Node.

The agent shell inherits the sidecar environment through `childEnv`, and both Windows shell adapters are non-login (`pwsh -NoProfile`, `bash -c`), so the appended entries survive into agent shell commands.

## Configuration

The shell reads `%LOCALAPPDATA%\ai.deepseek.harness.desktop-tauri\config.toml`. Missing fields are optional; the file is created on first run.

```toml
workspace = 'C:\Users\you'
# dsh_home = 'C:\Users\you\.dsh'
# node_dir = 'C:\path\to\node'
# pnpm_dir = 'C:\path\to\pnpm'
```

`workspace` is the sidecar working directory. `dsh_home` is forwarded as `DSH_HOME`; omitting it uses the dsh default. `node_dir` and `pnpm_dir` override the bundled toolchain locations. The dedicated `desktop-tauri` profile is initialized from the shipped `web` template on first launch. Sidecar stderr is appended to `logs/sidecar.log` with token values redacted; stdout is parsed for the entry URL and never persisted.

## Known limitations

- Windows x64 only; the shell is unsigned and intended for self use.
- There is no automatic update feed, tray icon, account login, or multi-instance management. Import Runtime is the v1 runtime update mechanism.
- Restarting interrupts a running turn. Session logs stay durable, but the in-flight turn may be lost.
- The PyPI seed wheel can lag the current release; use Import Runtime for a locally built runtime.
- The `dsh web:` stdout line is a presentation contract. The shell waits up to 90 seconds, accepts only a loopback URL carrying a `token` parameter, and reports the stderr tail when startup fails.
- WebView2 is a system component. The bootstrapper needs network access on a machine that does not already have it.

## Measurements

- Installer: 96.0 MB (`DeepSeek Harness_0.2.3-alpha.2_x64-setup.exe`, 100,622,691 bytes).
- Installed footprint: 246.0 MB (257,930,686 bytes).

- Runtime update payload: 141.1 MB (`dsh.exe`, 147,907,584 bytes) plus 5.2 MB (`dsh-rg.exe`, 5,429,760 bytes); an import copies both into `runtimes/<version>/`.
- Seed runtime: 0.2.2-rc.2 from PyPI; `--version` prints `0.2.2-rc.2` and the URL line is `dsh web: http://127.0.0.1:<port>/?token=<token>`.
- Agent-shell PATH spike: `node --version`, `npm --version`, `npx --version`, and `pnpm --version` resolve from the bundled directories with no system Node on `PATH`; no installer-managed user PATH entry is needed.
