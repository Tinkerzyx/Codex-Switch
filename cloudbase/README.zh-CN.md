# Codex Switch CloudBase MVP

本目录包含 `csw_` 业务的第一阶段云端实现：匿名设备身份、管理员验证、加密 profile 库存、订单/交付快照模型、支付确认分配和到期维护任务。

## 云端商品页面

桌面客户端从 `csw_client_api` 的 `bootstrap` 响应取得 `storePageHtml`，放入隔离的商品 iframe。商品页面模板和样式位于 `functions/csw_client_api/store-page.html`、`store-page.css`，沿用原桌面商品区的排版。修改模板后只需运行 `node deploy-functions.js csw_client_api`，用户不必重新安装客户端。

商品 `price`、`feature1Zh`、`feature1En`、`feature2Zh`、`feature2En`、`enabled` 存在云数据库 `csw_products` 中；修改这些字段后，客户端下次打开商品栏会重新获取。套餐标题和注意事项在云端页面模板中修改。月度商品 ID 为 `monthly_30d`，当前价格为 62 CNY。首次启动会把旧的月度商品价格迁移到 62，后续编辑不会被商品种子覆盖。云猫商家后台中 `bpx5pe` 的实际售价需单独保持一致；客户端会在展示付款二维码前校验云端订单价和云猫收银台金额。

## 部署前配置

1. 复制 `cloudbaserc.example.json` 为 `cloudbaserc.json`，填写现有 CloudBase 环境 ID。
2. 在 CloudBase 身份认证 v2 中开启匿名登录。
3. 为匿名用户开放调用 `csw_client_api` 和 `csw_admin_api` 的权限；数据库与云存储禁止客户端直接读写。
4. 将云存储权限设置为完全私有。
5. 运行 `node scripts/csw_generate_secrets.js <管理员密码>`，把输出分别配置到管理员与支付回调函数的环境变量。首次部署也可运行 `node scripts/csw_generate_secrets.js --write`，自动生成初始密码并写入已忽略的 `.env.local` 与 `cloudbaserc.json`。
6. 为 HTTP 云函数配置 CloudBase 服务端访问凭证，例如 `CLOUDBASE_APIKEY`。

需要配置的管理员函数环境变量：

```text
CSW_ADMIN_PASSWORD_SALT
CSW_ADMIN_PASSWORD_HASH
CSW_PROFILE_MASTER_KEY
CSW_ACCOUNT_HASH_KEY
```

`csw_payment_webhook` 需要：

```text
CSW_PAYMENT_WEBHOOK_SECRET
```

不要把这些值提交到 Git，也不要写进 Tauri 客户端。

## 设置或修改管理员密码

在 `cloudbase` 目录运行：

```powershell
npm run set-admin-password
tcb config update fn csw_admin_api
```

第一条命令会隐藏密码输入并要求二次确认，只重算密码盐和哈希，不会轮换 profile 加密主密钥。第二条命令出现环境变量更新方式时选择 `Merge update`。明文密码只保存在已忽略的 `.env.local` 中。

`csw_payment_webhook` 只接受带 `x-csw-signature` 请求头的 JSON。签名值为
`HMAC-SHA256(CSW_PAYMENT_WEBHOOK_SECRET, 原始请求体)` 的十六进制字符串。第三方支付平台的原始回调应先由一个只做验签和字段映射的适配层转换为以下字段，再调用该函数：

当前客户端的自动交付主要由短时轮询实现：对已绑定的云猫交易号调用 `shopApi/Pay/query`，查询结果为成功时由 `csw_client_api` 在事务中占用库存并分配授权，随后客户端领取 profile。下方 webhook 路由已可接收我们自定义的签名消息，但尚未接入云猫原始回调。当前轮询分支只检查查询接口的成功状态，尚未从提供方响应核对实付金额和商品；交易号由客户端提交绑定。上线真实支付前需补齐服务端交易号归属与金额校验，并处理客户端关闭或超时后的对账。

```json
{
  "provider": "catfk",
  "providerEventId": "支付平台事件唯一 ID",
  "tradeNo": "支付平台交易号",
  "orderId": "可选的 csw_orders ID",
  "status": "paid",
  "amount": 62,
  "currency": "CNY",
  "paidAt": "2026-08-25T12:00:00.000Z"
}
```

当前个人版环境的 HTTP Access 回调地址为：

```text
https://ocr-h5-demo-d5g8f2qpsf8c21544-1441954144.ap-shanghai.app.tcloudbase.com/csw-payment-webhook
```

## 部署

```powershell
cd cloudbase
npm run deploy
```

可以只部署一个函数：

```powershell
node deploy-functions.js csw_admin_api
```

部署后，在 CloudBase 控制台根据 `indexes.json` 创建索引。`indexes.json` 是设计清单，不会自动修改云端数据库。

## 订单与交付快照

`csw_orders` 在用户发起结账前冻结 `buyerSnapshot` 与 `productSnapshot`。其中购买者由 CloudBase `uid`、本机安装 ID 和客户端版本共同标识。支付确认后补充 `paymentSnapshot`；分配 profile 时写入 `deliverySnapshot`，其中包含：

```text
profileId / generation
交付时额度与重置时间
配置上传快照时间
授权开始与到期时间
分配前库存状态
配置包内容哈希
```

同一份快照还会复制进 `csw_allocations`，支付原始载荷只保存 SHA-256 摘要，幂等事件保存在 `csw_payment_events`，完整状态变化写入 `csw_audit_events`。这些字段是纠纷追溯依据，不随产品、库存或卡片后续变化而覆盖。

支付确认与库存占用在同一数据库事务中完成。订单金额或币种不一致、重复支付事件、库存已被其他订单占用时，不会生成第二份授权。

客户端商品目录只统计 `status: available` 且适用于对应商品的库存。创建订单草稿时，云函数会在事务中把一份库存从
`available` 改为 `reserved`；只有预留成功后客户端才继续创建收银台订单。支付成功后该库存变为 `occupied`。
尚未生成收银台交易号的失败预留默认保留 20 分钟，之后由 `csw_maintenance` 释放。已经生成交易号的订单继续保留库存，
直到支付对账完成或管理员明确释放，避免用户晚付款时库存已经被二次出售。

授权到期后，`csw_maintenance` 会把库存状态改为 `reclaim_pending`，同时写入
`credentialResetRequired: true`、`credentialResetReason` 和 `credentialResetMarkedAt`。
管理员重新生成登录凭证并上传新一代配置后，应将该标记清除，表示旧凭证已经失效且档案可以重新进入库存。
