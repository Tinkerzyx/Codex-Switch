# 当前结构与双平台边界

基于上游 `97bbd4b`（`main`）检查，2026-11-26。

## 现有源码

```text
codex-switch-app/
  package.json / package-lock.json   npm 命令、Tauri CLI 的锁定依赖
  scripts/toolchain.mjs              补足子进程的 rustup PATH（本次新增）
  src/
    index.html                      主窗口 DOM
    styles.css                      窗口、卡片、侧面板样式
    app.js                          页面状态、事件、登录轮询、额度缓存、IPC
    assets/                         现有按钮图片
  src-tauri/
    Cargo.toml / Cargo.lock         Tauri 2、serde、serde_json
    build.rs                        tauri_build::build()
    src/main.rs                     桌面程序入口
    src/lib.rs                      数据、文件操作、账号解析、系统调用、命令和托盘
    tauri.conf.json                 共享窗口与应用配置
    tauri.windows.conf.json         Windows NSIS 构建目标（本次新增）
    tauri.macos.conf.json           macOS app/dmg 外壳基线（本次新增）
    capabilities/default.json       主窗口能力配置
    icons/                          原有 ICO 与从其转换的 macOS 图标
.github/workflows/desktop.yml        三种原生构建与调试包产物
docs/                               开发、基线和功能验收记录
```

前端没有 React、TypeScript 或 Vite；`frontendDist` 直接指向 `src/`。`app.js` 通过 `window.__TAURI__.core.invoke` 调用 Rust 命令，通过事件接收托盘操作。前后端字段使用 camelCase。

## 当前核心流程

- 保存：复制当前 auth/config 到 profile 目录，更新 `profiles.json`。
- 切换：停止目标进程 → 备份当前文件 → 回存 active profile → 恢复目标 profile → 启动目标应用。
- 新登录：停止目标进程 → 保存 active profile → 将当前文件移至备份 → 启动目标应用 → 前端轮询文件大小稳定后保存 profile。
- 删除：将 profile 移入 trash，再更新索引。
- 额度：解析本地令牌，通过 PowerShell 调用现有 usage 端点，向前端返回套餐和剩余额度。

## 已发现的 macOS 缺口

| 位置 / 行为 | 当前实现 | 后续适配方向 |
| --- | --- | --- |
| `codex_dir()` | `CODEX_HOME` 或 `USERPROFILE/.codex` | 保留覆盖值，macOS 使用用户主目录；增加纯路径解析测试 |
| `locate_data_dir()` | 查找工作目录祖先，或取上两级目录 | 确定安装后稳定的数据目录；兼容旧 Windows 数据；增加测试隔离覆盖值 |
| `now_stamp()` / `now_text()` | PowerShell，失败退回 Unix 秒 | 共享 Rust 时间格式和一致的时区约定 |
| `find_shell_app()` / `start_codex_app()` | StartApps、AppsFolder、Explorer | macOS 应用发现与 `open` 适配；明确目标是 Codex 还是 ChatGPT |
| `hard_stop_codex_app()` | PowerShell + taskkill | 精确限定目标和进程范围，避免误停开发中的 Codex；在独立会话验收 |
| `open_path()` | PowerShell Start-Process | macOS Finder/open 适配 |
| `usage_for_profile()` | PowerShell HTTP 与响应整形 | 提取共享 Rust HTTP/解析逻辑，使用脱敏固定响应测试 |
| 窗口 / 托盘 | Windows 透明窗、双击托盘 | macOS 外壳目前禁用透明；验证菜单栏点击、隐藏/恢复及原生外观 |
| 登录完成判断 | auth > 3KB、config > 2KB，再比较大小 | 实测两端有效文件形态；改用内容解析与稳定性判断 |

这些是审查发现和后续开发项，不表示本次已完成移植。现有恢复流程还需要覆盖复制失败和中断后的恢复，单文件临时写入不等于整组文件事务。

## 后续代码维护方向

按功能逐步提取，避免在准备阶段重写整个 `lib.rs`：

```text
src-tauri/src/
  lib.rs                  Tauri 注册、共享状态、入口装配
  commands/               IPC 参数与结果转换
  profiles/               profile、索引、备份、恢复、兼容
  usage/                  共享 HTTP/解析与错误处理
  platform/
    mod.rs                统一的系统操作接口
    windows.rs            Windows 专用实现，#[cfg(windows)]
    macos.rs              macOS 专用实现，#[cfg(target_os = "macos")]
```

以上目录为约定目标，尚未建立。业务规则、数据格式和 IPC 保持共享；发现应用、打开目录、控制进程等系统行为通过平台实现注入。UI 只有原生交互确实有差异时才分支。
