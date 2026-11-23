---
description: "面向自用的 Tauri 2 桌面外壳：在 Windows x64 上从内置运行时启动 dsh Web UI，并通过 Import Runtime 更新运行时。"
---

# DeepSeek Harness Tauri 外壳

[English](README.md) | 中文

## 摘要

本应用是面向 Windows 10/11 x64 的 Tauri 2 小型外壳，用于承载 dsh Web UI。它把内置的单文件 dsh 运行时作为受监督的 sidecar 启动，在 WebView2 中加载带令牌的 `dsh web` URL，并在窗口关闭时终止整个 sidecar 进程树。安装包是自包含的：安装后既不需要本仓库，也不需要系统 Node。

## 构建

构建脚本需要 PowerShell 7、Rust 工具链和 Tauri CLI（本应用记录的 `cargo tauri --version` 为 2.12.6）。在应用目录中运行（本仓库中为 `apps/desktop-tauri`；独立检出时为仓库根目录）：

```powershell
pwsh scripts/build-installer.ps1
```

脚本先准备内置载荷，再执行 `cargo tauri build`：

- `prepare-runtime.ps1` 优先使用 `dist-exe/deepseek-harness-sdk-runtime-win-x64.exe`。该本地构建不存在时，它下载公开 PyPI 上最新的 `deepseek-harness-runtime-bin` win_amd64 wheel，校验 PyPI 发布的 SHA-256，并把 `deepseek-harness-sdk-runtime-win-x64.exe` 及其 `-rg.exe` sidecar 解压到 `resources/runtime/`。可用 `DSH_DESKTOP_RUNTIME_VERSION` 固定已发布的版本。
- `prepare-node.ps1` 下载 Node 24.22.3 win-x64，按发布的 `SHASUMS256.txt` 校验；下载 pnpm 11.28.5，校验 npm registry 的 SHA-512 integrity；然后在 `resources/pnpm/` 中创建 `pnpm.cmd` shim。

NSIS 安装包写入应用目录下的 `target/release/bundle/nsis/`。生成的载荷、`target/` 和安装包输出不提交。

## 安装

按用户安装（`currentUser` 模式）不需要提权，并注册卸载项。安装包包含种子运行时以及 Node、npm、npx、pnpm 工具链。WebView2 bootstrapper 会在系统尚无 WebView2 时下载运行时。

卸载会删除已安装的应用及其内置资源。`$DSH_HOME` 不属于安装内容，保持不变。

## 运行时选择与 Import

启动时优先使用 `%LOCALAPPDATA%\ai.deepseek.harness.desktop-tauri\runtimes\current.json` 记录的运行时；记录缺失或无效时回退到内置种子运行时。种子运行时永不修改。

**Import Runtime…** 打开文件对话框，只接受同时具备同名 `-rg.exe` sidecar、且 `--version` 能输出语义化版本的 `deepseek-harness-sdk-runtime-win-x64.exe`。有效导入会把两个文件复制到 `runtimes/<version>/`，把该版本记录为当前版本，并提示重启。上一版本的目录保留。无效导入不改变当前选择。新导入的运行时首次启动失败时，外壳恢复上一版本；没有上一版本时删除记录，改用内置种子运行时。首次启动成功后该选择被标记为稳定，之后的失败只报告错误，不再改变选择。

内置种子通过重新构建来更新：使用更新的 `dist-exe` 或更新的已发布 wheel。对于外壳没有安装过的 dsh 运行时，Import 是更新途径。

## 工具链固定

内置工具链为 Node 24.22.3（含 npm 与 npx）和 pnpm 11.28.5，两个下载都在解压前校验。外壳把内置的 `resources/node` 与 `resources/pnpm` 追加到继承的 `PATH` 之后，因此用户已安装的 Node 保持优先，内置工具链补齐其余条目。`pnpm.cmd` shim 通过内置 Node 运行 pnpm。

代理 shell 通过 `childEnv` 继承 sidecar 环境，且两个 Windows shell 适配器都不是登录 shell（`pwsh -NoProfile`、`bash -c`），因此追加的条目会保留到代理 shell 命令中。

## 配置

外壳读取 `%LOCALAPPDATA%\ai.deepseek.harness.desktop-tauri\config.toml`。缺失字段均为可选；首次运行时创建该文件。

```toml
workspace = 'C:\Users\you'
# dsh_home = 'C:\Users\you\.dsh'
# node_dir = 'C:\path\to\node'
# pnpm_dir = 'C:\path\to\pnpm'
```

`workspace` 是 sidecar 的工作目录。`dsh_home` 作为 `DSH_HOME` 传入；省略时使用 dsh 默认值。`node_dir` 与 `pnpm_dir` 覆盖内置工具链位置。首次启动会从随附的 `web` 模板初始化专用的 `desktop-tauri` profile。sidecar 的 stderr 追加写入 `logs/sidecar.log`，令牌值会被脱敏；stdout 仅用于解析入口 URL，从不落盘。

## 已知限制

- 仅支持 Windows x64；外壳未签名，面向自用。
- 没有自动更新源、托盘图标、账号登录或多实例管理。Import Runtime 是 v1 的运行时更新机制。
- 重启会中断正在执行的回合。会话日志仍然持久，但进行中的回合可能丢失。
- PyPI 种子 wheel 可能落后于当前发布；本地构建的运行时请使用 Import Runtime。
- `dsh web:` stdout 行是展示约定。外壳最多等待 90 秒，只接受带 `token` 参数的 loopback URL；启动失败时报告 stderr 尾部。
- WebView2 是系统组件。系统尚未安装 WebView2 时，bootstrapper 需要网络访问。

## 实测数据

- 安装包：96.0 MB（`DeepSeek Harness_0.2.3-alpha.2_x64-setup.exe`，100,622,691 字节）。
- 安装后占用：246.0 MB（257,930,686 字节）。

- 运行时更新载荷：`dsh.exe` 141.1 MB（147,907,584 字节）加 `dsh-rg.exe` 5.2 MB（5,429,760 字节）；导入会把两者复制到 `runtimes/<version>/`。
- 种子运行时：来自 PyPI 的 0.2.2-rc.2；`--version` 输出 `0.2.2-rc.2`，URL 行为 `dsh web: http://127.0.0.1:<port>/?token=<token>`。
- 代理 shell PATH 检查：在没有系统 Node 的 `PATH` 下，`node --version`、`npm --version`、`npx --version`、`pnpm --version` 都能从内置目录解析；不需要安装器写入用户 PATH。
