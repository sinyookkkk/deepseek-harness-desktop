# 2026-11-25 插件市场分类与后台安装 — 需求澄清

## 1. 原始诉求

用户提出三个优化点：

1. 插件市场能否改成 Rust 实现？
2. 插件市场上能否分类？
3. 插件能否支持后台安装？现在安装插件时会弹出弹窗，关闭弹窗又变成取消了。

## 2. 可行性分析结论

| 诉求 | 结论 | 依据 |
|---|---|---|
| Rust 重写市场 | 不建议 | 市场 UI 是 WebView 中的 React 客户端插件，Host 是 Node 服务；Tauri 壳没有 IPC，只负责启动 sidecar。Rust 无法替换市场本体，且会破坏浏览器/桌面共用与 plugin-manager 的安装语义。 |
| 市场分类 | 可做，不改 DSH 运行时 | `@deepseek-ai/dsh-plugin-market-index` 已定义 `metadata.categories` / `metadata.category` / `metadata.labels`；`ui-plugin-market` 已有分类 pills、标签筛选和排序。当前自定义索引 `dsh-plugin-market/public/index.json` 没有声明分类。 |
| 后台安装 | 可做，客户端为主 | Host `plugin-manager` 已支持 `installBundle({requestId})`、`waitForInstall(requestId)`、`cancelInstall(requestId)` 的分离任务。当前 `manager-store.closeInstall` 在安装进行中会显式调用 `cancelInstall`，这是客户端产品选择。刷新恢复需要 Host 增加 `getInstall` 查询与短期结果保留。 |

## 3. 用户选择

用户选择 **A：分类 + 后台安装（Rust 维持现状）**。

## 4. 范围

### In scope

- 在 `E:\workspace\dsh-plugin-market` 为索引增加分类声明和每个插件的分类，使“发现”页显示分类 pills 并可按分类筛选。
- 在 `E:\workspace\deepseek-harness` 的 `ui-plugin-manager` 中把“关闭安装弹窗”从“取消安装”改为“隐藏任务”。
- 增加跨面板可见的安装任务指示和完成/失败通知。
- 支持页面刷新后恢复进行中的安装任务：Host 增加任务查询接口，客户端用 sessionStorage 记住 taskId/requestId。
- Discover 确认后直接在后台安装，不再打开第二个安装弹窗；若仍在检查阶段，则显示任务条；检查失败、需要构建脚本审批时再打开详情弹窗。
- 安装任务进行中关闭弹窗只隐藏；只有显式取消才中止。
- 保持信任确认、固定版本、构建脚本审批、取消状态机等现有安全语义。

### Out of scope

- 插件市场本体或 Host 服务的 Rust 重写。
- 应用退出后的安装恢复（sidecar 生命周期，现有行为）。
- 多浏览器标签页之间的任务同步。
- 刷新后回放刷新前的完整终端日志（只提示“刷新前输出未保留”，新输出继续追加）。
- 上游 `plugin-market-generator` 的原生分类参数（v1 用索引侧后处理）。
- 自动批准安装脚本。
- 自动更新、托盘、账号等桌面壳能力。

## 5. 关键决策与假设

| 编号 | 决策/假设 | 来源 | 状态 |
|---|---|---|---|
| D1 | 分类只做在索引侧：`categories.json` + 后处理脚本注入 `metadata` | 用户 Gate 0 批准 | 已确认 |
| D2 | 单分类 + 多标签：`category` 单选，`labels` 表达跨切面属性 | 格式约束 | 已确认 |
| D3 | 分类使用 8 个：必备基础、能力增强、界面效率、开发工作流、绘图设计、3D 建模、趣味美化、其他 | 用户 Gate 0 批准 | 已确认 |
| D4 | 索引中每个插件必须有显式映射；缺失即 CI 失败并列出名单；`other` 只允许 `otherAllow` 白名单，每个非 other 分类至少一个插件 | 质量门（复审修订） | 已确认 |
| D5 | 后台安装支持页面刷新恢复：sessionStorage 保存 requestId，Host 提供活动/最近完成任务的查询 | 用户选择 | 已确认 |
| D6 | 关闭弹窗 = 隐藏（含 checking 阶段）；只有显式“取消安装”才取消 | 用户诉求 | 已确认（A） |
| D7 | 安装需构建脚本审批时，后台任务停在“等待确认”，点击打开弹窗，不自动批准 | 安全约束 | 必须 |
| D8 | 同时只允许一个安装任务；再次点安装聚焦现有任务。并发判断只用派生 `busy`（checking/pending），`taskId` 只作任务条身份 | 现有 store 行为 + 复审修订 | 已确认 |
| D9 | Discover 确认后直接后台安装，不再出现第二个安装弹窗；检查失败自动打开 | 用户诉求 | 已确认 |
| D10 | 分类改动不需要重建 DSH 运行时；后台安装需要重建运行时并更新桌面安装包或走 Import Runtime | 发布路径 | 必须 |
| D11 | 任务标识 `taskId` 从 checking 开始，独立于 Host requestId；防止 checking 阶段被当成新任务重置 | 复审 P0 | 已确认 |
| D12 | 安装结果通知使用独立 `installNotice` 槽，不复用通用 `notice`，避免被刷新失败覆盖 | 复审 P1 | 已确认 |

## 6. 验收线索

- `public/index.json` 通过格式校验；索引中每个插件都有显式映射的 `category`（不是“全部 other”）；`other` 只在 `otherAllow` 白名单；Discover 出现分类 pills，筛选正确。
- 在 Discover 点安装并确认后，弹窗关闭但安装继续；切换到聊天再回来，任务指示仍在；从聊天点“查看”会切到 Plugins 并打开详情。
- 安装中（含 checking 阶段）刷新页面后任务指示和阶段恢复；checking 阶段刷新提示“检查已中断，请重新发起”；完成后有通知。
- 手动“添加插件”流程中，安装进行时点 X 只隐藏，不取消；点“取消安装”才取消。
- 刷新发生在安装完成之后时，能通过 Host 保留的最近结果看到完成通知；Host 重启后任务不存在则清理本地记录。
- 需要构建脚本审批时，任务显示“等待确认”，不会自动执行脚本；安装失败时任务条常驻“安装失败”，不会被刷新失败通知覆盖。
- 已有安装任务时再点安装不会启动第二个。
- 现有单元测试、类型检查、lint、`verify-cordis-catalog` 通过。

## 7. 未决问题

- 刷新后是否需要完整回放刷新前的终端输出（默认不回放）。
- 本轮是否要产出新的桌面运行时/安装包，还是只交付源码与测试。
- 标签用 `local`（需本机应用/本地服务）替换原 `china` 是否接受。

## 8. 追加需求（2026-11-25 晚）：过滤不支持的插件

### 8.1 原始诉求

用户提出第 4 个需求：**plugin 市场需要过滤不支持的 plugin**。

### 8.2 现状证据

- 发现页对每个版本已计算 `MarketCompatibility`（`engines.dsh` 与当前 DSH 的比较，含 profile 豁免），但对“整个插件没有任何可安装版本”的条目只在卡片上禁用安装按钮，条目本身仍出现在列表、搜索结果和 `total` 中。
- 当前索引 72 个插件，运行包 `0.2.3-alpha.2` 下 **7 个**插件所有版本都不兼容：`dsh-boost`、`dsh-diagram`、`dsh-usage-stats`、`dsh-billing-stats`、`dsh-token-saver`、`billion-context-dsh`、`@ljwei-stak/dsh-model-route-bot`；另有 22 个插件未声明 `engines.dsh`。
- 索引格式 `MarketVersion.engines` 只有 `dsh` 字段，没有平台字段；`dsh.bundle.platforms` 只存在于安装后的包清单，索引无法表达。
- `plugin_market` 工具（agent 侧）复用同一个 `search`，同样会列出不可安装的条目。

### 8.3 口径与决策

| 编号 | 决策/假设 | 来源 | 状态 |
|---|---|---|---|
| D13 | “不支持”= **该插件没有任何可安装版本**：所有版本 `engines.dsh` 都不满足当前 DSH，且没有针对当前 DSH 的版本豁免 | 默认口径（用户未逐条选择，按推荐方案） | 已采用，可扩展 |
| D14 | 未声明 `engines.dsh` 的插件视为“未声明即任意版本可用”，**不**过滤（与 DSH 安装校验一致） | 现有 `evaluatePluginCompatibility` 语义 | 已确认 |
| D15 | 平台不支持（`dsh.bundle.platforms`）本轮不过滤：索引不发布该字段，需要先扩展索引格式与生成器 | 范围控制 | 已确认 |
| D16 | 默认隐藏；结果下方给出“已隐藏 N 个与当前 DSH 不兼容的插件”与显式“显示”开关；打开后条目照常显示但保留“不兼容”标记与禁用安装 | 推荐 UX | 已采用 |
| D17 | 过滤在 Host `pluginMarket.search` 内完成，`total`/`nextOffset` 只统计可见条目；新增可选查询字段 `includeIncompatible` 与结果字段 `incompatible` 计数；旧客户端不传即保持默认过滤 | 分页/总数正确性 | 已确认 |
| D18 | 已安装但全部版本不兼容的插件同样默认隐藏；已安装列表仍可见，可卸载/降级 | 一致性 | 已确认 |
| D19 | `plugin_market` 工具沿用默认过滤，并在结果里给出 `incompatible` 计数，避免模型以为索引缺失 | agent 侧一致性 | 已确认 |

### 8.4 验收线索

- 默认 Discover 列表、搜索结果、`total`、分页都不含 7 个全不兼容插件。
- 结果下方显示隐藏数量；点“显示”后这些条目出现，带“不兼容”标记且安装按钮不可用；再点“隐藏”恢复。
- 有豁免的版本仍算可安装，其插件不会被隐藏。
- 只声明部分版本兼容的插件不被隐藏（默认版本仍取兼容版本）。
- 搜索只剩不兼容插件时，显示“没有符合条件的插件”以及隐藏数量提示，而不是空白的“此索引没有插件”。
- `plugin_market` 的 `list_plugins` 返回 `incompatible` 计数。
- Host 单测、客户端单测、类型检查、`verify-cordis-catalog` 通过。
