## Why

The plugin market is hard to browse because the published index declares no categories, and installing a plugin is tied to a modal: closing the dialog cancels the run, and a page refresh loses any visible progress. The market itself cannot move to Rust without breaking the shared Web/Desktop client and the Node install pipeline, so the improvement belongs in the index and the existing client.

## What Changes

- Publish category and label metadata in the `dsh-plugin-market` index:
  - add `categories.json` with 8 categories, 4 labels, an explicit package mapping, and an `otherAllow` whitelist;
  - add a post-processing script that injects the metadata after the existing generator and fails when a plugin has no explicit mapping;
  - run the post-processor from `scripts/build.sh` and its tests in the daily GitHub Actions build.
- Decouple installation from the dialog:
  - closing the install dialog hides it instead of cancelling; an explicit Cancel action still stops the run;
  - show a global task bar across panels while a check or install is running, with View/Cancel;
  - show completion, failure, cancellation, and build-approval-needed notices in a dedicated install-notice channel.
- Start market installs in the background: the Discover confirmation is the only dialog on the normal path; check failures open the install dialog, while execution failures and build approval stay in the task bar/notification.
- Recover install tasks after a page refresh:
  - add `pluginManager.getInstall(requestId)` with active tasks and a short-lived recent-result cache;
  - persist a credential-free task record in `sessionStorage`;
  - restore running tasks, or a just-finished result; display a sanitized spec and say when pre-refresh logs were not retained; a refresh during the pre-install check reports that the check was interrupted.
- Preserve existing safety behavior: pinned version, source display, trust confirmation, explicit cancellation, and build-script approval.
- Filter plugins that cannot run on the current DSH:
  - Host `pluginMarket.search` hides entries with no installable version (no compatible and no exempted version) before paging, so `total` and `nextOffset` count only visible entries;
  - add optional `includeIncompatible` to `MarketQuery` and an `incompatible` count to `MarketSearchResult`, so the client can show “N hidden” and reveal them on request;
  - show a footer note and toggle in Discover; revealed entries keep the “Incompatible” tag and a disabled Install action;
  - report the same count in the `plugin_market` tool's `list_plugins` output.
- Do not rewrite the market in Rust. The market UI is a WebView React plugin and the Host is a Node service; the Tauri shell has no command/IPC surface for it.

## Capabilities

### New Capabilities

- `plugin-market/categories`: index-declared categories, labels, per-plugin mapping, and the build gate that keeps the mapping complete.
- `plugin-market/compatibility-filter`: default filtering of plugins with no installable version, the visible hidden count, and the explicit reveal switch.
- `plugin-manager/background-install`: install tasks that outlive the dialog, global task feedback, dedicated install notices, and refresh recovery through the Host task query.

### Modified Capabilities

None.

## Impact

- `E:\workspace\dsh-plugin-market`: new `categories.json`, `scripts/apply-market-metadata.mjs`, its tests, `package.json`, `scripts/build.sh`, the daily workflow, and the README.
- `E:\workspace\deepseek-harness`:
  - Host `packages/boot/plugin-manager`: `getInstall`, recent-result cache, `cancelling` phase, types.
  - Host `packages/boot/plugin-market`: `includeIncompatible` query field, `incompatible` result count, paging-time compatibility filter, tool count, tests, README.
  - Client `packages/client/ui-plugin-manager`: task state, close/hide semantics, background start, task bar, install notice, refresh recovery, locales, tests.
  - Client `packages/client/ui-plugin-market`: `includeIncompatible` query, hidden-count note and reveal switch, 8-language copy, tests, README.
  - Contract/docs: `slot-contract.ts`, package READMEs (EN/ZH), liveshare endpoint classification, `api/remotes` type export, `docs/subsystems/boot.md` + `boot.zh.md`, `tool-cordis/src/api-catalog.ts`.
- `E:\workspace\deepseek-harness-desktop`: rebuild the runtime and desktop installer (or use Import Runtime) for the client change; the index change needs no runtime update.
- No Rust code, no new npm runtime dependency, and no session or profile format change.
- `pluginManager/getInstall` is an additive Remote; the new client treats an older Host without it as “recovery unsupported” and clears the saved record.
