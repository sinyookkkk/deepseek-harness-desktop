## 1. Market index categories

- [ ] 1.1 Add `categories.json` to `dsh-plugin-market` with 8 category declarations, 4 labels, `otherAllow`, and an explicit mapping for all 56 current plugin names; verify every name appears in `public/index.json`
- [ ] 1.2 Add `scripts/apply-market-metadata.mjs` that merges categories/labels, injects per-plugin `metadata.category`/`metadata.labels`, preserves keywords/sortKeys, and writes deterministic JSON
- [ ] 1.3 Add failure checks for a missing mapping, undeclared id, non-whitelisted `other`, and an empty non-other category; add a warning-only path for stale mappings
- [ ] 1.4 Add `package.json` with `node --test scripts/*.test.mjs` and a test file covering injection, preservation, failure modes, stale warning, and deterministic output
- [ ] 1.7 Run the script and tests against the real `public/index.json`; verify 100% of plugins have a category, every non-other category is non-empty, `other` is within `otherAllow`, and category counts are printed
- [ ] 1.8 Update `scripts/build.sh`, `.github/workflows/build-market.yml` paths/steps, and `README.md`; verify the workflow runs the test and publishes only on success
- [ ] 1.9 Publish the index to GitHub Pages and verify Discover shows 8 category pills plus All, filters by category, filters by labels, and keeps search and sort working

## 2. Host task query

- [ ] 2.1 Write Host tests for `getInstall` before implementation: active task returns spec/startedAt/phase; settled task returns result within TTL; unknown/expired returns null; rejected run is not stored
- [ ] 2.2 Extend `InstallControl` with `spec`, `startedAt`, and `cancelling`; set `cancelling` in `cancelInstall` before aborting
- [ ] 2.3 Add a bounded `recentInstalls` map (8 entries, 10 minutes) and retain a settled result without changing `waitForInstall` semantics
- [ ] 2.4 Implement `@Remote getInstall(requestId)` returning the discriminated union and pruning expired entries
- [ ] 2.5 Add the liveshare host/client endpoint classification rows, export `PluginInstallTask` from `api/remotes/client`, and run `pnpm gen-cordis-catalog`; verify `pnpm verify-cordis-catalog`

## 3. Client task model

- [ ] 3.1 Add store tests for `taskId`/`busy`: close during checking/starting/running/cancelling/applying/unconfirmed hides without calling `cancelInstall`; settled close dismisses; a failed task does not block retry or a new install
- [ ] 3.2 Add `taskId`, `restored`, and `installNotice` to the state; implement `dismissInstallTask` and `abandonTask`
- [ ] 3.3 Change `closeInstall`, `openDialog`, `focusInstall`, `installFrom`, `openCatalogInstall`, and `runInstall` to the `busy`/`taskId` split; keep failed retry working
- [ ] 3.4 Start market installs with `open:false`; on check failure set `open:true` and preserve spec/inputError
- [ ] 3.5 Write `installNotice` instead of the shared `notice`; add a `cancelled` install outcome; exclude install notices from the page toast; verify a refresh failure cannot replace it
- [ ] 3.6 Implement `viewInstallTask` (select Plugins panel, then open without analytics or dismissing the result) and update the Plugins toolbar to use it

## 4. Global feedback

- [ ] 4.1 Add `InstallTaskPill` with checking/installing/failed/unknown and approval-needed states, the active=View+Cancel and failed/unknown=View+Close action matrix; place it top-center below the toast
- [ ] 4.2 Add `InstallNoticeToast` reading `installNotice` with a View action and dismissal
- [ ] 4.3 Register both overlays in `index.ts` and update the overlay-order/injection assertions in `browser-plugin.client.spec.tsx`
- [ ] 4.4 Add the 8-language keys `installHide`, `installTaskChecking`, `installTaskFailed`, `installTaskNeedsApproval`, `installTaskUnknown`, `installTaskDismiss`, `installRestoredNoLogs`, `installBackgroundCancelled`, `installCheckInterrupted`; remove or repurpose `installCloseCancels`; verify the locale types compile
- [ ] 4.5 Add component tests for the pill, notice, approval state, panel switching, and the absence of the Add-button analytics event on View

## 5. Refresh recovery

- [ ] 5.1 Add store tests for session write/update/clear, active restore, settled restore, checking-interrupted notice, initial-query failure, old-Host cleanup, wait-null/wait-failure, and no overwrite of a newer task
- [ ] 5.2 Persist the credential-free record at check start and add `requestId` after `startInstall`; clear on done/cancelled/dismiss, retain on failed/unknown until dismiss or TTL expiry; on retry reuse `taskId` and update the record
- [ ] 5.3 Implement `restoreInstallTask()`: read the record, query `getInstall`, keep the record and retry once on a network failure, clear it on an old-Host/unknown-method error, map `installing` to `running`, restore the subject/confirmed flags, sanitize the displayed spec, and set `restored:true`
- [ ] 5.4 Handle wait-null and wait-failure races by querying `getInstall` again: null twice becomes unknown; an RPC failure that still fails becomes `unconfirmed` (busy, cancellable), never a startable second task
- [ ] 5.5 Make `settleInstall(result, {restored:true})` skip analytics, `applied`, highlight, and toggle tracking; keep the normal path unchanged
- [ ] 5.6 Show “pre-refresh output was not retained” only when `restored` and no log has arrived yet; clear the flag on the first chunk or settlement
- [ ] 5.7 Add tests proving a restored result does not emit a second analytics event or highlight, a sanitized spec is displayed, an initial RPC failure keeps the record and a retry is possible, and a normal new install does not show the restored-log message

## 6. Contracts and documentation

- [ ] 6.1 Update `slot-contract.ts` to describe a background install task and document the optional View path
- [ ] 6.2 Update `ui-plugin-manager`, `boot/plugin-manager`, and `ui-plugin-market` READMEs in English and Chinese
- [ ] 6.3 Verify `pnpm typecheck`, focused vitest suites, and `pnpm verify-cordis-catalog`

## 7. Runtime and desktop verification

- [ ] 7.1 Run `pnpm build:artifacts` and launch `dev:web`; manually verify normal install, hide/cancel, failure/approval, panel switch, and all three refresh scenarios
- [ ] 7.2 Build the runtime and desktop installer (or use Import Runtime), then repeat the manual verification in the installed app
- [ ] 7.3 Record measured results and update the desktop README known-limitations section

## 8. Compatibility filter (added requirement)

- [ ] 8.1 Write Host tests first: default search hides a plugin whose every version is incompatible; `incompatible` counts it; a partly compatible plugin stays; an exempted plugin stays; a plugin without `engines.dsh` stays
- [ ] 8.2 Write Host paging tests: filter runs before offset/limit, `total`/`nextOffset` count visible entries, next page is contiguous
- [ ] 8.3 Add `MarketQuery.includeIncompatible` and `MarketSearchResult.incompatible`; implement the pre-paging filter with the existing `evaluatePluginCompatibility`; keep annotating only the returned page
- [ ] 8.4 Include the `incompatible` count in the `plugin_market` tool's `list_plugins` output; update its test
- [ ] 8.5 Write client tests: no note without hidden entries; hidden note with count; reveal sends `includeIncompatible`; revealed entries keep the disabled Install; empty result still shows the note; query changes keep the reveal
- [ ] 8.6 Add `DiscoverQuery.includeIncompatible`, pass it through `market-store.page()`, render the note/toggle, and add the 4 keys in 8 languages
- [ ] 8.7 Update `ui-plugin-market` and `boot/plugin-market` READMEs EN/ZH, run `pnpm gen-cordis-catalog`, and verify `pnpm verify-cordis-catalog`
- [ ] 8.8 Verify `pnpm typecheck` and the focused Host and client vitest suites
- [ ] 8.9 Rebuild the client bundle and runtime (with 7.2), then verify the note, reveal, and disabled Install in the desktop app
