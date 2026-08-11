# Codex Switch

[English](README.md) | 中文

一个轻量的 Windows 桌面应用，用于切换本地 Codex / ChatGPT 登录快照。

Codex Switch 会为不同账号保存独立的 `.codex/auth.json` 和 `.codex/config.toml` 本地副本，并在需要时快速恢复。它适合经常在多个 Codex 可用账号之间切换、但不想使用代理网关的用户。

## 功能

- 将当前 Codex 登录文件保存为一个 profile
- 通过恢复本地 `auth.json` 和 `config.toml` 切换到选中的 profile
- 新建 profile 登录流程：清理当前本地登录文件，打开 ChatGPT/Codex，等待登录文件稳定后自动保存新 profile
- 删除 profile 时移动到本地 trash，而不是直接永久删除
- 查看原始 `auth.json` 和 `config.toml` 内容
- 刷新已保存 profile 的 usage 信息
- 通过 Windows StartApps / AppsFolder 自动发现并启动 ChatGPT/Codex
- 切换前硬关闭 ChatGPT/Codex 进程组
- 支持托盘图标、profile 切换、显示/隐藏窗口

## 下载

普通用户建议从 GitHub Releases 下载最新 Windows 安装包：

```text
Codex Switch_0.1.0_x64-setup.exe
```

构建后也可以使用便携版 exe：

```text
codex-switch-app.exe
```

## 数据安全

本应用会管理可能包含有效登录凭证的文件。

不要上传或分享你的本地数据目录：

```text
codex_switch_data/
```

不要分享这些文件：

```text
auth.json
config.toml
```

仓库里的 `.gitignore` 已经排除了本地 profile 数据、构建产物和临时文件。

## 工作原理

Codex Switch 默认使用本地 Codex 配置目录：

```text
%USERPROFILE%\.codex
```

如果设置了 `CODEX_HOME` 环境变量，则优先使用 `CODEX_HOME`。

每个 profile 会保存这些文件的本地快照：

```text
auth.json
config.toml
```

切换 profile 时，应用会：

1. 将当前 active 文件保存回当前 active profile
2. 停止 ChatGPT/Codex 进程
3. 备份当前 `.codex` 文件
4. 恢复选中 profile 的文件
5. 重新启动 ChatGPT/Codex

新建登录时，应用会清理当前本地登录文件，启动 ChatGPT/Codex，等待 `auth.json` 和 `config.toml` 出现并稳定，然后从 `auth.json` 解析账号信息并保存为新 profile。

## Usage 查询

Usage 刷新功能会读取对应 profile 的本地 `auth.json`，并请求：

```text
https://chatgpt.com/backend-api/wham/usage
```

这不是公开稳定 API。如果 OpenAI 调整接口或响应格式，usage 显示可能失效，但 profile 切换功能仍然可以继续工作。

## 开发

需要：

- Windows
- Node.js
- Rust toolchain
- WebView2 Runtime

开发运行：

```powershell
cd codex-switch-app
npm install
npm run tauri dev
```

构建：

```powershell
npm run tauri build
```

构建产物：

```text
codex-switch-app/src-tauri/target/release/codex-switch-app.exe
codex-switch-app/src-tauri/target/release/bundle/nsis/Codex Switch_0.1.0_x64-setup.exe
```

如果构建失败并提示 exe 被占用，请先从托盘退出 Codex Switch，再重新构建。

## 仓库结构

```text
codex-switch-app/
  src/
    index.html
    styles.css
    app.js
    assets/
  src-tauri/
    src/
      main.rs
      lib.rs
    tauri.conf.json
```

本地开发时可能还保留旧原型文件，但公开版本主要使用 `codex-switch-app/`。

## 协议

MIT
