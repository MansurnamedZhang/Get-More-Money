# Docker 生产部署

该方案在一台 Linux 服务器上运行前端、API Gateway、Identity Service、
Investment Core 和 Caddy。只有 Caddy 的 80/443 端口暴露到宿主机，SQLite
数据库保存在独立 Docker volume 中。

## 前置条件

- Docker Engine 与 Docker Compose 插件；
- 一个解析到服务器公网 IP 的域名；
- 防火墙允许 TCP 80、443，SSH 仅向可信来源开放。

## 1. 准备配置

```bash
cp deploy/.env.example deploy/.env
openssl rand -hex 32
openssl rand -hex 32
```

把两个随机值分别写入 `INTERNAL_API_TOKEN` 和
`SANYU_CREDENTIAL_MASTER_KEY`，并把 `APP_DOMAIN` 改为实际域名。
`deploy/.env` 已被 Git 忽略，不要提交真实密钥。

## 2. 校验并启动

```bash
docker compose --env-file deploy/.env -f compose.production.yaml config
docker compose --env-file deploy/.env -f compose.production.yaml up -d --build
docker compose --env-file deploy/.env -f compose.production.yaml ps
```

Caddy 会在域名解析生效且 80/443 可访问后自动申请 HTTPS 证书。

## 3. 验证

```bash
curl --fail "https://你的域名/ready"
docker compose --env-file deploy/.env -f compose.production.yaml logs --tail=100
```

首次打开站点时创建管理员。生产环境会启用 Secure Cookie，因此不要绕过
HTTPS 直接访问容器端口。

## 4. 更新

```bash
git pull --ff-only
docker compose --env-file deploy/.env -f compose.production.yaml build
docker compose --env-file deploy/.env -f compose.production.yaml up -d
```

更新前先备份两个数据 volume。数据库迁移会在服务启动时自动执行。

## 数据与密钥

- `sanyu-invest-identity-data`：用户和会话数据库；
- `sanyu-invest-core-data`：投资账本、行情、配置与组合数据；
- `SANYU_CREDENTIAL_MASTER_KEY`：加密 API 凭据；丢失后无法解密已保存密钥；
- `INTERNAL_API_TOKEN`：Gateway 与内部服务之间的认证凭据。

数据库备份和 `SANYU_CREDENTIAL_MASTER_KEY` 必须一起保管，并存放一份加密的
异机备份。不要把 SQLite 数据库或生产环境文件提交到 Git。

## 停止

```bash
docker compose --env-file deploy/.env -f compose.production.yaml down
```

`down` 不会删除命名 volume。不要使用 `down -v`，除非明确要删除全部生产数据。
