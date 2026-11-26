## Context

- The plugin market is split between a Node Host service (`packages/boot/plugin-market`), a React client plugin (`packages/client/ui-plugin-market`), an index format (`packages/util/plugin-market-index`), and a reference generator (`apps/plugin-market-generator`). The desktop Tauri shell only starts the `dsh web` sidecar; it has no command or IPC surface for the market.
- The published community index (`E:\workspace\dsh-plugin-market`) has 56 plugins and declares only sort keys, although the format and Discover UI already support categories and labels.
- `pluginManager` already supports detached installs (`installBundle({requestId})`, `waitForInstall`, `cancelInstall`), but it deletes a request as soon as it settles and only exposes the active map.
- `manager-store.closeInstall` currently hides the dialog and then calls `cancelInstall` for any pending phase; the dialog is mounted only inside the Plugins panel, and `openInstall` does not switch panels.
- The client has one generic `notice` slot, which a refresh failure can replace.
- `getInstall` is a new Remote, so the liveshare endpoint classification, the `api/remotes` type export, and the generated cordis catalog must change with it.

## Goals / Non-Goals

**Goals:**

- Make Discover show 8 categories plus All and filter by them, with labels for cross-cutting attributes, without a runtime update.
- Keep an install running when its dialog is closed; expose a global task bar and dedicated result notifications.
- Start market installs in the background; open the dialog only for check failures or an explicit View.
- Restore a Host-owned task or its recent result after a page refresh.
- Preserve the existing trust confirmation, pinned version and source display, registry selection, explicit cancellation, and build-script approval.

**Non-Goals:**

- Rust rewrite of the market, Host, or Tauri shell.
- Recovering an install after the sidecar exits.
- Cross-tab task synchronization.
- Replaying the pre-refresh terminal log.
- Upstream generator category flags; categories are added by the market-side post-processor.
- Auto-approving install scripts.
- Desktop tray, automatic update feeds, account login, and other shell capabilities.

## Decisions

### D1. Categories are index data, applied by a post-processor

The existing generator validates the base index and must not be forked for this change. A new `categories.json` declares the category/label titles, maps every package name to a category, and lists the only packages allowed in `other`. `scripts/apply-market-metadata.mjs` injects the metadata after generation, validates every reference, and fails when a plugin has no explicit mapping. Stale mappings (a name no longer in the index) only warn. `scripts/build.sh` is the single post-processing execution point; the daily workflow runs `node --test` before committing or publishing.

Alternative: extend the upstream generator. Rejected for v1 because the market CI would have to wait for an upstream release and the index format already supports the fields.

### D2. No Rust rewrite

The market UI is rendered by the WebView React client; the Host is Node; the Tauri shell intentionally exposes no command/IPC surface. A native Rust market would duplicate the UI and the install/profile state machine, and would not work in browser mode. The performance cost is network and pnpm, not JavaScript.

### D3. `taskId` is the task identity; `busy` is the concurrency gate

`InstallState` gains `taskId`, created when the pre-install check starts. A derived `busy = checking || isInstallPending(phase)` decides whether a second install may start. `taskId` only decides whether a task bar/result card exists. `dismissInstallTask()` clears a settled result; `abandonTask()` clears a failed check while keeping the spec and input error. `runInstall` uses `busy`, so a failed screen can retry.

### D4. Closing hides; explicit Cancel stops

`closeInstall` only hides while `busy`; for a settled result it calls `dismissInstallTask`. The dialog keeps its explicit Cancel button and back arrow. The X/Escape/backdrop labels become “hide task”.

### D5. Market installs start in the background

`installFrom` starts a task with `open:false` after the Discover confirmation. Check failures set `open:true` and preserve the spec/error with `abandonTask`. Execution failures and build approval stay in the task bar and notification, without stealing focus.

### D6. Global task bar and dedicated install notice

Two `shell.overlay` components are added: a task bar (top-center, below the existing toast) and an install-result toast. The task bar shows checking/installing/failed/unknown, with actions by state. A new `installNotice` state slot carries install results so that `notice` (refresh failures, enable/uninstall outcomes) cannot replace them.

### D7. Host task query and bounded recent results

`InstallControl` records `spec`, `startedAt`, and a `cancelling` phase. `pluginManager.getInstall(requestId)` returns a discriminated union: active task with phase, recently settled task with result, or null. Settled results are kept in a Map of at most 8 entries for 10 minutes; rejected runs are not stored. `waitForInstall` keeps its current active-only semantics.

### D8. Refresh recovery uses credential-free session state

`sessionStorage` stores `taskId`, `requestId`, `startedAt`, a display name, and the confirmed title/version/selection/saveExact flags. It never stores the raw spec. On access, `restoreInstallTask()` queries `getInstall`, maps Host `installing` to client `running`, restores a settled result with `restored:true` (skipping analytics, applied-state changes, highlight, and toggle tracking), and handles the wait-null race by querying again. A wait RPC failure also queries again; if that fails, the task stays `unconfirmed` (busy, cancellable) instead of becoming `unknown`. The initial `getInstall` failure keeps the record and retries once; an old Host without `getInstall` clears it. A restored spec is displayed through `sanitizeInstallInput`, and a task restored without any new log says the pre-refresh output was not retained. A refresh before the Host task exists reports an interrupted check.

### D9. View switches panels

`viewInstallTask` selects the Plugins panel first and then opens the dialog; overlays never call `openInstall`, which is the analytics-bearing Add action. The Plugins toolbar uses the same non-dismissing view action.

### D10. Remote and documentation artifacts ship together

The new Remote requires the liveshare endpoint classification, the `api/remotes` type export, and a regenerated cordis catalog. The slot contract, both package READMEs (EN/ZH), and the browser overlay test are updated in the same change.

## Risks / Trade-offs

- A new plugin without a mapping fails the daily index build. This is intentional: the market owner must classify it; stale mappings stay warnings so removals do not freeze the build.
- The task identity state machine is the riskiest client change; it gets dedicated store tests for close/hide, second install, retry, and every restored phase.
- The task bar and install notice are new global UI; tests must cover panel switching and overlay ordering.
- `getInstall` retains install output for a short time; the cache is bounded by count and TTL, and rejected runs are excluded.
- A refresh during the pre-install check cannot resume the check; the user is told and starts again.
- The client change needs a new runtime and a rebuilt desktop installer (or an Import Runtime update); the index change does not. This release step is required for the change to reach desktop users.

## Migration Plan

- Ship the index change independently; older clients ignore the new metadata.
- Ship the client change in a new DSH runtime and rebuild the desktop installer (or use Import Runtime). An older Host without `getInstall` is treated as “recovery unsupported”; the saved record is cleared.
- `taskId`/`restored`/`installNotice` are UI state only; no session or profile format change.
- The market build adds `package.json` and a test step; existing `build.sh` behavior is preserved except for the post-processing step.
