# Sub Manager Lite

订阅管理服务 API，用于记录订阅、计算开销、账单预警。

**[English](./README_EN.md)**

## 快速启动

```bash
# 修改 docker-compose.yaml 中的 API_TOKEN
docker-compose up -d
```

服务地址: http://localhost:8000

## Rust 本地开发

需要 Rust 1.88 或更高版本。

```bash
cargo test --locked
API_TOKEN=your-strong-token cargo run --release --locked
```

默认数据库为 `sqlite:///./sub_manager.db`；Docker 镜像继续使用 `sqlite:////data/sub_manager.db`。

## 安全注意事项

- `API_TOKEN` 必须设置为强随机值（至少 16 位）。服务启动时会校验，弱 Token 或占位符将拒绝启动。
- 公网部署时务必通过 HTTPS 反向代理（Nginx/Caddy 等）访问，避免 Token 明文传输被窃听。
- 服务内置按 IP 限流，鉴权失败次数过多会返回 429，用于防爆破；该限流基于进程内内存，仅适用于单实例部署。
- `/`、`/docs`、`/redoc` 默认未鉴权，公网部署建议在反向代理层限制文档端点的访问。

## Agent 使用指南

详见 [SKILL.md](./SKILL.md)

## 首次使用

首次调用时，Agent 会询问服务地址和 Token，并自动保存到 `~/.sub_manager_lite_setting.json`。后续调用无需重复提供。

## 常用 Prompt 示例

### 1. 首次配置

```
配置 Sub Manager Lite 服务：
服务地址：http://localhost:8000
Token：your-token-here
```

### 2. 记录订阅

```
帮我记录一个订阅：Netflix，每月 15.99 美元，下次扣费日期是 2024-12-15，分类是娱乐，开启自动续期。
```

### 3. 查看花费统计

```
查看我今年的订阅总花费。
```

### 4. 设置定时提醒

```
每天早上 9 点帮我检查未来 7 天即将扣费的订阅，如果有则提醒我。
```

### 5. 从 Wallos 迁移

```
把我的 Wallos 订阅数据迁移过来。
Wallos 地址：http://your-wallos.com
Wallos Token：wallos-token
```

### 6. 更新订阅

```
Netflix 涨价了，帮我更新价格到 18.99 美元，到期日改成 2025-01-15。
```

### 7. 取消订阅

```
帮我取消 Netflix 的订阅。
```

### 8. 更新服务配置

```
Sub Manager 服务地址改成了 http://192.168.1.100:8000，帮我更新配置。
```
