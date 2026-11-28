# Windows / macOS 网页会话维护约定

本变更基于已合并 PR #1、#3 的 `origin/main`（`64fe06d`，1.8.0），保留 Mac 已验收的登录、CLI 发现及 `~/Documents/CodexSwitch` 档案目录。源码、规则和验证结果通过 GitHub PR 共享；各电脑的 profile、Cookies、认证文件和浏览器数据留在本机。

## 两端统一规则

- `profiles.json` 的 `webSessionId` 表示仍需保留的网页登录会话；档案删除后，对应会话可被回收。
- 当前登录、恢复记录中的登录、使用中的浏览器窗口及窗口销毁后 60 秒内的会话不清理。维护期间拒绝再次打开同一会话，避免缓存回收与浏览器创建竞争。
- 启动和每 6 小时维护一次；删除档案、取消或恢复中断登录后，在 2、12、72、192 秒附近重试。锁定或尚未释放的文件交由后续维护重试。
- 索引或恢复记录缺失/损坏时，无法确认所有权就停止回收。无会话目录的新安装可正常启动。
- 只处理程序生成的 32 位十六进制目录名；不跟随符号链接或 Windows junction，不清理未知目录。
- 有效会话只清显式允许的缓存。Cookies、Local Storage、IndexedDB、会话数据库和 profile 认证文件保留。

## 平台实现

| 行为 | Windows | macOS 14+ |
| --- | --- | --- |
| 浏览器 | WebView2 | WKWebView |
| 有效会话缓存 | `EBWebView` 下显式列出的 Cache、Code Cache、GPU/Shader 缓存等 | `WKWebsiteDataTypeDiskCache` 与 `WKWebsiteDataTypeMemoryCache` |
| 废弃会话 | 删除对应目录 | 先用 `removeDataStoreForIdentifier` 删除原生 store，成功后删除本地标记目录；失败保留标记以便重试 |
| 执行线程 | 后台维护线程；启动阶段在浏览器创建前执行 | 后台线程调度 WebKit 主线程调用；等待异步完成时不持有会话全局锁 |
| 原生标识 | 会话目录 | 继续使用原有 `SHA256("com.codexswitch.app.web-session:" + id)[0..16]`，避免已有登录失联 |
| 安装维护 | 新版 EXE 的 `--cleanup-web-sessions`，持有单实例锁 | 启动后有事件循环再维护，禁止直接删除 `~/Library/WebKit` 的内容 |

Mac 原生 API 不提供本实现所需的删除字节统计，报告中的 `freedBytes` 不代表 WebKit 实际释放量；`clearedWebkitCaches` 记录清理完成的有效 store 数量。旧版本已删除本地标记但遗留的原生 store 无法可靠反推出所属会话，本实现不批量删除无法确认所有权的 store，也不碰默认 store。API 超时后，维护预约仍由原生回调持有，直至实际完成，避免超时后的重入。

最新报告：本机数据目录下的 `diagnostics/web-session-cleanup.json`。`failedEntries` 记录待重试项，不把原生清理失败视为成功回收。

## GitHub 协作与验收

两端都从最新 `origin/main` 建立分支；PR 合入同一条主线，不另维护长期 Windows/Mac 业务分叉。版本由 `scripts/version.mjs` 统一分配，npm/Cargo/Tauri、锁文件、二进制和安装包使用同一版本。

现有 GitHub Actions `Desktop checks` 必须检查 Windows x64、Apple Silicon 和 Intel Mac 的测试及原生安装包构建。Windows 本地测试不能替代 Mac 上的原生编译或真实账号验收。

Mac 端验收（本次 Windows 开发环境无法代做，提交到 PR 后由 Mac 端记录结果）：

- 使用已有两个 profile 启动、查询额度、轮流切换账号，确认无需重新网页登录。
- 在有效网页登录会话存在时执行维护，确认原生 cache 计数更新，Cookies/登录状态仍有效。
- 新建测试 profile 后删除，等待重试；对应原生 store 和本地标记目录应被回收。
- 打开登录窗口时保持未完成状态并触发维护，窗口应正常使用；取消后，等待 60 秒释放宽限和重试，再确认回收。
- 中断登录后重启，确认旧 profile 可恢复，恢复记录未被提前清掉；完成恢复后再回收废弃会话。
- 模拟原生 store 尚在使用或回调失败，确认保留本地标记，后续可重试。
- 在单独测试数据目录放置无效索引、未知目录和符号链接，确认停止或跳过相应清理，不影响链接目标。

2026-11-28 Windows 1.7.8 清理实测：回收 22 个无引用会话、31 个缓存目录，释放 1,390,039,780 字节；清理前后 823 个需保留文件的 SHA-256 一致，升级保留文件校验通过。此记录仅证明 Windows 路径，不表示 Mac 原生路径已完成实机验收。

接口依据：[Apple 原生 store 删除](https://developer.apple.com/documentation/webkit/wkwebsitedatastore/remove(foridentifier:completionhandler:))、[Apple 按类型清理数据](https://developer.apple.com/documentation/webkit/wkwebsitedatastore/removedata(oftypes:modifiedsince:completionhandler:))。

## 2026-11-28 Mac 端执行记录

基于 PR #4 源码 `91cc987`。下载三平台已通过 CI 的 Apple Silicon 1.8.1 包；产物 commit e3969e8 是 main 与 PR #4 的测试合并提交，Git tree 与 PR 源码一致。应用 ad-hoc 签名校验通过，已安装并原生打开 `/Applications/Codex Switch.app`，保留上一版应用。此次使用既有 CI 包，未重新编译或分配新版本。

- 本机前端/版本脚本：38 pass。
- 本机 Mac Rust：29 pass，1 个既有 live 网络测试 ignored。
- 启动后的原生维护报告：clearedWebkitCaches=1，failedEntries=0，removedSessions=0。freedBytes=0 按接口限制不表示未释放缓存。
- 维护前后记录的 4 个 profile/会话标记文件 SHA-256 一致；16 个原生 Cookies/LocalStorage/IndexedDB 相关文件 SHA-256 一致。
- 原生界面版本 1.8.1，两个已有 profile 均显示正常套餐、双周期额度和重置时间，并观察到额度更新。
- 数据备份保留于本机 Documents 下 `CodexSwitch-backup-before-1.8.1-*`，认证数据未提交。

仍待用户配合执行：清理后的双账号实际切换、已有网页登录会话免登录复用、删除测试 profile 的 native store 回收、取消登录/中断恢复及原生失败重试。当前任务已完成拉取、审查、测试、安装和有效会话原生缓存清理验证；不把其余项目标为通过，PR 保持未合并。
