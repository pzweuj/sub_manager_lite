# Sub Manager Lite

订阅管理服务 API，用于记录订阅、计算开销、账单预警。

**[English](./README_EN.md)**

## 快速启动

```bash
# 修改 docker-compose.yaml 中的 API_TOKEN
docker-compose up -d
```

服务地址: http://localhost:8000

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