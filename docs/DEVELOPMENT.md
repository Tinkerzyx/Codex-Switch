# Windows / macOS 开发与验收工作流

准备日期：2026-11-26。仓库：<https://github.com/Tinkerzyx/Codex-Switch>。

用户已确认有 Windows 电脑可同步代码并实机验收；当前开发机是 Apple Silicon Mac。工作流按同仓库、同功能分支准备。macOS 当前属于构建与外壳准备阶段，功能移植缺口见 [ARCHITECTURE.md](ARCHITECTURE.md)，执行结果见 [BASELINE.md](BASELINE.md)。

## 1. 环境与首次安装

两端统一 Node.js `.node-version`、Rust `rust-toolchain.toml`，以及已提交的 `package-lock.json` / `Cargo.lock`。npm 脚本会自动将标准 rustup 的 bin 目录加入子进程 PATH，也支持 `CARGO_HOME`；不会修改系统 PATH。

macOS 安装 Xcode Command Line Tools；Windows 安装 Microsoft C++ Build Tools 的“使用 C++ 的桌面开发”、Windows SDK、WebView2 Runtime，并使用 MSVC Rust 工具链。官方说明：[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)。

```sh
git clone https://github.com/Tinkerzyx/Codex-Switch.git
cd Codex-Switch/codex-switch-app
npm ci
npm run check
npm test
```

已有本地克隆只需进入对应 `codex-switch-app`。rustup 首次使用会按仓库文件安装固定版本及 rustfmt/clippy，需要网络。

本次 Mac 已安装的 stable 与固定工具链均为 1.100.0，为避免重复下载，在本地仓库目录设置了 `rustup override set stable`（不进入 Git）。升级仓库工具链时须先 `rustup override unset`，再按新的固定版本安装。可用 `rustup show active-toolchain` 和 `rustc --version` 检查；其他机器直接使用仓库固定版本即可。遇到 Cargo 并发下载超时，可临时设置 `CARGO_HTTP_MULTIPLEXING=false` 重试。

## 2. 常用命令

以下 npm 命令适用于 PowerShell 和 macOS 终端，工作目录均为 `codex-switch-app/`：

| 命令 | 用途 |
| --- | --- |
| `npm ci` | 安装锁定的 Tauri CLI |
| `npm run check` | JS/工具脚本语法、Rust 格式和 Clippy |
| `npm test` | Rust 测试；当前上游没有测试用例，0 tests 不代表功能通过 |
| `npm run dev` | 原生 Tauri 开发运行；macOS 现阶段按下节隔离运行 |
| `npm run build:debug` | 原生调试构建与打包，Cargo 使用 `--locked` |
| `npm run build -- -- --locked` | 原生 release 构建与打包 |
| `npm run tauri -- info` | 本机工具链诊断 |

Tauri 自动合并 `tauri.windows.conf.json` / `tauri.macos.conf.json`。Windows 输出 NSIS `.exe`；macOS 输出 `.app` 和 `.dmg`。macOS 目前关闭原生窗口透明度以建立无需 private API 的外壳构建基线，最终外观单独验收。配置规则参考 [Tauri configuration files](https://v2.tauri.app/develop/configuration-files/)。

调试包用于验收；正式签名、公证、自动更新和 Release 发布另行配置。现有版本号保留 0.1.0，本次不表示新正式版本。

### macOS 当前的隔离外壳检查

`locate_data_dir()` 尚依赖工作目录，Finder 启动与账号管理尚未验收。构建后仅在临时目录检查窗口，避免写入真实账号目录；不要点击登录、切换或终止进程：

```sh
# 从 codex-switch-app/ 执行；路径含空格，必须保留引号。
app_binary="$PWD/src-tauri/target/debug/bundle/macos/Codex Switch.app/Contents/MacOS/codex-switch-app"
smoke_root=$(mktemp -d /tmp/codex-switch-smoke.XXXXXX)
mkdir -p "$smoke_root/work/app" "$smoke_root/codex-home" "$smoke_root/codex_switch_data"
cd "$smoke_root/work/app"
CODEX_HOME="$smoke_root/codex-home" "$app_binary"
```

当前数据目录算法会把空白 profile 数据放在 `smoke_root/codex_switch_data/`。从托盘 Exit 退出。该检查只证明外壳能启动；真实账号和系统控制必须在移植后另行验收。

## 3. 分支与两台机器的同步

- `main`：已集成的源码。合并以同一提交的双平台结果为依据。
- `codex/cross-platform-foundation`：本次准备分支。
- `feat/<功能名>` / `fix/<问题名>`：后续每个功能或修复使用一个短期分支。
- 平台适配属于同一个功能，不设长期 `windows`、`macos` 分支。

功能开始时从最新 main 建分支，先填 [FEATURE.md](templates/FEATURE.md)，明确行为和两端验收条件，再实现。一个人负责公共接口与集成；另一台机器先同步、构建和验收。若同时写代码，事先分配文件范围，提交后让对方 fetch、检查差异再同步，避免两人各自改写共享 `app.js` / `lib.rs`。

Windows 首次获取准备分支（分支已推送后）：

本轮可直接按 [Windows 接手步骤](WINDOWS_HANDOFF.md) 操作；其中包含已通过构建的提交号、产物入口和空白数据隔离运行方式。

```powershell
git clone --branch codex/cross-platform-foundation https://github.com/Tinkerzyx/Codex-Switch.git
cd Codex-Switch/codex-switch-app
npm ci
npm run check
npm test
npm run build:debug
git rev-parse HEAD
```

已有克隆且工作区干净时：

```sh
git fetch origin
git switch <功能分支>
git pull --ff-only
git rev-parse HEAD
```

有本地修改先提交到自己的分支或妥善暂存，再同步。共享分支不用 force push；并行修改通过合并提交或独立短期分支的 PR 汇总。不要复制另一台机器的 `node_modules`、`target` 或账号数据。

## 4. 每个需求的完成流程

1. 写清用户操作、预期结果、两端差异、异常情况、是否涉及已有数据。
2. 在同一功能分支实现共享逻辑及所需系统适配，新增有意义的测试，保留固定 IPC 和数据格式。
3. 本地执行 `npm run check`、`npm test`、原生调试打包；记录实际测试数量。
4. 推送功能分支并创建 PR；CI 分别在 Windows x64、macOS ARM64、macOS Intel 上检查和打包。
5. 下载 CI 产物或在本机从同一 SHA 构建；Mac 侧和 Windows 实机分别运行验收用例，记录系统版本、架构、SHA、截图和失败现象。
6. 修复后两端同步新 SHA，重跑受影响用例；上一提交的验收不能直接套用到新提交。
7. 双端用例和构建通过后合入 main；正式发布、签名和版本提升按当次发布要求执行。

PR 使用 [.github/pull_request_template.md](../.github/pull_request_template.md)。每个验收项只能写 `pass`、`fail`、`pending` 或说明原因的 `N/A`。CI 覆盖静态检查、测试和原生打包；未执行 GUI/账号操作时不能填“实机功能通过”。

## 5. 双端基础验收范围

| 用例 | 验收要点 |
| --- | --- |
| 首次启动 / 空状态 | 正确尺寸，无账号时可操作，错误提示清晰 |
| 3/4 个 profiles | 单列/双列、侧面板、拖动、尺寸与缩放 |
| 窗口与托盘 | 最小化、隐藏、恢复、菜单、退出、重复启动 |
| 数据路径 | 默认路径、覆盖路径、中文和含空格用户名、无权限目录 |
| 保存 / 重启 | profile 数据正确，退出重开仍保留 |
| 切换 | 切换前备份、回存原账号、恢复目标、只控制目标进程 |
| 新账号登录 | 等待/取消/超时，文件未写完不会保存，异常后可恢复 |
| 删除 | 进入 trash，active 状态一致，不误删其他 profile |
| 额度 | 成功、401/403、超时、无令牌、响应字段缺失；不暴露凭证 |
| 安装与回滚 | 原生包安装/启动、覆盖旧版、保留用户数据、回退版本 |

涉及账号和进程的验收在独立会话或专用测试账号进行，避免终止正在承载开发任务的 Codex。日常自动测试使用合成的 auth/config 和隔离目录；日志、截图、仓库和产物都不能包含真实凭证。

## 6. CI 与回滚

`Desktop checks` 在 PR、main 推送、本次准备分支推送以及手动触发时运行。三个原生作业分别检查、测试、生成调试包，附带 `COMMIT.txt`、Rust 和 Node 版本；Artifacts 保留 14 天。macOS `.app` 先 tar 打包以保留权限与符号链接。CI 不自动发布 Release。

工作流可用后，在仓库 Actions 中选择对应 SHA 的运行。构建失败定位到该平台首个有效错误；构建成功后的 GUI/账号行为继续按验收表检查。

回滚优先 revert 出问题的功能提交，再重新构建两端。涉及数据格式修改时先设计兼容和恢复步骤，并保留原始备份；不要通过清空用户数据完成回滚。
