# 当前权威基线：main / 1.8.0

2026-11-28 已核对 GitHub：PR #1 和 PR #3 均已合入 `main`，当前基线为 `64fe06d`（1.8.0）。后续两端从最新 `origin/main` 协作，使用同一版本及三平台 CI。网页会话管理增量在独立 PR 中审查，规则和 Mac 待验收项目见 [维护约定](WEB-SESSION-MAINTENANCE.md)。下文保留此前的基线对齐和验收历史，不再表示 PR #3 尚未合并。

## 历史基线：PR #1 / 1.7.7

核对日期：2026-11-26。用户明确指定以 PR #1 为主。

- PR：https://github.com/Tinkerzyx/Codex-Switch/pull/1
- 分支：`codex/macos-port`
- 最终源码提交：`06048b1e3bc72bf54868e5d17469035bf4f57d61`
- 本机下载的 1.7.7 预览包记录：`8750d83cfd016c932890b016e827452fdd396f37`，为同一 PR 的预览合并提交。
- 原 GitHub main `97bbd4b` 的源码仍标记 0.1.0，不适合作为后续移植依据。
- 当前工作分支：`feat/macos-profile-1.7.7`，直接从 PR #1 的真实提交建立。

已核实 PR #1 有共享 Rust HTTP、按窗口时长识别的双周期额度解析、跨平台时间、认证内容校验、平台适配层和 Windows 现有功能。继续复用该分支的业务与界面。

验证：npm ci 成功；35 个前端/发布脚本测试通过；npm run version:check 确认为 1.7.7。远端原生 CI 已核实 success：https://github.com/Tinkerzyx/Codex-Switch/actions/runs/45770509531 。本次对齐没有重新构建或替换安装应用。

## 保留的前序工作

- 旧版上的保存实现提交仍在 `feat/save-current-profile`，提交 `50ec133`。
- 尚未完成的时间和额度改动在 stash `Preserve profile timestamp and quota work before aligning with PR 1`。其中一次旧测试运行有临时目录冲突导致失败，不作为验收通过证据。
- 实际保存的账号档案位于 `~/Documents/CodexSwitch`，包括 profiles.json 和双文件 profiles；已核对源文件未变化。切换源码未移动或修改这些档案。

## 下一步仍需整合的用户要求

1. macOS 档案库使用 `~/Documents/CodexSwitch`（PR #1 当前默认是 Application Support，需适配并兼容已有档案）。
2. 保留 More → Save Current as New，自动从 auth.json 解析名称，无新增按钮和命名输入框。
3. 在成熟实现上验证真实 usage API 与卡片展示。已用实际档案只读查询 HTTP 200，响应包含 plan_type、rate_limit.primary_window/secondary_window、used_percent、reset_at。
4. 核对档案创建/更新时间、认证刷新时间、额度重置时间的语义与格式；兼容前序实现写入的 Unix 秒数字符串。
5. 保持当前 profile 数据完整，账号切换仍待独立实机验证。

## 1.7.8 验证版进展

基于本基线的 1.7.8 已构建、签名校验、安装并核验界面及真实额度刷新。文稿目录已接入，已有档案可读取；源 auth/config 与旧数据保持完整。详细结果和待验证的 A/B 切换用例见 [验证记录](MACOS-1.7.8-VERIFICATION.md)。当前仍为独立草稿 PR #3，尚未合并 main。

## 1.8.0 用户验收与合并授权

2026-11-26 用户确认网页登录、工作空间选择、Profile 生成与信息、两个账号切换、桌面应用关闭重启后的身份均正确。用户授权将基础可用版本合入 main；小功能尚未验收，保留后续逐项修复。验证源码为 `154732d`，三平台 CI 成功。详见验证文档的最终验收章节。历史“待用户验证/不自动合并”状态由本次明确授权更新。
