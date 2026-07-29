# Rust 后端 API（V1）

基础地址：`http://localhost:3001/api/v1`。除健康检查与认证接口外，所有接口都要求有效的 HttpOnly 会话 Cookie。金额、数量、价格和权重在 JSON 中使用十进制字符串；时间使用 RFC 3339 并统一存为 UTC。

## 认证

- `GET /auth/status`：检查是否需要首次设置、当前是否登录；
- `POST /auth/setup`：仅在没有用户时创建本地管理员；
- `POST /auth/login`、`POST /auth/logout`、`GET /auth/me`。

密码使用 Argon2 哈希，数据库不保存明文密码；会话令牌只以 SHA-256 摘要入库。

## 核心资源

- 账户：`GET/POST /accounts`，`GET/PUT/DELETE /accounts/{id}`；
- 标的：`GET/POST /instruments`，`GET/PUT/PATCH/DELETE /instruments/{id}`；`PATCH` 停用或恢复，`DELETE` 仅永久删除没有账本流水的空标的；
- 区块链网络：`GET/POST /blockchain-networks`，`GET/PUT/PATCH/DELETE /blockchain-networks/{id}`；投资标的可多选网络并以逗号分隔的网络代码保存，已被标的引用的网络不能改代码或永久删除；
- 流水：`GET/POST /transactions`，`GET/PUT/DELETE /transactions/{id}`（编辑会冲销并重记，删除会作废并保留反向分录）；
- 价格：`GET/POST /prices`，`DELETE /prices/{instrument_id}`，`POST /prices/refresh-crypto-usd` 添加缺失的主流虚拟货币/稳定币标的并刷新美元价格；
- 汇率：`GET/POST /fx-rates`，`POST /fx-rates/refresh-major` 刷新主流货币交叉汇率；
- 组合：`GET /portfolio/summary`；
- 设置：`GET/PUT /settings`；
- IPS：`GET/PUT /policy`；
- 目标、决策、复盘：`/targets`、`/decisions`、`/reviews`；
- 分类、投资逻辑、对账、审计：`/classifications`、`/theses`、`/reconciliations`、`/audit-logs`。

确认流水的 `PUT` 不会覆盖原记录：服务在同一数据库事务中把原流水标记为已冲销、写入反向分录，再创建正确流水。

## CSV 导入

`POST /imports/transactions?commit=false&source=csv` 预览，确认后把 `commit` 改为 `true`。请求体为 `text/csv`，一笔业务可由多行同名 `transaction_group` 组成。

字段：

```text
transaction_group,transaction_type,trade_at,account_id,instrument_id,leg_type,quantity,unit_price,price_currency,memo,external_id
```

系统按 `source + external_id` 去重，并按完整文件 SHA-256 防止同一批次重复提交。

## 定时 API

- 数据源：`GET/POST /data-sources`，`GET/PUT/DELETE /data-sources/{id}`；
- 任务：`GET/POST /sync-jobs`，`GET/PUT/DELETE /sync-jobs/{id}`；
- 立即执行：`POST /sync-jobs/{id}/run`；
- 运行记录：`GET /sync-runs`。
- API 采集器：`GET/POST /api-collectors`，`GET/PUT/DELETE /api-collectors/{id}`；
- 立即运行采集器：`POST /api-collectors/{id}/run`。

公开数据支持 `tencent_quote`、`coingecko_simple`、`frankfurter` 和 `generic_json`。`tencent_quote` 使用 `usAAPL`、`hk00700` 形式的行情代码；通用 API 只允许公网 HTTPS，拒绝本机和私有网络地址。账户余额和流水适配器只允许使用只读凭据引用，不能包含下单或提币权限。

前端“数据 → API 数据管理器”会根据真实持仓识别缺失或陈旧的价格、缺失的报告币种汇率，并用向导同时创建数据源和同步任务。后台自动识别币种为 USD/HKD 的股票与 ETF，分别映射为美股、港股行情代码并创建 5 分钟刷新任务；新增标的会在 30 秒内完成发现，已删除或暂停的采集器不会被强制重建。任务也可立即手动运行。

采集器接口把一个数据源与一个同步任务作为整体提供增删查改。删除采用归档语义：采集器立即停止调度并从列表隐藏，但已有运行记录和已写入的价格、汇率仍保留。

后端首次启动且尚无主流汇率时，会从 Frankfurter 的 ECB 数据源初始化 CNY、USD、EUR、GBP、JPY、HKD、CHF、CAD、AUD、SGD、NZD 的全部双向交叉汇率。之后由用户在“数据 → 行情与汇率”手动刷新；汇率属于最近发布的日频参考值，不是实时交易报价。

“数据 → 行情与汇率”还可手动从 CoinGecko 一次刷新 BTC、ETH、BNB、SOL、XRP、ADA、DOGE、TRX、AVAX、DOT、USDT、USDC、DAI、FDUSD、PYUSD 的美元价格。缺失标的会自动创建，已有标的按代码复用；这些数据写入标的价格历史，不会作为法定货币交叉汇率，也不会连接交易账户或执行交易。

## 错误格式

```json
{
  "error": {
    "code": "validation_error",
    "message": "买卖交易必须同时包含资产分录和现金分录"
  }
}
```

主要状态码：`400` 校验失败、`401` 未登录、`404` 不存在、`409` 唯一性或幂等冲突、`502` 外部 API 失败、`500` 数据库错误。
