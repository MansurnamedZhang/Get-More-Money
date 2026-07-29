# SANYU INVEST · 个人投资管理系统

本地优先、单用户使用的投资管理系统。Rust + Axum 提供受登录保护的 API 和组合计算，React + TypeScript 提供完整操作界面，SQLite 是唯一业务真源。

> 项目仍在持续开发中。请先使用测试数据验证导入、计算和备份流程，不要把页面结果直接作为交易或税务依据。

当前版本包含：

- 首次使用创建本地管理员，Argon2 密码哈希与 HttpOnly 会话；
- 券商、银行、基金平台、交易所和自托管钱包账户；
- 股票、ETF、基金、债券、现金、黄金、虚拟货币和稳定币标的；
- 多分录不可变账本，流水编辑自动冲销并重记；
- CSV 预览、校验、幂等去重与批量导入；
- 可切换核心报告币种，首次启动初始化 11 种主流货币的 110 组交叉汇率，并支持手动刷新；
- 手工价格、汇率，以及腾讯美股/港股行情、CoinGecko、Frankfurter 和通用 JSON 定时采集；
- 自动识别 USD/HKD 股票与 ETF，美股/港股默认每 5 分钟后台刷新，页面每 60 秒读取最新行情；
- API 数据管理器自动识别缺失/陈旧价格和缺失汇率，支持采集器增删查改、启停、立即运行和历史追溯；
- 移动平均成本、持仓、市值、已实现/未实现盈亏、XIRR、资产配置和集中度风险；
- IPS、目标配置、决策日志、周期复盘、分类、投资逻辑、对账和审计记录；
- JSON 导出及 SQLite 文件级备份。

详细说明：[微服务架构](docs/microservices-architecture.md) · [系统设计](docs/system-design.md) · [后端 API](docs/backend-api.md)

## 架构

```text
浏览器 :3000
   └── API Gateway :3001
       ├── Identity Service :3101 ── data/services/identity.db
       └── Investment Core :3100 ── data/services/investment-core.db
```

前端只访问 API Gateway。各服务默认仅监听本机回环地址，Identity Service 与 Investment Core 分别维护自己的 SQLite 数据库。

## 环境要求

- Windows 10/11 与 PowerShell 5.1 或更高版本；
- Rust stable（项目使用 Rust 2024 edition）；
- Node.js 22.13.0 或更高版本；
- npm 10 或更高版本。

## 本地启动

首次构建并启动完整开发栈：

```powershell
.\scripts\run-microservices.ps1
```

如果 PowerShell 执行策略阻止脚本运行：

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\scripts\run-microservices.ps1
```

日常停止和重启：

```powershell
.\scripts\stop-microservices.ps1
.\scripts\run-microservices.ps1 -SkipBuild
```

需要丢弃本地开发数据时：

```powershell
.\scripts\reset-local-data.ps1
```

打开 `http://localhost:3000`。首次进入会要求创建本地管理员；系统不会设置默认密码。浏览器只访问 `127.0.0.1:3001` 的 API Gateway，内部服务监听 `3100` 和 `3101`。服务数据库统一位于 `data/services`。

## 项目结构

```text
backend/                  投资核心服务与数据库迁移
services/api-gateway/     API 网关
services/identity-service/ 身份与会话服务
services/contracts/       跨服务契约
frontend/                 React + TypeScript Web 界面
scripts/                  本地启动、停止和数据重置脚本
docs/                     架构、系统设计与 API 文档
```

## 验证

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings

cd frontend
npm run build
```

TWR、回撤和波动率需要连续历史价格与估值覆盖；数据不足时系统明确显示“数据不足”，不会生成模拟指标。

## 数据与安全

- `.env`、SQLite 数据库、凭据密钥、日志、备份与运行时文件均被 Git 忽略；
- API 凭据只应使用只读权限，不要提交真实密钥或包含个人投资数据的导出文件；
- `reset-local-data.ps1` 会删除本地开发数据库，运行前请确认已经备份；
- GitHub Actions 会检查 Rust 格式、Clippy、测试以及前端构建与渲染测试。

## 许可证

当前仓库尚未附带开源许可证。公开发布源码不等于授予复制、修改或分发权；如果准备接受外部使用或贡献，请在发布前选择并添加合适的许可证。
