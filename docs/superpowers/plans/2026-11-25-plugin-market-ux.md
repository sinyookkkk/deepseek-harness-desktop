# 2026-11-25 插件市场分类与后台安装 — 实施计划

- 输入：Phase 0 设计、OpenSpec proposal/design/specs/tasks
- 原则：每个任务先写失败测试，再写最小实现，再跑相关测试；提交只在用户确认后进行。

## Task 1 — 市场分类元数据

文件：
- `E:\workspace\dsh-plugin-market\categories.json`
- `E:\workspace\dsh-plugin-market\scripts\apply-market-metadata.mjs`
- `E:\workspace\dsh-plugin-market\scripts\apply-market-metadata.test.mjs`
- `E:\workspace\dsh-plugin-market\package.json`
- `E:\workspace\dsh-plugin-market\scripts\build.sh`
- `E:\workspace\dsh-plugin-market\.github\workflows\build-market.yml`
- `E:\workspace\dsh-plugin-market\README.md`

TDD：
1. RED：写测试，覆盖注入、保留 sortKeys/keywords、缺失映射失败、未声明 id 失败、非白名单 other 失败、空分类失败、陈旧映射告警、确定性输出。
2. 运行 `node --test scripts/*.test.mjs` 确认失败。
3. GREEN：实现 `apply-market-metadata.mjs` 与 `categories.json`（56 条映射，8 分类，4 标签，`otherAllow`）。
4. 运行测试与真实索引：`node scripts/apply-market-metadata.mjs public/index.json categories.json`。
5. REFACTOR：抽出 `validate`/`apply`/`write`，保持无依赖。
6. 更新 build.sh/workflow/README，确保 CI 先测试后提交。

验收：`public/index.json` 100% 插件有分类；非 other 分类非空；other 仅 `otherAllow`；重复运行零 diff。

## Task 2 — Host `getInstall`

文件：
- `packages/boot/plugin-manager/src/types.ts`
- `packages/boot/plugin-manager/src/index.ts`
- `packages/boot/plugin-manager/tests/manager.spec.ts`
- `packages/experimental/liveshare/src/endpoint-classification.ts`
- `packages/experimental/liveshare/src/client/endpoint-classification.ts`
- `packages/api/remotes/src/client/index.ts`
- `docs/subsystems/boot.md` / `boot.zh.md` / `tool-cordis/src/api-catalog.ts`（生成）

TDD：
1. RED：Host 测试——活动任务返回 spec/startedAt/phase；结算后在 TTL 内返回 result；超 TTL/超 8 条/未知/reject 返回 null；cancel 后 phase 为 cancelling。
2. GREEN：`InstallControl` 加 spec/startedAt/cancelling；`recentInstalls` 8 条/10 分钟；`getInstall` Remote。
3. 更新 liveshare 分类、api/remotes 导出；`pnpm gen-cordis-catalog`；`pnpm verify-cordis-catalog`。
4. 运行 `pnpm vitest run packages/boot/plugin-manager` 与类型检查。

## Task 3 — 客户端任务模型

文件：
- `packages/client/ui-plugin-manager/src/client/manager-store.ts`
- `packages/client/ui-plugin-manager/tests/manager-store.client.spec.ts`
- `packages/client/ui-plugin-manager/src/client/slot-contract.ts`

TDD：
1. RED：close 在 busy 相位只隐藏不取消；settled close 走 dismiss；failed 不阻塞 retry/new；已有 busy 任务时 installFrom/openInstall/toolbar 只聚焦；checking 点查看不丢检查；检查失败保留 spec/inputError。
2. GREEN：`taskId`/`busy`/`dismissInstallTask`/`abandonTask`；改 `closeInstall`/`openDialog`/`focusInstall`/`installFrom`/`openCatalogInstall`/`runInstall`；`installFrom` 后台启动；`installNotice` 独立；`viewInstallTask`。
3. 更新 slot contract 文案。
4. 运行 store 测试与类型检查。

## Task 4 — 全局任务条与通知

文件：
- 新增 `InstallTaskPill.tsx` + CSS
- 新增 `InstallNoticeToast.tsx`
- `packages/client/ui-plugin-manager/src/client/index.ts`
- `packages/client/ui-plugin-manager/src/client/locales.ts`
- `packages/client/ui-plugin-manager/tests/components.client.spec.tsx`
- `packages/client/ui-plugin-manager/tests/browser-plugin.client.spec.tsx`

TDD：
1. RED：任务条在 checking/running/failed/approval 显示，动作矩阵正确；通知 View 切面板；不发 add-button analytics；overlay 顺序断言更新。
2. GREEN：实现组件、注册 overlay、`viewInstallTask` 切面板、8 语言键。
3. 运行组件测试。

## Task 5 — 刷新恢复

文件：
- `manager-store.ts`、`manager-store.client.spec.ts`、`PluginManagerPage.tsx`、locales。

TDD：
1. RED：写入/更新/清除记录；恢复 active（installing→running）；恢复 settled 跳过 analytics/applied/highlight/toggle；checking 刷新提示中断；初始 getInstall 网络失败保留并重试一次；旧 Host 清理；wait null 回退；wait RPC 失败 → unconfirmed；恢复 spec 脱敏；无日志提示只在 restored 且无 chunk 时出现。
2. GREEN：sessionStorage、`restoreInstallTask`、`settleInstall(restored)`、页面提示。
3. 运行 store/组件测试。

## Task 6 — 文档与契约

- 更新 `ui-plugin-manager`、`boot/plugin-manager`、`ui-plugin-market` 的 README/README.zh。
- 更新 `browser-plugin` 测试、cordis catalog。
- `pnpm typecheck`、相关 vitest、`pnpm verify-cordis-catalog`。

## Task 7 — 运行时/桌面验证（必须）

- `pnpm build:artifacts`；`dev:web` 手动验证：正常安装、关闭/取消、失败/审批、切面板、三种刷新。
- 构建运行时，走 Import Runtime 或重建桌面安装包，在安装版重复验证。
- 记录结果，更新桌面 README 已知限制。

## Task 8 — 不兼容插件过滤（追加需求）

文件：
- `packages/boot/plugin-market/src/{types,index,tools}.ts`
- `packages/boot/plugin-market/tests/{market,tools}.spec.ts`
- `packages/boot/plugin-market/README.md`、`README.zh.md`
- `packages/client/ui-plugin-market/src/client/{presentation,market-store,DiscoverSection,locales}.ts(x)`、`DiscoverSection.module.css`
- `packages/client/ui-plugin-market/tests/{market-store.client.spec.ts,discover.client.spec.tsx}`
- `packages/client/ui-plugin-market/README.md`、`README.zh.md`
- `packages/extensions/tool-cordis/src/api-catalog.ts`（生成物）

TDD：
1. RED（Host）：默认过滤全不兼容插件；`incompatible` 计数；部分兼容/有豁免/无 engines 保留；过滤先于分页，`total`/`nextOffset` 只算可见条目；工具输出带计数。
2. GREEN（Host）：`MarketQuery.includeIncompatible`、`MarketSearchResult.incompatible`、`search()` 分页前过滤、工具计数。
3. RED（客户端）：无隐藏时不显示提示条；有隐藏时显示数量；点“显示”后查询带 `includeIncompatible`；显示时安装仍禁用；`total === 0` 时提示条仍在；搜索/分类/排序切换保留显示状态。
4. GREEN（客户端）：`DiscoverQuery.includeIncompatible`、`page()` 传参、提示条与开关、8 语言 4 个键。
5. 生成 `pnpm gen-cordis-catalog`，跑 `verify-cordis-catalog`、`pnpm typecheck`、聚焦 vitest。
6. 与 Task 7 一起重建客户端 bundle/运行时，在桌面版验证提示条与开关。

判定口径：`compatibility.compatible || compatibility.exempted` 视为可安装；未声明 `engines.dsh` 视为任意版本可用；平台字段本轮不参与。
