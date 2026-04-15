---
name: sub_manager_lite
description: Subscription management service - track subscriptions, calculate expenses, billing alerts, auto-renewal
trigger: User asks subscription-related operations like "add subscription", "update subscription", "check expenses", "upcoming bills", "expired subscriptions", "cancel subscription", "restore subscription", "delete subscription", "migrate from Wallos"
---

# Sub Manager Lite

Subscription management API service for tracking subscriptions, calculating expenses, billing alerts, and auto-renewal.

**[中文版](./SKILL.md)**

## Configuration Management

### Config File Path

Configuration saved at: `~/.sub_manager_lite_setting.json`

### First-time Call Flow

1. Check if `~/.sub_manager_lite_setting.json` exists
2. If not exists or empty, ask user for:
   - Service URL (e.g., `http://localhost:8000`)
   - API Token (user's `API_TOKEN` value)
3. Write configuration to `~/.sub_manager_lite_setting.json`:
   ```json
   {
     "url": "http://localhost:8000",
     "token": "your-token-here"
   }
   ```
4. Notify user: "Configuration saved, no need to repeat for future calls"

### Subsequent Call Flow

1. Read `~/.sub_manager_lite_setting.json`
2. Use `url` and `token` from file for requests
3. If file not exists or invalid, follow first-time flow

### Update Configuration

User can request config update anytime:
```
Update Sub Manager service URL to http://192.168.1.100:8000
```
Agent updates config file and notifies user.

## Authentication

All requests require header:
```
X-API-Token: <Token>
```

## Endpoints

| Method | Path | Description |
|--------|------|-------------|
| POST | `/subscriptions/` | Add subscription |
| PUT | `/subscriptions/{id}` | Update subscription (price, due date, etc.) |
| PUT | `/subscriptions/{id}/cancel` | Cancel subscription |
| PUT | `/subscriptions/{id}/restore` | Restore canceled subscription |
| DELETE | `/subscriptions/{id}` | Delete subscription record |
| GET | `/subscriptions/` | List subscriptions (filter by name, status) |
| GET | `/subscriptions/stats` | Expense statistics (excludes expired subscriptions) |
| GET | `/subscriptions/upcoming` | Billing alerts (with days param) |
| GET | `/subscriptions/expired` | Expired subscriptions list |

## Endpoint Parameters

### GET /subscriptions/stats
- `period`: `monthly` or `yearly`, default monthly
- `base_currency`: Base currency, default CNY, options: USD, EUR, etc.
- **Note**: Only counts active subscriptions that haven't expired
- **Multi-currency**: Auto-fetches exchange rates; falls back to currency grouping if rates unavailable

### GET /subscriptions/upcoming
- `days`: Alert days, default 7, range 1-365

### GET /subscriptions/expired
- No parameters
- Returns subscriptions with Active status but past due date
- Sorted by due date descending (most expired first)

## Request Examples

Add subscription:
```json
POST /subscriptions/
{
  "name": "ChatGPT Plus",
  "price": 20.0,
  "currency": "USD",
  "billing_cycle": "Monthly",
  "ending_date": "2024-12-31",
  "category": "Productivity",
  "auto_renew": true
}
```

Update subscription:
```json
PUT /subscriptions/1
{
  "price": 25.0,
  "ending_date": "2025-01-31"
}
```

Stats response:
```json
{
  "total_cost": 1650.0,
  "base_currency": "CNY",
  "period": "yearly",
  "active_count": 5,
  "breakdown_by_category": {"Productivity": 1440.0, "Entertainment": 210.0},
  "breakdown_by_currency": {"USD": 240.0, "CNY": 100.0},
  "rates_used": {"USD": 7.25, "CNY": 1.0},
  "rates_available": true
}
```

**Note**: When `rates_available=false`, exchange rates couldn't be fetched. `total_cost` only counts base-currency subscriptions; refer to `breakdown_by_currency` for full picture.
```

## Billing Cycle

| Value | Monthly Calculation | Yearly Calculation |
|-------|---------------------|---------------------|
| Monthly | Price | Price × 12 |
| Yearly | Price ÷ 12 | Price |
| Weekly | Price × 4.33 | Price × 52 |

## Data Model

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| name | string | ✓ | Subscription name |
| price | float | ✓ | Price (positive number) |
| currency | string | default CNY | Currency code |
| billing_cycle | enum | default Monthly | Monthly/Yearly/Weekly |
| ending_date | date | ✓ | Due date YYYY-MM-DD |
| category | string | default Other | Category label |
| status | enum | default Active | Active/Canceled |
| auto_renew | bool | default false | Auto-renewal enabled |

## Auto-Renewal

When `auto_renew=true`, subscription extends by one billing cycle upon expiry:
- Monthly → +30 days
- Yearly → +365 days
- Weekly → +7 days

Auto-renewal task runs daily at 00:05.

## Migrate from Wallos

When user requests migration from Wallos:

### Prerequisites

Get from user:
- Wallos service URL (e.g., `http://your-wallos-instance.com`)
- Wallos API Token (from Wallos settings)

### Migration Steps

1. Call Wallos API to get subscription list:
   ```
   GET {wallos_url}/api/subscriptions/get_subscriptions.php
   Header: Authorization: Bearer {wallos_token}
   ```

2. Iterate through `subscriptions` array, map each entry:

### Field Mapping

| Wallos Field | Sub Manager Field | Mapping |
|--------------|-------------------|---------|
| `name` | `name` | Direct |
| `price` | `price` | Direct |
| `currency_id` | `currency` | Query Wallos currency table or use default |
| `next_payment` | `ending_date` | Direct (YYYY-MM-DD) |
| `category_name` | `category` | Direct, use "Other" if empty |
| `inactive` | `status` | `inactive=1` → Canceled, else Active |
| `auto_renew` | `auto_renew` | Direct (1/0 → true/false) |
| `cycle` + `frequency` | `billing_cycle` | Combined inference |

### Billing Cycle Inference

Wallos uses `cycle` + `frequency` combination:

| cycle | frequency | billing_cycle |
|-------|-----------|---------------|
| "Monthly" | 1 | Monthly |
| "Yearly" | 1 | Yearly |
| "Weekly" | 1 | Weekly |
| "Daily" | 30 | Monthly (30 days) |
| Number (days) | 1 | Monthly (if ≤30) |
| Number (days) | 1 | Yearly (if ≥365) |

**Simplified rules**:
- If cycle is string, use directly
- If cycle is number (days):
  - 1-30 → Monthly
  - 31-364 → Monthly (prorated by actual days)
  - ≥365 → Yearly

### Migration Request Example

For each Wallos subscription, construct POST request:
```json
POST /subscriptions/
{
  "name": "Netflix",
  "price": 15.99,
  "currency": "USD",
  "billing_cycle": "Monthly",
  "ending_date": "2024-12-15",
  "category": "Entertainment",
  "status": "Active",
  "auto_renew": true
}
```

### Migration Complete

Report to user after migration:
- Total subscriptions migrated
- Success/failure count
- Skipped canceled subscriptions (if any)

## Scheduled Reminder Task

User can request Agent to set up scheduled reminders:

**User request**:
```
Every day at 9 AM check subscriptions due in next 7 days and remind me if any.
```

**Agent actions**:
1. Set up scheduled task (using system cron or Agent's scheduling)
2. Daily at 9:00:
   - Call `GET /subscriptions/upcoming?days=7`
   - If list not empty, send reminder to user
3. Notify user task is set, reminder method (desktop notification, email, etc.)

**Reminder message example**:
```
⏰ Subscription Alert: 3 subscriptions due in next 7 days:
1. Netflix - $15.99 - 2024-12-15
2. ChatGPT Plus - $20.00 - 2024-12-18
3. GitHub Copilot - $10.00 - 2024-12-20
Total: $45.99
```

## Common Scenarios

| User Intent | Agent Action |
|-------------|--------------|
| "Add new subscription" | POST /subscriptions/ |
| "Subscription price increased" | PUT /subscriptions/{id} update price |
| "Renewed, update due date" | PUT /subscriptions/{id} update ending_date |
| "How much this month" | GET /subscriptions/stats |
| "How much this year" | GET /subscriptions/stats?period=yearly |
| "What's due next month" | GET /subscriptions/upcoming?days=30 |
| "What subscriptions expired" | GET /subscriptions/expired |
| "Cancel subscription" | PUT /subscriptions/{id}/cancel |
| "Restore subscription" | PUT /subscriptions/{id}/restore |
| "Delete subscription" | DELETE /subscriptions/{id} |
| "Migrate from Wallos" | Follow migration flow |
| "Set expiry reminder" | Set scheduled task calling upcoming endpoint |

## Handling Expired Subscriptions

When user queries expired subscriptions (GET /subscriptions/expired), ask user to decide:

| User Intent | Agent Action |
|-------------|--------------|
| "Still need this subscription" | PUT /subscriptions/{id} update ending_date or set auto_renew=true |
| "No longer need" | PUT /subscriptions/{id}/cancel or DELETE /subscriptions/{id} |
| "Batch process expired" | Iterate expired list, ask user for each |

## Error Codes

| Status Code | Action |
|-------------|--------|
| 401 | Token invalid, prompt user to update config |
| 404 | Subscription not found |
| 400 | Invalid request data or state conflict |