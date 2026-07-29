# SANYU INVEST 前端

个人投资管理系统的 Web 界面，采用 React、TypeScript 和 Vinext。

## 本地运行

```powershell
Copy-Item .env.example .env
npm.cmd run dev
```

前端默认运行在 `http://localhost:3000`，并连接 `http://localhost:3001/api/v1` 的 Rust 后端。若使用 `127.0.0.1` 打开前端，运行时会自动把本地 API 主机同步为 `127.0.0.1`，避免会话 Cookie 因主机名不一致而失效。后端不可用时，界面会显示连接错误与重试入口。

## 构建

```powershell
npm.cmd run build
```
