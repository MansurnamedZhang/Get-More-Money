# SANYU INVEST 微服务架构

## 1. 重构目标

这次重构不采用“多个进程共享一张数据库”的伪微服务。每个完成迁移的服务必须拥有独立数据、独立迁移、独立健康检查和版本化契约。前端只认识 API Gateway，不感知后端服务拆分。

当前采用绞杀者迁移：

```text
React 前端
    │  http://localhost:3001/api/v1
    ▼
API Gateway
    ├── Identity Service ── identity.db
    └── Investment Core 兼容边界 ── data/services/investment-core.db
            ├── 账本与资产目录
            ├── 组合计算
            ├── 市场数据（待迁出）
            ├── 计划与决策（待迁出）
            └── 审计导出（待迁出）
```

网关已经为 Market Data、Planning、Audit 配置独立路由槽位。领域迁移完成时，只需修改对应服务 URL，前端接口不变。

## 2. 服务边界

| 服务 | 数据所有权 | 同步接口 | 发布事件 | 当前状态 |
|---|---|---|---|---|
| API Gateway | 无业务数据 | 统一 `/api/v1`、会话校验、路由 | 无 | 已完成 |
| Identity Service | users、auth_sessions | `/auth/*`、内部会话验证 | `identity.user.created` | 已完成 |
| Investment Core | accounts、instruments、transactions、transaction_legs | 账户、标的、账本、组合 | 账本与目录事件 | 兼容边界 |
| Market Data Service | prices、fx_rates、collectors、sync_runs | 行情、汇率、采集器 | `market.price.updated` | 路由槽位已就绪 |
| Planning Service | policy、targets、decisions、reviews | 策略、目标、决策、复盘 | `planning.policy.updated` | 路由槽位已就绪 |
| Audit Service | 审计投影、导出任务 | 审计摘要与导出 | 不发布业务命令 | 路由槽位已就绪 |

`services/service-map.json` 是可被工具读取的拓扑清单；`services/contracts/asyncapi.yaml` 是事件契约。

## 3. 强制架构规则

1. 服务不得直接查询其他服务的数据库。
2. 写操作只进入拥有该聚合的服务。
3. 跨服务读取优先使用本地投影；实时一致性确有必要时才同步调用。
4. 跨服务写入使用 Outbox 事件，不使用分布式事务。
5. 每个事件包含 `event_id`、`event_version`、`correlation_id` 和聚合标识。
6. 消费者以 `event_id` 幂等，允许事件至少一次投递。
7. 网关只负责认证、路由和协议转换，不包含投资业务规则。
8. 内部服务默认仅监听 `127.0.0.1`；非本地环境必须更换内部令牌并启用安全 Cookie。
9. Investment Core 在兼容模式下也必须校验 `x-internal-token`，禁止绕过网关直接访问业务接口。
10. 所有服务统一提供根路径 `/health`；网关 `/ready` 聚合全部逻辑服务的可用状态。

尚未独立部署的市场、规划和审计逻辑服务暂时映射到 Investment Core，但它们已经拥有独立路由与就绪检查槽位。后续只需修改对应的 `*_SERVICE_URL`，不需要改动前端或公共 API。

## 4. 请求链路

```text
浏览器 Cookie
    │
    ▼
Gateway ──► Identity /internal/auth/validate
    │              │
    │              └── 返回 actor id / username
    │
    └──► 领域服务
          x-correlation-id
          x-actor-id
          x-actor-name
          x-internal-token
```

客户端不能指定内部身份头；网关会移除客户端传入值并重新生成。所有响应都返回 `x-correlation-id`，便于跨服务排障。

## 5. 数据一致性

投资组合是跨账本、价格、汇率和策略的读模型。最终结构不在请求时跨四个服务做大型联表，而是维护 Portfolio Projection：

```text
ledger.transaction.* ─┐
catalog.instrument.* ─┼──► Portfolio Projection ──► /portfolio/summary
market.price.* ───────┤
planning.policy.* ────┘
```

投影允许短暂最终一致，但响应必须携带 `calculated_at` 和各数据源版本。关键写入先落本服务数据库和 outbox，再异步发布。

## 6. 迁移顺序

### 阶段 1：统一入口与身份独立（当前）

- 前端只访问 Gateway。
- Identity Service 拥有独立 SQLite。
- 原后端改为内部 Investment Core，关闭自身登录校验，由 Gateway 注入可信身份。

### 阶段 2：Market Data

- 迁移价格、汇率、采集器和调度。
- 建立 instruments 最小投影，仅保存采集所需标识。
- 发布价格与汇率更新事件。

### 阶段 3：Planning

- 迁移 policy、targets、decisions、reviews。
- 使用 instrument id 作为外部引用，不建立跨库外键。
- 组合服务消费目标和风险边界事件。

### 阶段 4：Portfolio Projection 与 Audit

- 从 Core 中分离只读组合投影。
- Audit Service 消费全部领域事件，生成不可变审计流和导出包。
- 移除兼容 Core 中已经迁出的表与路由。

## 7. 本地运行

本机没有 Docker 依赖。开发编排使用 PowerShell 与独立 SQLite 文件：

```powershell
.\scripts\run-microservices.ps1 -ResetData
```

停止：

```powershell
.\scripts\stop-microservices.ps1
```

清空开发数据：

```powershell
.\scripts\reset-local-data.ps1
```

端口：

- `3000`：前端
- `3001`：API Gateway
- `3100`：Investment Core 内部服务
- `3101`：Identity Service

## 8. 未来部署

SQLite 适合当前本地开发。每个服务已经按数据库所有权拆开，迁往 PostgreSQL 时按服务逐一替换连接层，无需一次性迁移全系统。进入多人或云部署前，再引入：

- PostgreSQL：每个服务独立数据库或独立 schema；
- NATS JetStream：领域事件和持久订阅；
- OpenTelemetry：统一 trace、metric、log；
- Secrets Manager：内部令牌与外部行情凭据；
- 独立发布流水线：按服务变更范围构建和部署。
