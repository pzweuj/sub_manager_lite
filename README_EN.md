# Sub Manager Lite

A subscription management API service for tracking subscriptions, calculating expenses, and billing alerts.

## Quick Start

```bash
# Modify API_TOKEN in docker-compose.yaml
docker-compose up -d
```

Service URL: http://localhost:8000

**[中文文档](./README.md)**

## Agent Guide

See [SKILL.md](./SKILL.md)

## First-time Setup

On first use, Agent will ask for service URL and Token, then save to `~/.sub_manager_lite_setting.json`. Subsequent calls don't need repeated input.

## Example Prompts

### 1. Initial Setup

```
Configure Sub Manager Lite:
URL: http://localhost:8000
Token: your-token-here
```

### 2. Add Subscription

```
Add a subscription: Netflix, $15.99/month, next payment 2024-12-15, category: Entertainment, enable auto-renew.
```

### 3. Check Expenses

```
Show my total subscription cost this year.
```

### 4. Set Reminder

```
Every day at 9 AM, check subscriptions due in the next 7 days and remind me if any.
```

### 5. Migrate from Wallos

```
Migrate my Wallos subscription data.
Wallos URL: http://your-wallos.com
Wallos Token: wallos-token
```

### 6. Update Subscription

```
Netflix price increased, update to $18.99, new due date 2025-01-15.
```

### 7. Cancel Subscription

```
Cancel my Netflix subscription.
```

### 8. Update Config

```
Sub Manager URL changed to http://192.168.1.100:8000, update my config.
```