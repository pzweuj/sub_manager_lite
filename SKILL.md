---
name: sub_manager_lite
description: 订阅管理服务 - 记录订阅、计算开销、账单预警、自动续期
trigger: 用户询问订阅相关操作时调用，如"记录订阅"、"更新订阅"、"查看花费"、"即将扣费"、"过期订阅"、"取消订阅"、"恢复订阅"、"删除订阅"、"从Wallos迁移"
---

# Sub Manager Lite

订阅管理 API 服务，用于记录订阅、计算花费、账单预警、自动续期。

**[English Version](./SKILL_EN.md)**

## 配置管理

### 配置文件路径

配置保存在：`~/.sub_manager_lite_setting.json`

### 首次调用流程

1. 检查 `~/.sub_manager_lite_setting.json` 是否存在
2. 若不存在或配置为空，向用户询问：
   - 服务 URL（如 `http://localhost:8000`）
   - API Token（用户设置的 `API_TOKEN` 值）
3. 将配置写入 `~/.sub_manager_lite_setting.json`：
   ```json
   {
     "url": "http://localhost:8000",
     "token": "your-token-here"
   }
   ```
4. 告知用户："配置已保存，后续调用无需重复提供"

### 后续调用流程

1. 读取 `~/.sub_manager_lite_setting.json`
2. 使用文件中的 `url` 和 `token` 进行请求
3. 若文件不存在或配置无效，按首次流程处理

### 更新配置

用户可随时要求更新配置：
```
更新 Sub Manager 的服务地址为 http://192.168.1.100:8000
```
Agent 更新配置文件并告知用户。

## 鉴权

所有请求需携带请求头：
```
X-API-Token: <Token>
```

## 接口

| 方法 | 路径 | 功能 |
|------|------|------|
| POST | `/subscriptions/` | 新增订阅 |
| PUT | `/subscriptions/{id}` | 更新订阅（价格、到期日等） |
| PUT | `/subscriptions/{id}/cancel` | 取消订阅 |
| PUT | `/subscriptions/{id}/restore` | 恢复已取消订阅 |
| DELETE | `/subscriptions/{id}` | 删除订阅记录 |
| GET | `/subscriptions/` | 查询列表（支持 name、status 过滤） |
| GET | `/subscriptions/stats` | 费用统计（支持 period 参数，自动排除过期订阅） |
| GET | `/subscriptions/upcoming` | 账单预警（支持 days 参数） |
| GET | `/subscriptions/expired` | 过期订阅列表 |

## 接口参数

### GET /subscriptions/stats
- `period`: `monthly`（月度）或 `yearly`（年度），默认 monthly
- `base_currency`: 基准货币，默认 CNY，可选 USD、EUR 等
- **注意**：只统计未过期的活跃订阅
- **多货币支持**：自动获取汇率转换，汇率不可用时返回货币分组

### GET /subscriptions/upcoming
- `days`: 预警天数，默认 7，范围 1-365

### GET /subscriptions/expired
- 无参数
- 返回状态为 Active 但到期日已过期的订阅
- 按到期日降序排列（最久过期的排在前面）

## 请求示例

新增订阅：
```json
POST /subscriptions/
{
  "name": "ChatGPT Plus",
  "price": 20.0,
  "currency": "USD",
  "billing_cycle": "Monthly",
  "ending_date": "2024-12-31",
  "category": "生产力",
  "auto_renew": true
}
```

更新订阅：
```json
PUT /subscriptions/1
{
  "price": 25.0,
  "ending_date": "2025-01-31"
}
```

统计返回：
```json
{
  "total_cost": 1650.0,
  "base_currency": "CNY",
  "period": "yearly",
  "active_count": 5,
  "breakdown_by_category": {"生产力": 1440.0, "娱乐": 210.0},
  "breakdown_by_currency": {"USD": 240.0, "CNY": 100.0},
  "rates_used": {"USD": 7.25, "CNY": 1.0},
  "rates_available": true
}
```

**注意**：`rates_available=false` 时表示无法获取汇率，此时 `total_cost` 仅统计基准货币订阅，用户需参考 `breakdown_by_currency`。
```

## 计费周期

| 值 | 月度计算 | 年度计算 |
|----|----------|----------|
| Monthly | 价格 | 价格 × 12 |
| Yearly | 价格 ÷ 12 | 价格 |
| Weekly | 价格 × 4.33 | 价格 × 52 |

## 数据模型

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| name | string | ✓ | 订阅名称 |
| price | float | ✓ | 价格（正数） |
| currency | string | 默认 CNY | 货币代码 |
| billing_cycle | enum | 默认 Monthly | Monthly/Yearly/Weekly |
| ending_date | date | ✓ | 到期日 YYYY-MM-DD |
| category | string | 默认 其他 | 分类标签 |
| status | enum | 默认 Active | Active/Canceled |
| auto_renew | bool | 默认 false | 是否自动续期 |

## 自动续期

当 `auto_renew=true` 时，订阅到期后系统自动延长一个计费周期：
- Monthly → 延长 30 天
- Yearly → 延长 365 天
- Weekly → 延长 7 天

自动续期任务每天 00:05 执行。

## 从 Wallos 迁移

当用户要求从 Wallos 迁移订阅数据时，按以下步骤操作：

### 前置条件

需向用户获取：
- Wallos 服务 URL（如 `http://your-wallos-instance.com`）
- Wallos API Token（在 Wallos 设置中获取）

### 迁移步骤

1. 调用 Wallos API 获取订阅列表：
   ```
   GET {wallos_url}/api/subscriptions/get_subscriptions.php
   Header: Authorization: Bearer {wallos_token}
   ```

2. 遍历返回的 `subscriptions` 数组，逐条映射并创建：

### 字段映射

| Wallos 字段 | Sub Manager 字段 | 映射说明 |
|-------------|------------------|----------|
| `name` | `name` | 直接映射 |
| `price` | `price` | 直接映射 |
| `currency_id` | `currency` | 需查询 Wallos 货币表或使用默认值 |
| `next_payment` | `ending_date` | 直接映射（YYYY-MM-DD） |
| `category_name` | `category` | 直接映射，空值用"其他" |
| `inactive` | `status` | `inactive=1` → Canceled，否则 Active |
| `auto_renew` | `auto_renew` | 直接映射（1/0 → true/false） |
| `cycle` + `frequency` | `billing_cycle` | 组合推断周期类型 |

### 计费周期推断

Wallos 使用 `cycle` + `frequency` 组合表示周期：

| cycle | frequency | billing_cycle |
|-------|-----------|---------------|
| "Monthly" | 1 | Monthly |
| "Yearly" | 1 | Yearly |
| "Weekly" | 1 | Weekly |
| "Daily" | 30 | Monthly（30天） |
| 数字(天数) | 1 | Monthly（如果 ≤30） |
| 数字(天数) | 1 | Yearly（如果 ≥365） |

**简化规则**：
- 若 cycle 为字符串，直接使用
- 若 cycle 为数字天数：
  - 1-30 → Monthly
  - 31-364 → Monthly（按实际天数折算）
  - ≥365 → Yearly

### 迁移请求示例

对每个 Wallos 订阅，构造 POST 请求：
```json
POST /subscriptions/
{
  "name": "Netflix",
  "price": 15.99,
  "currency": "USD",
  "billing_cycle": "Monthly",
  "ending_date": "2024-12-15",
  "category": "娱乐",
  "status": "Active",
  "auto_renew": true
}
```

### 迁移完成

迁移完成后向用户报告：
- 迁移订阅数量
- 成功/失败数量
- 已跳过的已取消订阅（如有）

## 定时提醒任务

用户可要求 Agent 设置定时提醒，例如：

**用户请求**：
```
每天早上 9 点帮我检查未来 7 天即将扣费的订阅，如果有则提醒我。
```

**Agent 操作**：
1. 设置定时任务（如使用系统 cron 或 Agent 的定时功能）
2. 每天 9:00 执行：
   - 调用 `GET /subscriptions/upcoming?days=7`
   - 若返回列表不为空，向用户发送提醒消息
3. 告知用户任务已设置，提醒方式（如桌面通知、邮件等）

**提醒消息示例**：
```
⏰ 订阅提醒：未来 7 天内有 3 个订阅即将扣费：
1. Netflix - $15.99 - 2024-12-15
2. ChatGPT Plus - $20.00 - 2024-12-18
3. GitHub Copilot - $10.00 - 2024-12-20
总金额：$45.99
```

## 常见场景

| 用户意图 | Agent 操作 |
|----------|------------|
| "记录新订阅" | POST /subscriptions/ |
| "订阅涨价了" | PUT /subscriptions/{id} 更新 price |
| "续费了，更新到期日" | PUT /subscriptions/{id} 更新 ending_date |
| "本月花了多少" | GET /subscriptions/stats |
| "今年花了多少" | GET /subscriptions/stats?period=yearly |
| "下月有什么扣费" | GET /subscriptions/upcoming?days=30 |
| "有哪些过期订阅" | GET /subscriptions/expired |
| "取消订阅" | PUT /subscriptions/{id}/cancel |
| "恢复订阅" | PUT /subscriptions/{id}/restore |
| "删除订阅" | DELETE /subscriptions/{id} |
| "从Wallos迁移数据" | 按迁移流程操作 |
| "设置到期提醒" | 设置定时任务调用 upcoming 接口 |

## 过期订阅处理

当用户查询过期订阅时（GET /subscriptions/expired），返回的订阅需要用户决定处理方式：

| 用户意图 | Agent 操作 |
|----------|------------|
| "仍需要这个订阅" | PUT /subscriptions/{id} 更新 ending_date 或设置 auto_renew=true |
| "不再需要" | PUT /subscriptions/{id}/cancel 或 DELETE /subscriptions/{id} |
| "批量处理过期订阅" | 遍历 expired 列表，询问用户对每个的处理方式 |

## 错误码

| 状态码 | 处理 |
|--------|------|
| 401 | Token 无效，提示用户更新配置 |
| 404 | 订阅不存在 |
| 400 | 请求数据无效或状态冲突 |