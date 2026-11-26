# Windows 接手与实机验收

本轮准备分支：`codex/cross-platform-foundation`。

三平台已通过构建的源码提交：`fed9e5242fcaf325cc8dc460aaebc5342e68922f`。后续仅更新交接文档的提交不改变此构建证据。验收某个新功能时，改用该功能的最终 SHA，并同时记录两端结果。

## 1. 获取与检查

先安装 Node.js 24.18.0、Rust/rustup、Microsoft C++ Build Tools 的“使用 C++ 的桌面开发”和 Windows SDK、WebView2。详细依赖见 [Tauri 官方说明](https://v2.tauri.app/start/prerequisites/)。

在 PowerShell 中执行：

```powershell
git clone --branch codex/cross-platform-foundation https://github.com/Tinkerzyx/Codex-Switch.git
cd Codex-Switch
git rev-parse HEAD
cd codex-switch-app
npm ci
npm run check
npm test
npm run build:debug
```

已有克隆时，先确认工作区干净，再执行：

```powershell
git fetch origin
git switch codex/cross-platform-foundation
git pull --ff-only
git rev-parse HEAD
```

如要严格复现本次已验证的二进制，用独立本地分支定位该提交，再运行上述 npm 命令：

```powershell
git switch -c verify/foundation-fed9e52 fed9e5242fcaf325cc8dc460aaebc5342e68922f
```

当前上游无自动化用例，`npm test` 输出 0 tests 是已知状态。日后每个功能需要补充对应测试和原生验收，不能只看命令退出码。

## 2. 先运行空白数据外壳

从 `codex-switch-app` 执行。该启动方式隔离 profile 与 Codex 配置，只检查界面、拖动、最小化、托盘隐藏和恢复；此阶段不点击新登录、激活账号或终止进程。

```powershell
$binary = (Resolve-Path '.\src-tauri\target\debug\codex-switch-app.exe').Path
$smokeRoot = Join-Path ([System.IO.Path]::GetTempPath()) ('codex-switch-smoke-' + [guid]::NewGuid())
New-Item -ItemType Directory -Force $smokeRoot | Out-Null
New-Item -ItemType Directory -Force (Join-Path $smokeRoot 'codex-home') | Out-Null
New-Item -ItemType Directory -Force (Join-Path $smokeRoot 'codex_switch_data') | Out-Null
$previousCodexHome = $env:CODEX_HOME
Push-Location $smokeRoot
try {
    $env:CODEX_HOME = Join-Path $smokeRoot 'codex-home'
    Start-Process -FilePath $binary -WorkingDirectory $smokeRoot -Wait
} finally {
    Pop-Location
    $env:CODEX_HOME = $previousCodexHome
}
```

退出时使用托盘 Exit。程序是本次源码生成的调试版本，不需要覆盖已有安装或清空已有 profile。`CODEX_HOME` 只隔离本应用文件路径，并不保证外部 Codex/ChatGPT 应用也使用该路径；账号操作须在后续专用测试会话中执行。

## 3. 现成安装包

[Windows x64 调试产物](https://github.com/Tinkerzyx/Codex-Switch/actions/runs/45862900310/artifacts/15139456405) 内包含 `Codex Switch_0.1.0_x64-setup.exe` 及源码/工具链信息。CI 产物保留 14 天；入口失效时可从上述提交本地构建。

安装包用于后续安装与升级验收。本轮保留原来的 0.1.0 版本和应用标识，可能与已有安装重叠；首次空白外壳验证使用上一节本地生成的可执行文件。正式发布、签名和升级策略另行验收。

## 4. 回传记录

在当前聊天中回传以下信息即可，截图请隐藏个人信息：

```text
Windows 版本 / CPU：
git rev-parse HEAD：
npm run check：pass / fail
npm test：命令结果 + 实际测试数量
npm run build:debug：pass / fail
空白窗口 / 拖动 / 最小化：pass / fail / pending
托盘隐藏 / 恢复 / Exit：pass / fail / pending
发现的问题：操作步骤 + 预期 + 实际 + 脱敏截图/错误
```

Windows 实机结果目前为 pending。下一项功能使用 [FEATURE 模板](templates/FEATURE.md) 记录正常流程、失败流程和两端相同 SHA 的验收证据。
