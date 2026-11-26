# macOS 1.7.8 验证版

基于 PR #1 的 1.7.7，提交 `06048b1`。独立分支 `feat/macos-profile-1.7.7`；通过实机验收后再合并。

## 共享与平台差异

| 范围 | 本版做法 |
| --- | --- |
| 主界面、More、卡片和操作入口 | 沿用 1.7.7 共享前端 |
| 账号名称、Profile、索引、备份与恢复 | 沿用共享 Rust 实现 |
| 额度 | 沿用共享 HTTP 请求及按窗口时长识别 5 小时/7 天的解析 |
| 切换 | 沿用操作锁、分阶段恢复记录、源/目标身份核对和备份流程 |
| macOS 档案目录 | 按用户要求改为 `~/Documents/CodexSwitch` |
| 应用发现与启停、代理、登录容器 | 使用已有 macOS 平台实现，继续实机验证 |
| Windows 悬浮窗/原生二维码捕获 | 保留原实现；macOS 使用已有对应能力或浏览器回退 |

本轮兼容此前写入的 Unix 秒数档案时间，将其显示为本机时间；新档案沿用 1.7.7 日期格式。额度 Reset 在请求返回前显示未知，避免误用认证刷新时间。现有旧 Application Support 数据库保留，两个档案库不自动合并。

## 自动检查

- 35 个前端/发布脚本测试：pass。
- Rust：26 pass，1 个已有 live X 网络测试按原设计 ignored。
- 原生应用构建、ad-hoc 签名校验、安装与 1.7.8 界面：pass。
- 真实 usage 刷新：pass，套餐、双周期额度及重置时间正常显示。
- 读取已有文稿目录档案：pass；源 auth/config 校验和与升级前一致，旧 Application Support 数据完整保留。
- Windows / Intel Mac 本轮 CI：pending。

## 用户实机验收

本轮代理只检查启动、档案读取、额度及安装完整性。A/B 切换会重新启动 ChatGPT/Codex，请在当前开发任务结束后验证。

| 编号 | 操作 | 应看到的结果 | 当前结果 |
| --- | --- | --- | --- |
| A1 | 启动并检查版本 | 显示 1.7.8，保留 1.7.7 的完整界面 | pass |
| A2 | More → Save Current as New | 自动解析名称，文稿目录新增 auth.json/config.toml 及索引 | pending |
| A3 | Refresh / 自动刷新 | 套餐、5 小时/7 天额度和重置时间来自后台接口 | pass |
| A4 | 退出并重开 | 档案仍存在，额度可再次刷新 | pending |
| B1 | 通过已有新账号入口登录第二个测试账号 | A、B 两份档案分别保留，登录身份正确 | pending |
| B2 | 激活 A | 先备份，再恢复 A；桌面应用实际账号和额度属于 A | pending |
| B3 | 激活 B 并重启桌面应用 | 桌面实际账号和额度仍属于 B，A 档案不被改成 B | pending |
| B4 | 检查切换备份和原档案 | 两个账号身份、配置与恢复记录一致 | pending |
| B5 | 取消登录/模拟异常后重开 | 当前账号和已保存档案可恢复，不混写 | pending |

回传时带版本、操作编号、实际现象；截图隐藏邮箱和账号标识，不粘贴 auth/config 内容。成功判断以桌面应用实际账号、额度及重启后的结果为准，不能只看“切换成功”提示。

## 合并条件

自动检查及两个平台的相关构建通过；A、B 实机切换和重启保持通过；失败路径可恢复；保留原有 Windows 行为。之后按 PR #1 与验证版依赖顺序合入 main，当前不自动合并。

本机已安装到 `/Applications/Codex Switch.app`，上一版应用包保留为同目录的 previous 备份。档案与源文件备份保留在 `~/Documents/CodexSwitch/backups/verification-1.7.8-*`。构建源码提交为 `19e3be0`，后续文档记录不改变二进制源码。草稿 PR：https://github.com/Tinkerzyx/Codex-Switch/pull/3 。
