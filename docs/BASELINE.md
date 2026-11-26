# 双平台准备基线

日期：2026-11-26。来源：`Tinkerzyx/Codex-Switch`，上游 `main` 提交 `97bbd4b`。准备分支：`codex/cross-platform-foundation`。

## 范围

完成源码获取与结构检查，准备固定工具链、npm 检查命令、平台构建配置、原生 CI、协作与验收模板。本次不实现新的账号功能，也不声称已完成 macOS 功能移植。

本机：macOS 27.0 / Apple Silicon arm64，Node.js 24.18.0，Rust 1.100.0，已安装 Xcode Command Line Tools。完整 Xcode 对当前桌面基线不是必需项。

## 检查结果

| 检查 | 状态 | 说明 |
| --- | --- | --- |
| Git 获取 | pass | 已克隆并建立独立准备分支 |
| `npm ci` | pass | 使用现有 lockfile，CLI 2.11.4 |
| JS / 工具脚本语法 | pass | `npm run check:frontend` |
| Rust 格式 | pass | `npm run fmt:check`；本机 stable 与固定版本均为 1.100.0 |
| JSON / diff 空白检查 | pass | npm、Tauri 配置可解析；`git diff --check` |
| Rust 编译 / Clippy | pass | `cargo check --locked` 与 `npm run check`；Clippy 覆盖 all-targets，禁止警告 |
| Rust 测试命令 | pass | `npm test` 执行成功，实际 0 个测试；不代表功能覆盖 |
| macOS ARM64 调试打包 | pass | `npm run build:debug` 生成 `.app` 和 8.6 MB `.dmg` |
| macOS 原生外壳启动 | pass | 在临时 CODEX_HOME 与空白数据目录运行，原生窗口显示 Profiles loaded，未操作账号 |
| Windows x64 CI 构建 | pass | Clippy、测试命令、NSIS 打包和产物上传均成功 |
| macOS ARM64 CI 构建 | pass | Clippy、测试命令、app/dmg 打包和产物上传均成功 |
| macOS Intel CI 构建 | pass | Clippy、测试命令、app/dmg 打包和产物上传均成功 |
| Windows 实机功能 | pending | 用户已有 Windows 电脑，按同一 SHA 验收 |
| macOS 账号 / 系统功能 | pending | Windows 专用系统调用尚未移植，见 ARCHITECTURE.md |

本机复用已安装的同版本 stable，通过仓库目录的 rustup override 选择；固定工具链文件不变。首次 Cargo 下载超时后，使用 `CARGO_HTTP_MULTIPLEXING=false` 重试成功。Tauri 提示现有 bundle identifier 以 `.app` 结尾，正式确定 macOS 应用标识时需要处理；本次保留现有标识。

## 远端验证与交接

验证提交：`fed9e5242fcaf325cc8dc460aaebc5342e68922f`。三平台构建在 2026-11-26 09:04（America/Los_Angeles）全部完成，[CI 记录](https://github.com/Tinkerzyx/Codex-Switch/actions/runs/45862900310)。三个作业实际均为 0 个测试用例；状态为 pass 的是测试命令及打包流程。

| 平台 | 原生产物 | 下载入口 |
| --- | --- | --- |
| Windows x64 | `Codex Switch_0.1.0_x64-setup.exe` | [Windows 调试产物](https://github.com/Tinkerzyx/Codex-Switch/actions/runs/45862900310/artifacts/15139456405) |
| macOS Apple Silicon | `.app.tar.gz` 与 `Codex Switch_0.1.0_aarch64.dmg` | [ARM64 调试产物](https://github.com/Tinkerzyx/Codex-Switch/actions/runs/45862900310/artifacts/15139487010) |
| macOS Intel | `.app.tar.gz` 与 `Codex Switch_0.1.0_x64.dmg` | [Intel 调试产物](https://github.com/Tinkerzyx/Codex-Switch/actions/runs/45862900310/artifacts/15140855164) |

产物是 GitHub Actions 临时验收包，保留 14 天，下载可能要求 GitHub 登录。各包包含 `COMMIT.txt`、`RUST.txt`、`NODE.txt`，用来核对源码和工具链。过期后按同一提交重新构建即可。

[Windows 接手步骤与实机记录](WINDOWS_HANDOFF.md) 已准备。开发准备工作可交接；Windows 人工 GUI 操作、账号流程以及 macOS 系统功能的验收仍需按记录执行。源码分支保留待验收，不因构建成功自动合并 main。

## 接下来的功能开发

先按 [FEATURE 模板](templates/FEATURE.md) 写出需求与两端验收条件。涉及路径、应用启动、进程管理、额度请求的功能，需同时完成对应 macOS 系统适配；纯 UI 也需分别检查 WebView2 和 WKWebView。

账号/进程操作使用独立测试会话；本轮外壳检查不切换真实账号，不终止开发宿主进程。正式签名、公证、发布和生产可用性不属于本次准备结果。
