# 2026-11-25 插件市场分类与后台安装 — 设计

- 状态：Phase 0 设计稿 v2（已按 Gate 0 三路审查修订，待复审）
- 模式：medium（约 18 文件）
- 关联需求：[clarifications/2026-11-25-plugin-market-ux.md](../clarifications/2026-11-25-plugin-market-ux.md)
- 涉及仓库：
  - `E:\workspace\dsh-plugin-market`（索引内容与发布）
  - `E:\workspace\deepseek-harness`（Host 与 Web 客户端）
  - `E:\workspace\deepseek-harness-desktop`（运行时与桌面安装包，发布用）

## 1. 概述

本设计交付两个能力：

1. **市场分类**：给 `dsh-plugin-market` 的索引增加分类声明与每个插件的分类，让 DSH “插件 → 发现”页出现分类 pills、标签筛选和对应过滤结果。
2. **后台安装**：把安装任务从“弹窗的生命周期”中解耦。关闭弹窗只隐藏，不取消；任务在后台继续，跨面板有任务指示，完成/失败有通知；刷新页面后能恢复 Host 任务的进度与结果；只有显式“取消安装”才停止。

Rust 重写不在本设计内。桌面壳没有注册任何 Tauri command（`withGlobalTauri:false`，只加载 splash 并启动 sidecar），Rust 侧没有可扩展的市场 IPC 面；市场 UI 是 WebView 中的 React 客户端插件，Host 是 Node 服务。改 Rust 无法影响市场本体，反而会破坏浏览器/桌面共用与现有安装语义。

## 2. 目标与非目标

### 目标

- G1：`public/index.json` 声明 8 个分类和 4 个标签；索引中的每个插件都有显式映射的 `metadata.category`，不能靠“全部落到其他”过验收。
- G2：Discover 显示 8 个分类 pill + “全部”，点击后按分类过滤；标签多选、搜索、排序保持可用。
- G3：安装任务开始后（含 checking 阶段），关闭弹窗只隐藏；跨面板显示后台任务；只有显式“取消安装”才中止。
- G4：Discover 确认后直接在后台安装，正常路径不再打开第二个安装弹窗；检查失败自动打开详情，安装执行失败/待构建脚本审批通过任务条与通知呈现，点击“查看”打开详情。
- G5：保持信任确认、固定版本、来源展示、构建脚本审批、取消确认等安全语义不变。
- G6：分类改动不要求用户更新 DSH 运行时；后台安装随新运行时发布。
- G7：页面刷新后，已创建 Host 任务的安装恢复任务条、阶段和完成/失败结果；检查阶段刷新会明确提示“检查已中断，请重新发起”，不静默丢失。

### 非目标

- 市场 UI 或 Host 服务的 Rust 重写。
- 应用退出后的安装恢复；sidecar 退出会中断安装，这是现有生命周期。
- 多浏览器标签页之间的任务同步；Host 的文件锁会串行化安装，但 v1 不做跨标签页 UI 合并。
- 上游 `apps/plugin-market-generator` 的原生分类参数；v1 用索引侧后处理。
- 自动批准安装脚本。
- 桌面托盘、自动更新、账号等壳能力。
- 刷新后的历史终端输出回放（默认不回放；新输出继续追加）。

## 3. 现状与约束

- 索引格式 `@deepseek-ai/dsh-plugin-market-index` 已支持根 `metadata.categories` / `metadata.labels` / `metadata.sortKeys`，以及插件 `metadata.category`（单选）/ `metadata.labels`（多选）。
- `ui-plugin-market` 已渲染分类 pills、标签菜单、排序；只有索引声明分类时才显示。
- `dsh-plugin-market/public/index.json` 当前有 56 个插件，根 `metadata` 只有两个 `sortKeys`，没有任何分类或标签。
- `plugin-market-generator` 没有分类参数；每日 CI 会重新生成并覆盖 `public/index.json`，不能手改。
- Host `plugin-manager` 已支持分离任务：`installBundle(spec,{requestId})`、`waitForInstall(requestId)`、`cancelInstall(requestId)`；只保留活动请求，完成后结果不保留。
- `InstallControl` 目前只有 `abort` / `phase` / `result`，`phase` 只有 `installing|applying`；`cancelInstall` 只读 `applying` 判断 too-late。
- `manager-store.closeInstall` 在 `isInstallPending(phase)` 时先 `open:false` 再 `cancelInstall()`，所以关闭弹窗 = 取消。
- `openDialog()` 用 `install.requestId === undefined` 判断“新开”；checking 阶段还没有 requestId，会重置状态，必须引入独立任务标识。
- `InstallDialog` 只挂在 `PluginManagerPage` 内；`openInstall` 只改 `open`，不切面板，跨面板“查看”需要先 `layout.selectPanel(PANEL_ID)`。
- `notice` 是单槽，`settleRefresh` 会用 `refresh-failed` 覆盖它；安装完成通知不能复用这个槽。
- 运行包版本为 `0.2.3-alpha.2`；当前源码领先 tag 7 个提交，分类 UI 已包含在该版本。
- 桌面壳 `withGlobalTauri:false`，没有 Tauri command；全局 UI 只能放在 Web 客户端的 `shell.overlay`。
- 新增 Remote 会牵动 `liveshare` 端点分类、`api/remotes` 类型导出和 `gen-cordis-catalog` 生成物。
- `slot-contract.ts` 目前写明安装会打开 Plugins 安装弹窗；改为后台启动后需同步契约文字。
- 市场仓库没有 `package.json`/测试运行器；新增脚本测试需要补最小载体。

## 4. 方案 A：索引分类

### 4.1 数据模型

在 `public/index.json` 中注入：

```json
"metadata": {
  "categories": {
    "essentials": { "title": { "zh": "必备基础", "en": "Essentials" } }
  },
  "labels": {
    "vision": { "title": { "zh": "视觉", "en": "Vision" } },
    "local": { "title": { "zh": "需本机环境", "en": "Local runtime" } }
  },
  "sortKeys": { "github_stars": { "title": { "zh": "GitHub 星标数", "en": "GitHub stars" }, "order": "desc" } }
}
```

每个插件：

```json
"metadata": {
  "category": "essentials",
  "labels": ["vision"],
  "keywords": ["..."],
  "sortKeys": { "github_stars": 8327 }
}
```

约束：

- `category` 只能引用根声明过的 id；一个插件只有一个分类。
- `labels` 只能引用根声明过的 id；可多个，用于跨切面属性。
- 没有隐式兜底：索引中的每个插件名都必须出现在 `categories.json` 的 `plugins` 映射中；缺失即构建失败，列出缺失名单。
- `other` 只用于显式登记为“其他”的插件，不允许当作漏映射的垃圾桶。
- 分类/标签的显示顺序由 UI 按本地化标题排序，格式不提供 order 字段。

### 4.2 分类体系（已随 Gate 0 批准）

| id | 中文标题 | 英文标题 | 覆盖示例 |
|---|---|---|---|
| `essentials` | 必备基础 | Essentials | billion-context、dsh-cost-meter、dsh-context |
| `capabilities` | 能力增强 | Capabilities | 视觉/OCR、搜索、记忆、文件引用、安全拦截、工具目录 |
| `interface` | 界面效率 | Interface | 侧边栏、Web 全家桶、PPT、对话分支、架构图 |
| `development` | 开发工作流 | Development | LSP、Git、代码审查、TDD、OpenSpec/Superpowers、VSCode 模式 |
| `graphics` | 绘图设计 | Graphics | 草图、白板、drawio、Mermaid、Excalidraw、文生图 |
| `modeling` | 3D 建模 | 3D & Modeling | Blender、CAD、USD 预览、分子查看、GenUI 3D |
| `fun` | 趣味美化 | Fun | 桌面宠物、余额挂件、表情包、主题、PUA |
| `other` | 其他 | Other | 无法归入以上分类且经显式登记的插件 |

标签 4 个：

- `vision`（视觉）：识图/OCR/视觉路由。
- `search`（搜索）：网页搜索/聚合搜索/语义检索。
- `memory`（记忆）：长期记忆/上下文记忆。
- `local`（需本机环境）：需要本机应用或本地服务（Blender、CAD、ComfyUI、Wallpaper Engine 等）。

映射原则：

- 按插件的主要用途选唯一分类，不按 README 的来源章节照搬。
- 跨切面属性放标签。
- 56 个插件逐条登记；`other` 最多保留给明确无法归类的少数插件（如 `huashu-nuwa`），验收时列出清单。

### 4.3 文件与流水线

新增：

- `categories.json`
  - `categories`：id → `{ title: { zh, en } }`
  - `labels`：id → `{ title: { zh, en } }`
  - `plugins`：npm 包名 → `{ category, labels? }`；必须覆盖索引中的每个插件名。
  - `otherAllow`：允许归入 `other` 的包名白名单数组；不在白名单里的插件不得使用 `other`。
  - 不再提供 `defaultCategory`；缺失不是“回退”而是“构建失败”。

- `scripts/apply-market-metadata.mjs`
  - 读取 `public/index.json` 与 `categories.json`。
  - 合并根 `metadata.categories` / `metadata.labels`，保留原有 `sortKeys`。
  - 为每个插件补 `metadata.category` 与 `metadata.labels`，保留 `keywords` / `sortKeys`。
  - 结构化校验：分类 id 已声明、标签 id 已声明、映射里的包名都存在、索引中每个插件都有映射、`other` 只出现在 `otherAllow` 中、每个非 other 分类至少有一个插件。
  - 映射里存在但索引中不存在的包名（插件改名/下架）只告警，不阻塞构建。
  - 以 2 空格缩进、末尾换行、稳定字段顺序写回；内容未变则不写文件。
  - 退出码：0 成功（含仅有陈旧映射告警）；1 数据错误（缺失映射、未声明 id、非白名单 other、空分类）；2 文件缺失/JSON 非法。
  - 不承诺检测 JSON 重复键（`JSON.parse` 后已无法还原）；重复键由编辑规范避免。

- `package.json`（市场仓库，最小载体）
  - `"private": true`，`"scripts": { "test": "node --test scripts/*.test.mjs" }`。
  - 不引入运行时依赖，只用 Node 内置 `node:test` 与 `assert`。

- `scripts/apply-market-metadata.test.mjs`
  - 正常注入；保留 sortKeys/keywords；缺失映射报错；未声明 id 报错；陈旧映射告警；确定性输出。

更新：

- `scripts/build.sh`：generator 成功后执行 `node scripts/apply-market-metadata.mjs public/index.json categories.json`。
- `.github/workflows/build-market.yml`：`paths` 增加 `categories.json`、`package.json`、脚本与测试文件；构建后跑 `node --test scripts/*.test.mjs`。
- `README.md`：增加“分类与标签”维护说明；说明缺失映射会让构建失败，这是有意的质量门。

分类校验不做“调用 harness 里的格式包”这一步，避免依赖 `.dsh-harness` 未构建的 `lib/`；格式本身已由 generator 在注入前校验，注入只添加已声明 id 的结构化字段，脚本再做一次结构校验即可。

### 4.4 数据流

```text
plugins.txt
  → plugin-market-generator（现有）
  → public/index.json（无分类）
  → apply-market-metadata.mjs + categories.json
  → public/index.json（含分类/标签）
  → node --test scripts/*.test.mjs
  → GitHub Pages
  → DSH Discover 刷新
```

### 4.5 错误处理

| 情况 | 行为 |
|---|---|
| `categories.json` 缺失或 JSON 非法 | 退出 2，CI 失败，不发布半成品 |
| 索引中的插件没有映射 | 退出 1，打印缺失包名清单；CI 失败 |
| 非白名单插件使用 `other`，或某个非 other 分类为空 | 退出 1，打印包名/分类 id；CI 失败 |
| 插件引用了未声明的 category/label | 退出 1，打印包名与非法 id |
| 映射里的包名在索引中不存在（插件改名/下架） | 告警并忽略，构建继续（exit 0）；不因陈旧映射冻结每日发布 |
| 索引格式本身非法 | 生成器已失败，后处理不执行 |
| 重复运行 | 输出稳定，不产生 diff |

### 4.6 测试策略

- `scripts/apply-market-metadata.test.mjs`：
  - 正常注入；保留 `sortKeys`/`keywords`；缺失映射失败；未声明 id 失败；陈旧映射只告警；确定性输出。
- 用真实 `public/index.json` 跑一次，断言：
  - 插件总数 == 有 `category` 的插件数；
  - 所有 `category`/`labels` 都已声明；
  - 使用 `other` 的插件集合 == `otherAllow` 的子集，且每个非 other 分类至少有一个插件；
  - 输出各分类计数。
- 本地用 `node` 起静态服务，把索引 URL 加到 Discover，确认分类 pills 出现、筛选生效。
- CI：`node --test` 通过后才提交/部署。

### 4.7 发布

- 只改 `dsh-plugin-market` 并推送到 `main`；GitHub Actions 重新生成并发布 Pages。
- 不需要重建 DSH 运行时；用户在 Discover 点刷新即可看到分类（旧缓存以 Host 的 `cacheTtlMs` 为准，不能只等 HTTP 缓存）。
- 分类缺失映射导致 CI 失败时，修复 `categories.json` 后重跑。

## 5. 方案 B：后台安装

### 5.1 任务模型

`InstallState` 增加/明确：

- `taskId: string | null`：**任务条/结果卡的身份**，从 checking 开始，保留到用户 dismiss、开始新任务、或 done 后清理。它**不**用作并发闩。
- `busy`（派生，不存 state）：`phase === 'checking' || isInstallPending(phase)`。所有“是否已有任务/能否启动第二个”的判断只用 `busy`；结算后 phase 不在其中，因此不会因为遗留 requestId 卡住。
- `requestId?: PluginInstallRequestId`：Host 任务标识，`startInstall` 后才有。
- `open: boolean`：安装弹窗是否可见。
- `restored: boolean`：本状态是否由刷新恢复而来，用于“刷新前输出未保留”提示与跳过重复 analytics/highlight。
- `phase`：`idle | checking | starting | running | cancelling | applying | done | failed | unconfirmed | unknown`。
- `isInstallPending(phase)`：`starting | running | cancelling | applying | unconfirmed`。
- `needsBuildApproval(state)`：`phase === 'failed' && failure.pendingBuilds?.length > 0`。
- `showsTaskPill(state)`：`taskId !== null && !open && (phase === 'checking' || isInstallPending(phase) || phase === 'failed' || phase === 'unknown')`。
- **不新增 `background` 字段**：是否显示任务条由 `taskId + open` 推导；Discover 与手动路径只差在启动时 `open` 的值。

`closeInstall()` 新语义：

- `busy` 为 true：只 `patchInstall({ open:false })`，**不调用** `cancelInstall()`。
- `busy` 为 false 且已有结算任务（`failed` / `unknown` / `done`）：调用 `dismissInstallTask()`，清除任务条/结果卡，回到空状态。
- 没有任务（`idle`）：归零。
- 已结算的 `failed`/`unknown` 在用户不打开详情时保留任务条；一旦打开详情再关闭，就视为已读并清除。任务条本身不提供会重置状态的“取消”，只提供“查看 / 关闭”。

新增 `dismissInstallTask()` / `abandonTask()`：

- `dismissInstallTask()`：清 `taskId`、`installNotice`、`sessionStorage`，回到 `IDLE_INSTALL`（保留弹窗开关按调用方决定）。任务条“关闭”、`closeInstall` 对已结算任务、开始新任务前调用。
- `abandonTask()`：只清 `taskId`、`sessionStorage`、`requestId`，**保留**当前 `spec` / `inputError` / `registryError` 与 `open:true`，用于检查失败后仍能显示原因；不重置表单。
- 任务条动作矩阵：
  - `checking` / `isInstallPending`：查看、取消。
  - `failed` / `unknown`：查看、关闭。
  - `done`：不显示任务条（由通知呈现）。

`cancelInstall()` 不变：只由显式取消按钮/返回箭头/任务条“取消”调用。checking 阶段取消会 abort inspect 并 `dismissInstallTask()`；后台取消后写 `installNotice`（cancelled）。

`openDialog()` / 新增 `focusInstall()`：

- `openDialog()`：`busy` 为 true 时只 `patchInstall({open:true})`；`busy` 为 false 时先 `dismissInstallTask()`（清掉已结算结果），再创建新状态。绝不因为 `taskId` 存在而阻止新安装。
- `focusInstall()`：只负责 `patchInstall({open:true})`，不重建 `registryRead`、不替换任务；用于任务条“查看”。
- `installFrom()`：`busy` 为 true → `focusInstall()` + `layout.selectPanel(PANEL_ID)`，不启动第二个；`busy` 为 false → `dismissInstallTask()` 后创建新任务并 `open:false` 启动。
- `openCatalogInstall()` / `openInstall()` 同样按 `busy` 判断；`busy` 为 false 时清掉旧结果再开新任务。
- `runInstall()` 入口只检查 `busy`；**failed 状态允许重试**（失败屏“重试”、GitHub 镜像恢复），重试时复用 `taskId` 并续写 sessionStorage。

`installNotice`（新增，独立于 `notice`）：

- `PluginManagerState` 增加 `installNotice: ManagerNotice | null`。
- `notifyHiddenInstall()` 写 `installNotice`，不再写通用 `notice`。
- 全局 `InstallNoticeToast` 读 `installNotice`；`dismissInstallNotice` 清空。
- 通用 `notice` 继续承载刷新失败、启用/卸载等操作，不会被安装通知覆盖。
- `noticeText()` 为 install 增加 `cancelled` 映射。

### 5.2 交互流程

#### 手动添加插件

1. 点“添加插件” → 输入 spec → Run。
2. 创建 `taskId`；检查/安装期间弹窗保持打开，用户可看日志。
3. 点 X / Esc / 遮罩 → 弹窗隐藏，任务继续；底部“取消安装”和顶部返回箭头仍可显式取消。
4. checking 阶段也按同样规则：隐藏后显示“检查中”任务条；只有显式取消才中止检查。
5. 任务完成/失败 → 全局通知；失败时任务条常驻“安装失败 · 查看”。
6. 需要构建脚本审批 → 任务条常驻“等待确认”；点开后才显示审批按钮，绝不自动批准。

#### Discover 安装

1. 在条目/版本行点 Install → 现有确认弹窗（版本、作者、来源、信任提示）。
2. 点确认 → 创建 `taskId`、`open:false`，直接开始检查/安装。
3. 检查失败（输入非法、已安装、registry 非法、inspect RPC 失败、inspect refused、网络失败）→ 自动 `open:true` 显示原因；用 `abandonTask()` 清 `taskId`/记录但**保留 spec 与 inputError**，不能调用会重置表单的 `dismissInstallTask()`。这是唯一自动抢焦点的路径。
4. 检查通过 → Host 安装，保持隐藏；全局任务条显示进度。
5. 安装执行失败/待审批/完成 → 任务条或通知，点“查看”打开详情；不自动抢焦点。
6. 不再出现第二个安装弹窗（除非上述检查失败或用户主动查看）。
7. 已有任务时再点 Install → 聚焦现有任务并切到 Plugins 面板，不启动第二个。

#### 全局任务条

- 位置：`shell.overlay`，**顶部居中、Toast 下方**（约 `top:88px; left:50%; transform:translateX(-50%)`）的窄条卡片；不与聊天右下角“回到底部”按钮、输入区或侧边栏底部控制冲突。
- 内容：插件名/版本（来自 `confirmed.title` 或 `subject.name`，缺失时用“安装任务”）、阶段文案、进度点、“查看”、“取消”。
- 阶段文案复用 `installStarting` / `installingTitle` / `installCancelling` / `installApplying` / `installChecking`；审批态新增“等待确认”；失败态“安装失败”。
- 可见性：仅 `showsTaskPill`；弹窗打开时隐藏。
- 失败态常驻，直到用户打开详情后关闭、或开始新任务；完成态不常驻（由通知呈现）。
- 生命周期：跨侧边栏面板切换保留；刷新恢复见 5.3。

#### 完成/失败通知

- 新增 `InstallNoticeToast`，注册到 `shell.overlay`：
  - 读 `installNotice`；文案复用 `noticeText()` 的 `installBackground*` 键，并补 `cancelled`。
  - `actions` 提供“查看” → `viewInstallTask()`；`onDone` → `dismissInstallNotice()`。
  - 保留现有 `PluginRefreshToast` 处理 `refresh-failed`。
- 页面内 `noticeLine` 不再显示 `kind === 'install'`，避免与全局通知重复。

### 5.3 刷新恢复

#### Host 侧

- 扩展 `InstallControl`：记录 `spec`、`startedAt`（epoch ms），`phase` 增加 `cancelling`。
- `cancelInstall` 在 abort 前把 `control.phase = 'cancelling'`（`applying` 仍返回 too-late）。
- 新增 `@Remote getInstall(requestId): PluginInstallTask | null`，判别联合：
  - 活动：`{ active:true, requestId, spec, startedAt, phase:'installing' | 'cancelling' | 'applying' }`
  - 最近结算：`{ active:false, requestId, spec, startedAt, result }`
  - 未知：`null`
- 短期保留结算结果：`recentInstalls` Map，最多 8 条、TTL 10 分钟；`installBundle` 结算时写入；`getInstall` 读取时清理过期项；`change()` reject（锁超时/销毁）不入表。
- `waitForInstall` 语义不变（只活动任务）。
- 不新增 `listActiveInstalls`：刷新恢复只依赖客户端保存的 requestId。
- `updateBundle` 复用 `installBundle` 的 requestId 也可被查询；客户端不持久化更新任务，因此不会恢复更新。

#### 客户端侧

- 安全读写 `sessionStorage`（隐私模式/禁用时返回 null）。
- 记录只存**不含凭据**的字段：
  ```json
  {
    "taskId": "uuid",
    "requestId": "uuid|null",
    "startedAt": 1790000000000,
    "displayName": "插件标题或包名",
    "confirmed": { "title": "…", "version": "…" },
    "selection": true,
    "saveExact": true
  }
  ```
  **不存原始 spec**（可能含 `https://user:token@…`）；恢复时从 Host 的 `getInstall` 取 spec，展示前用 `sanitizeInstallInput` 脱敏。
- 创建任务（checking 开始）时写入；`startInstall` 拿到 requestId 后补 `requestId`。
- 清除时机：
  - `done` / `cancelled` / 显式取消 / `getInstall` 返回 null / `dismissInstallTask()`：清除。
  - `failed`（含待审批） / `unknown`：**保留**记录，等待用户 dismiss；与 Host `recentInstalls` 的 10 分钟 TTL 对齐，超过 TTL 后 `getInstall` 返回 null 时再清。
- 在 `apply()` 的 `whenAccess('pluginManager/listBundles')` 回调里，`refresh()` 之后调用 `restoreInstallTask()`。
- 恢复流程：
  1. 读取记录；没有则返回。
  2. 若当前 `busy` 或 `taskId` 已存在且 `startedAt` 不早于记录，说明本页已有更新的任务，丢弃旧记录。
  3. 记录没有 `requestId`（刷新发生在 checking）：不重放检查；写 `installNotice`（检查已中断，请重新发起）并清除记录。
  4. 有 `requestId`：调用 `getInstall(requestId)`。
  5. **活动任务**：
     - 先把 Host phase 映射到客户端 phase：`installing → running`、`cancelling → cancelling`、`applying → applying`；不得把 `installing` 直接写进 `InstallState.phase`。
     - 设置 `this.request = { requestId, acknowledged:true, replyLost:false, recovering:false }`
     - `patchInstall({ open:false, restored:true, taskId, requestId, spec, phase: mappedPhase, subject: restoredSubject(info) })`
     - 调用 `waitForInstall(requestId)`：
       - 返回 `null`：立即再 `getInstall` 一次，有 `result` → `settleInstall(result, { restored:true })`；仍 null → `dismissInstallTask()` + `installNotice`（unknown）。
       - 返回 `!ok`（RPC 失败）：同样立刻再 `getInstall` 一次；仍失败 → `patch phase: 'unconfirmed'`（保持 busy）+ `installNotice`（unconfirmed），保留记录；用户可取消或再次核对，**不得停在 running，也不得转成可启动第二个任务的 unknown**。
     - 期间 `plugin-manager/install-state` 与 `install-log` 事件继续更新。
  6. **最近结算**：
     - `settleInstall(info.result, { restored:true })`：只 patch 状态 + `notifyHiddenInstall`，**跳过** `finishAnalytics` / `applied` / `highlight` / `trackToggle`，避免重复埋点和高亮。
     - `failed` / `unknown` 保留记录（供待审批/失败任务条与再次刷新）；`done` 清除记录。全局通知显示结果。
  7. **`null`**：清除记录并 `load()` 刷新插件列表；Host 已重启或任务过期时不弹错误。
- 恢复出来的 `subject` 由 `confirmed` / `selection` / `saveExact` / spec 组合；名称/版本在结算后由 `ChangeResult` 补齐。
- 刷新前终端输出不回放；`restored && runs.length === 0 && isInstallPending(phase)` 时弹窗显示“刷新前的输出未保留”，首个新 chunk 或结算后隐藏。
- `getInstall` RPC 网络失败：保留记录，不展示假任务；在 `connection/reset` 或下一次打开 Plugins 时重试一次。`gateway/unknown-method`/`bad-request`（旧 Host）：视为不支持恢复，清记录并记日志。
- `dismissInstallTask()` 同时清 `taskId`、`installNotice`、sessionStorage，避免旧结果挡住下一次安装。

### 5.4 数据流/时序

```text
用户确认安装
  → 创建 taskId（写 sessionStorage）
  → phase=checking（inspect）
      ├─ 失败 → open:true 显示原因；dismissInstallTask（清 taskId/记录）
      └─ 成功 → startInstall(requestId)（补写 requestId）
                  → phase=starting/running
                  → Host installBundle
                      ├─ install-state/install-log → 客户端更新
                      └─ 结算 → settleInstall
                                  ├─ 隐藏时写 installNotice
                                  ├─ done/cancelled → 清 sessionStorage/taskId
                                  └─ failed/unknown → 保留记录/任务条，待 dismiss

关闭弹窗（busy）
  → open:false（不取消）
  → showsTaskPill → 任务条（checking/running/…/failed）

刷新
  → 读 sessionStorage
      ├─ 无 requestId → installNotice（检查已中断）+ 清记录
      └─ 有 requestId → getInstall
                          ├─ active → 恢复状态 + waitForInstall
                          │            ├─ null → 再 getInstall → result 或 unknown
                          │            └─ !ok → 再 getInstall → unknown（不停在 running）
                          └─ settled → settleInstall(restored) + 通知
                                       failed/unknown 保留记录，done 清除
```

### 5.5 组件与接口

新增 Remote 的配套改动（必须一起做）：

- `packages/boot/plugin-manager/src/types.ts`：`PluginInstallTask`。
- `packages/boot/plugin-manager/src/index.ts`：`getInstall`、`recentInstalls`、`InstallControl` 扩展、`cancelling`。
- `packages/boot/plugin-manager/tests/manager.spec.ts`：活动/结算/过期/未知 id/锁超时。
- `packages/experimental/liveshare/src/endpoint-classification.ts`：`pluginManager/getInstall: 'owner-only'`。
- `packages/experimental/liveshare/src/client/endpoint-classification.ts`（如存在对应客户端表）：同步添加。
- `packages/api/remotes/src/client/index.ts`：导出 `PluginInstallTask`。
- `docs/subsystems/boot.md`、`docs/subsystems/boot.zh.md`、`tool-cordis/src/api-catalog.ts`：由 `pnpm gen-cordis-catalog` 重新生成。

客户端文件：

- `packages/client/ui-plugin-manager/src/client/manager-store.ts`
  - `taskId` / `restored` / `installNotice`
  - `closeInstall` 新语义；`focusInstall`；`installFrom` 后台启动；`runInstall` 护栏；`settleInstall(result, {restored})`
  - sessionStorage 持久化与 `restoreInstallTask()`
  - 派生 helper（`showsTaskPill`、`needsBuildApproval`、`restoredSubject`）
- `packages/client/ui-plugin-manager/src/client/PluginManagerPage.tsx`
  - X / Esc / 遮罩走隐藏语义
  - pending 时 aria-label 改为“隐藏安装任务”
  - 页面 toast 排除 install notice
  - 工具栏“查看安装任务”改用 `viewInstallTask`，条件从 `requestId !== undefined` 改为 `taskId !== null`；不得再调用会 dismiss 旧结果的 `openInstall`
- 新增 `packages/client/ui-plugin-manager/src/client/InstallTaskPill.tsx`（+ CSS module）
- 新增 `packages/client/ui-plugin-manager/src/client/InstallNoticeToast.tsx`
- `packages/client/ui-plugin-manager/src/client/index.ts`
  - 注册两个 `shell.overlay` 组件（注意既有 `browser-plugin.client.spec.tsx` 对 overlay 顺序/注入面的断言）
  - 提供 `viewInstallTask`：`ctx.layout.selectPanel(PANEL_ID); face.viewInstallTask()`（无 analytics，不 dismiss 旧结果）
  - `face.viewInstallTask()` 只 `focusInstall()`；没有任务时才走 `openDialog()` 新建
- `packages/client/ui-plugin-manager/src/client/presentation.ts`
  - `SCREEN_TITLE_KEYS` / `toastHoldMs` / `showsTaskPill` 等可复用 helper
- `packages/client/ui-plugin-manager/src/client/locales.ts`
  - `installHide`、`installTaskChecking`、`installTaskFailed`、`installTaskNeedsApproval`、`installTaskUnknown`、`installTaskDismiss`、`installRestoredNoLogs`、`installBackgroundCancelled`、`installCheckInterrupted`（8 语言）
  - 复用 `installViewTask` / `installCancel`；`installCloseCancels` 不再使用则删除或改写

契约与文档：

- `packages/client/ui-plugin-manager/src/client/slot-contract.ts`
  - `install` 契约改为“启动后台安装任务；弹窗在检查失败或用户查看时打开”，签名不变。
- `packages/client/ui-plugin-manager/README.md` + `README.zh.md`
  - 后台安装、刷新恢复、任务条/通知。
- `packages/boot/plugin-manager/README.md` + `README.zh.md`
  - `getInstall`、最近结果保留；修正“Completed results are not retained”。
- `packages/client/ui-plugin-market/README.md` + `README.zh.md`
  - Discover 确认后后台安装；不再打开第二个弹窗。
- `packages/client/ui-plugin-market/src/client/DiscoverSection.tsx`
  - 代码不改（`installFrom` 内部处理）；只确认文案/文档与行为一致。

### 5.6 错误与边界

| 场景 | 行为 |
|---|---|
| 安装中（含 checking）关闭弹窗 | 隐藏，任务继续；任务条可见 |
| 显式取消 | 走现有 `cancelInstall` 状态机；`too-late`/`applying` 时提示无法取消 |
| 检查失败（输入非法/已安装/registry 非法/inspect 失败/refused/网络） | 自动 `open:true` 显示原因；`abandonTask()` 清任务但保留 spec/inputError |
| 安装执行失败 | 保持隐藏；任务条常驻“安装失败 · 查看” + 全局通知；会话记录保留到 dismiss 或 Host TTL |
| 构建脚本待审批 | 任务条常驻“等待确认”，点击打开弹窗；不自动批准；任务条只提供“查看 / 关闭”，不提供会重置状态的“取消” |
| 后台检查中点“查看” | 只 `focusInstall()`，不重建状态、不 abort inspect |
| 已有 busy 任务时再点安装/添加 | 聚焦现有任务并切面板，不启动第二个 |
| 已有已结算任务（failed/unknown）时再点安装/添加 | 先 `dismissInstallTask()` 清旧结果，再启动新任务 |
| 失败屏“重试”/GitHub 镜像恢复 | `runInstall` 只按 `busy` 拦；failed 允许重跑，复用 taskId |
| 刷新（Host 任务进行中） | 恢复任务条与阶段；`waitForInstall` 等待；null/!ok 时回退 `getInstall` |
| 刷新（Host 任务已完成） | `getInstall` 返回最近结果；恢复完成/失败/待审批并通知；不重复 analytics/highlight；failed/unknown 保留记录 |
| 刷新（checking 阶段） | 不重放检查；通知“检查已中断，请重新发起”；清记录 |
| 刷新（Host 已重启/任务过期） | 清记录并刷新列表，不弹错误 |
| `sessionStorage` 不可用 | 不持久化；当前页面内跨面板仍可用 |
| `getInstall` 网络失败 | 保留记录，不展示假任务；连接重置或下次打开 Plugins 重试一次 |
| `waitForInstall` RPC 失败 | 立即回退 `getInstall`；仍失败则 `unconfirmed`（保持 busy，可取消/核对），不转 unknown |
| 旧 Host 无 `getInstall` | 清记录并记日志，不反复重试 |
| 已结算 `unknown` | 任务条 + 通知；点“查看”打开核对/重试界面；关闭即 dismiss |
| 多标签页 | 各自 UI 独立；Host 文件锁串行化安装；v1 不合并任务条 |
| 应用退出 | sidecar 终止，安装中断；现有行为 |

### 5.7 测试策略

Host：

- `getInstall` 活动任务返回 `active:true` 与 `installing/cancelling/applying`。
- 结算后短期可查；超过 TTL 或超过 8 条按写入顺序淘汰；未知 id 返回 null。
- 锁超时/销毁的 reject 不写入 recent。
- `cancelInstall` 后 `phase` 为 `cancelling`，结算后结果可查。
- `updateBundle` 的 requestId 可查询但不影响安装恢复。

客户端 store：

- `closeInstall` 在 busy（checking/starting/running/cancelling/applying/unconfirmed）下只隐藏，不调用 `cancelInstall`，`taskId` 保留；在 failed/unknown/done 下调用 `dismissInstallTask()`。
- 显式取消在 checking 时 abort inspect 并 dismiss；在 pending 时走现有取消分支。
- `busy` 与 `taskId` 分离：busy 只由 checking/isInstallPending 推导；已结算 failed 的 taskId 不阻止新安装，不阻止失败屏“重试”。
- 已有 busy 任务时 `installFrom`/`openInstall`/工具栏“+”只聚焦，不启动第二个；已有已结算任务时先 dismiss 再启动；checking 中点“查看”不丢检查。
- `showsTaskPill` 覆盖 checking/running/failed/unknown；完成态不常驻；动作矩阵：active=查看+取消，failed/unknown=查看+关闭，done 无。
- `installNotice` 独立于 `notice`；`refresh-failed` 不覆盖安装通知。
- 刷新恢复：
  - 记录写入/更新/清除；无 requestId 时提示检查中断。
  - 活动任务恢复：requestId/spec/phase/任务条/`waitForInstall`。
  - `waitForInstall` 返回 null 或 !ok 时回退 `getInstall`；仍失败则 unknown，不停在 running。
  - 最近结果恢复：状态与通知，跳过 analytics/highlight；failed/unknown 保留记录，done 清除。
  - `sessionStorage` 抛错不影响安装。
  - 已有新任务时不覆盖。
- `restored` 只在恢复时为 true；“刷新前输出未保留”不会在正常新安装首帧误报。
- 8 语言 locale 键完整性。

组件/集成：

- 任务条在 pending + 隐藏时出现，打开弹窗时消失；失败/审批态常驻。
- 全局通知“查看”会切到 Plugins 面板并打开弹窗；不发 `plugin_add_button_click`。
- 页面 toast 不再显示 install notice。
- `browser-plugin.client.spec.tsx` 的 overlay 断言更新。
- `slot-contract.ts` 契约文字更新，Discover 行为一致。
- `pnpm gen-cordis-catalog` 后 `pnpm verify-cordis-catalog` 通过。

手动验证：

- `pnpm dev:web` 加载本地索引，走 Discover 安装/关闭/取消/重开/完成/失败/审批。
- 安装中刷新、刚完成刷新、checking 中刷新三种场景。
- 从聊天面板点任务条“查看”，确认切到 Plugins 并打开详情。
- 市场索引：8 分类 + 全部、标签筛选、搜索/排序。

### 5.8 发布

- `pnpm build:artifacts` 重建 Web 客户端与运行时。
- 更新 `dist-exe` 运行时，重建桌面安装包；或用户通过 Import Runtime 导入新运行时。
- 版本号：DSH 运行时按仓库发布流程；桌面安装包版本递增。
- 分类不需要这一步。

## 6. 兼容性与迁移

- 索引格式向后兼容：旧客户端忽略 `metadata.categories`/`category`/`labels`，仍能显示列表。
- 新索引在 `0.2.3-alpha.2` 即可显示分类，无需客户端升级。
- 新增 `pluginManager/getInstall` 是增量 Remote，旧客户端不会调用；新客户端配旧 Host 时按“旧 Host 无 getInstall”清理记录。
- `getInstall` 需要同步 liveshare 端点分类与 cordis catalog 生成物。
- 后台安装的 `taskId`/`restored` 只是 UI 字段，不影响会话或 profile 持久化。
- sessionStorage 记录不含 spec/凭据；升级后旧记录若无 `taskId` 字段则直接丢弃。

## 7. 风险与缓解

| 风险 | 影响 | 缓解 |
|---|---|---|
| 新插件漏映射 | 分类不完整 | 缺失即 CI 失败，列出名单；这是有意质量门 |
| 插件改名/下架导致陈旧映射 | 每日 CI 失败 | 陈旧映射只告警并忽略 |
| 分类单选无法表达多属性 | 归类争议 | 用 labels 表达跨切面 |
| 中文分类排序不符合预期 | pills 顺序奇怪 | 标题尽量短；格式不支持 order |
| 关闭=隐藏让用户以为已取消 | 后台仍在跑 | 任务条常驻、文案明确、完成通知 |
| 检查阶段关闭/查看逻辑复杂 | 丢失检查 | `taskId` 统一任务身份；检查中点查看只聚焦；测试覆盖 |
| 构建脚本审批被隐藏 | 安全风险 | 审批态任务条常驻“等待确认”，不自动批准 |
| 安装结果通知被覆盖 | 用户看不到结果 | 独立 `installNotice` 槽，不复用 `notice` |
| 跨面板“查看”打不开 | 功能失效 | `viewInstallTask` 先 `selectPanel(PANEL_ID)` 再 open |
| 刷新后重复 analytics/highlight | 数据/UI 污染 | `settleInstall(..., {restored:true})` 跳过副作用 |
| `getInstall` 竞态 | 永久 running | wait null 时回退 `getInstall` |
| 新增 Remote 漏生成物 | 编译/CI 失败 | 端点分类、api/remotes 导出、cordis catalog 一起改并验证 |
| 任务条与聊天/侧边栏悬浮控件冲突 | 遮挡 | 顶部居中、Toast 下方；避开右下角按钮与输入区 |
| 市场仓库无测试载体 | 脚本回归 | 增加最小 `package.json` + `node --test` |
| 测试覆盖不足 | 状态机回归 | 复用现有 store 测试，新增任务/恢复/通知分支 |
| 运行时重建成本 | 发布慢 | 分类先独立发布；后台安装随后发版 |
| 当前源码有未提交改动 | 冲突 | 只改目标文件；不改 `parse-thread.ts`；提交前审查 diff |

## 8. 验收标准

- 索引：
  - 索引中 100% 的插件有显式映射的 `metadata.category`，且都是已声明 id；总数由脚本动态断言（当前 56，不写死）。
  - 使用 `other` 的插件全部在 `otherAllow` 中（预期 0–1 个）；每个非 other 分类至少有一个插件；验收输出分类计数。
  - Discover 出现 8 个分类 pill + “全部”；点击过滤正确；“全部”恢复。
  - 标签筛选与搜索/排序不回归。
  - `node --test scripts/*.test.mjs` 通过。
- 客户端：
  - 安装中（含 checking）关闭弹窗不取消；显式取消才取消。
  - 任务条跨面板可见，显示名称/阶段/取消/查看；失败/审批态常驻。
  - 完成后全局通知，可打开结果；通知不被 `refresh-failed` 覆盖。
  - 从聊天面板点“查看”会切到 Plugins 并打开安装弹窗。
  - 已有任务时再点安装不会启动第二个。
  - 安装进行中刷新：任务条、阶段、取消恢复；`waitForInstall` 竞态有回退。
  - 安装刚完成时刷新：最近结果通知恢复；不重复 analytics/highlight。
  - checking 中刷新：提示“检查已中断”，不静默丢失。
  - 审批态不会自动执行脚本。
  - Discover 确认后不再出现第二个安装弹窗（正常路径）；检查失败会自动打开。
  - 现有测试、类型检查、lint 通过；`pnpm verify-cordis-catalog` 通过。
- Rust：
  - 维持现状，无代码变更；依据见 §1/§3。
- 发布：
  - 分类只发布索引即可生效。
  - 后台安装产出新运行时（若本轮含 T7），桌面安装包或 Import Runtime 可验证。

## 9. 工作分解（初步）

- T1：`dsh-plugin-market` 分类体系 + `categories.json`（56 条映射）+ 后处理脚本 + 测试 + CI + README。
- T2：Host `getInstall` + 最近结果保留 + `cancelling` + 类型 + Host 测试。
- T3：`ui-plugin-manager` 任务模型（`taskId`、`closeInstall`、`focusInstall`、`installFrom`、`installNotice`）+ store 测试。
- T4：全局任务条 + 全局通知 + `shell.overlay` 注册 + `viewInstallTask` 切面板 + 组件测试 + 8 语言文案。
- T5：sessionStorage 持久化 + `restoreInstallTask()` + 竞态回退 + 恢复测试。
- T6：契约/README/端点分类/api 导出/cordis catalog 生成 + `browser-plugin` 测试更新。
- T7：`pnpm build:artifacts`、运行时与桌面安装包验证（本轮可选，视发布需求）。

总预估：7–9 工程日（分类 1–1.5；Host 1–1.5；客户端任务 2–3；恢复 1–1.5；契约/测试/联调 1–1.5）。

## 11. 追加需求：不兼容插件过滤

### 11.1 目标

- G8：Discover 的列表、搜索结果、`total` 与分页默认只包含“至少有一个可安装版本”的插件。
- G9：被隐藏的数量可见，提供显式开关临时显示；显示时保留“不兼容”标记与禁用安装。
- G10：过滤不改变任何现有安全语义（固定版本、来源展示、信任确认、豁免）。

### 11.2 判定规则

一个条目“可安装”当且仅当存在某个版本满足：

```
compatibility.compatible === true || compatibility.exempted === true
```

即 `engines.dsh` 满足当前运行时，或 profile 已为该 `包@版本 + 当前运行时` 授予豁免。未声明 `engines.dsh` 的版本按“任意版本可用”处理（与 `evaluatePluginCompatibility` 一致），因此 22 个未声明范围的插件不被隐藏。平台字段不在索引中，本轮不参与判定。

### 11.3 Host 设计

`packages/boot/plugin-market`：

- `MarketQuery` 增加可选 `includeIncompatible?: boolean`（默认 false）。
- `MarketSearchResult` 增加 `incompatible: number`：查询命中但没有任何可安装版本的条目数，无论是否请求显示都返回。
- `search()` 在分页前用 `evaluatePluginCompatibility` 过滤命中项，因此 `total`、`nextOffset`、分页边界都只基于可见条目；随后仍只对返回页做完整注解（安装状态、默认版本等），保持现有“只注解当前页”的行为。
- `plugin_market` 工具：`list_plugins` 输出增加 `incompatible` 计数；`list_versions` 不变。
- 兼容性判定复用同一 `compatibilityOf()`，避免与安装校验产生第二套语义。

### 11.4 客户端设计

`packages/client/ui-plugin-market`：

- `DiscoverQuery` 增加可选 `includeIncompatible?: boolean`；切换来源、搜索、排序、刷新都保留该字段。
- `market-store.page()` 只在为 true 时发送 `includeIncompatible`，旧 Host 忽略未知字段/保持默认过滤，新客户端不因旧 Host 报错。
- Discover 结果区底部（`Show more` 之后）渲染提示条：默认“已隐藏 {count} 个与当前 DSH 不兼容的插件” + “显示”；打开后“正在显示与当前 DSH 不兼容的插件” + “隐藏”。
- 当 `total === 0` 但 `incompatible > 0` 时，空态文案后仍渲染提示条，说明“不是索引缺失，而是都被过滤”。
- 8 种语言补 4 个键：`incompatibleHidden`、`incompatibleShown`、`showIncompatible`、`hideIncompatible`。
- 条目仍由现有 `EntryAction` 渲染：无兼容版本时按钮 `aria-disabled` 并带 `noCompatibleVersion` tooltip。

### 11.5 风险

| 风险 | 影响 | 缓解 |
|---|---|---|
| 过滤口径过窄（只看版本） | 平台不支持的插件仍出现 | D15 记录；索引格式扩展后再加平台判定 |
| 用户以为插件下架 | 搜索不到 | 显示隐藏数量与开关 |
| Host 旧客户端行为变化 | 旧客户端列表变少 | 这些条目本就不可安装；属期望行为 |
| 过滤掉已安装插件 | 无法在 Discover 更新/卸载 | Installed 标签仍完整保留 |
| 全量注解导致性能回退 | 大索引变慢 | 只在分页前做轻量兼容性判定，仍只注解当前页 |

### 11.6 验收

- 7 个全不兼容插件默认不在列表/搜索/`total` 中；`incompatible` 计数正确。
- 打开开关后显示，安装按钮禁用；关闭后恢复隐藏。
- 部分版本兼容、以及有豁免的插件不被隐藏。
- 分页在过滤后仍然连续（第 2 页不重复、不跳过）。
- 新 Host + 旧客户端：旧客户端不传 `includeIncompatible`，得到过滤后的列表（可接受）。
- 新客户端 + 旧 Host：不传未知字段之外无其他要求；旧 Host 忽略字段时仍按旧行为显示，客户端提示条只在 `incompatible > 0` 时出现。
- Host 单测、客户端单测、`pnpm typecheck`、`pnpm verify-cordis-catalog` 通过。

## 12. 追加工作分解

- T8：Host 查询字段 + 结果计数 + 分页过滤 + 工具计数 + Host 测试。
- T9：客户端查询字段 + 提示条/开关 + 8 语言 + store/组件测试。
- T10：README（EN/ZH）、`api/remotes` 导出核对、`gen-cordis-catalog` 生成物、`verify-cordis-catalog`、类型检查与聚焦 vitest。
- T11（可选，与 T7 合并）：重建客户端 bundle 与运行时，在桌面安装版验证提示条与开关。

## 10. 待 Gate 复审确认

1. 刷新恢复默认**不回放**刷新前的终端输出，只恢复任务、阶段和结果；是否需要完整回放（成本更高）。
2. 本轮是否包含运行时/桌面安装包产出（T7），还是只交付源码、测试和分类索引。
3. 分类标签采用 `vision` / `search` / `memory` / `local`（替换原 `china`），`local` 表示需要本机应用或本地服务；是否接受。
