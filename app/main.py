"""
Sub Manager Lite - 订阅管理服务主入口
基于 FastAPI 构建的本地订阅管理服务
"""
import os
from contextlib import asynccontextmanager
from logging import getLogger

from apscheduler.schedulers.asyncio import AsyncIOScheduler
from fastapi import FastAPI
from fastapi.openapi.utils import get_openapi

from app.auth import validate_api_token
from app.cron import process_due_subscriptions
from app.database import init_db
from app.routers import subscriptions
from app.security import RateLimitMiddleware

logger = getLogger(__name__)

# 定时任务调度器
scheduler = AsyncIOScheduler()

# 到期订阅自检间隔（分钟），可用环境变量覆盖
SELF_CHECK_INTERVAL_MINUTES = int(os.getenv("SELF_CHECK_INTERVAL_MINUTES", "60"))


@asynccontextmanager
async def lifespan(app: FastAPI):
    """
    应用生命周期管理。
    启动时初始化数据库和定时任务，关闭时清理资源。
    """
    # 启动时：校验 Token 强度并初始化数据库
    validate_api_token()
    init_db()

    # 启动自检：立即校正一次到期订阅（停机重启后无需等待定时点）
    process_due_subscriptions()

    # 定期自检：默认每小时执行一次到期订阅处理
    scheduler.add_job(
        process_due_subscriptions,
        trigger="interval",
        minutes=SELF_CHECK_INTERVAL_MINUTES,
        id="due_subscriptions_job",
        replace_existing=True
    )
    scheduler.start()
    logger.info(f"定时任务调度器已启动，到期订阅自检每 {SELF_CHECK_INTERVAL_MINUTES} 分钟执行一次")

    yield

    # 关闭时：停止定时任务
    scheduler.shutdown()
    logger.info("定时任务调度器已关闭")


# 创建 FastAPI 应用实例
app = FastAPI(
    title="Sub Manager Lite",
    description="""
## 订阅管理服务 API

一个轻量级的本地订阅管理服务，帮助您：
- 📝 记录各种订阅服务（Netflix、ChatGPT、服务器等）
- 💰 计算月均花费，了解订阅开销
- ⏰ 账单预警，提前知晓即将扣费的订阅
- 🔄 自动续期，到期自动延长订阅周期
- 🗑️ 到期自动取消，未开启续期的订阅到期后自动转为取消状态

### 鉴权说明
除 API 文档页面外，所有接口均需要 Token 鉴权。
请在请求头中添加 `X-API-Token` 字段。

### 使用方式
1. 通过 Docker 快速部署服务
2. 设置 `API_TOKEN` 环境变量进行鉴权
3. 使用 OpenAPI 文档了解接口详情
""",
    version="1.0.0",
    lifespan=lifespan,
    docs_url="/docs",
    redoc_url="/redoc",
)

# 注册限流中间件（防 Token 爆破）
app.add_middleware(RateLimitMiddleware)

# 注册路由
app.include_router(subscriptions.router)


def custom_openapi():
    """
    自定义 OpenAPI Schema，增强文档描述。
    """
    if app.openapi_schema:
        return app.openapi_schema

    openapi_schema = get_openapi(
        title=app.title,
        version=app.version,
        description=app.description,
        routes=app.routes,
        tags=[
            {
                "name": "订阅管理",
                "description": "订阅服务的增删改查和统计接口"
            }
        ]
    )

    # 添加全局安全方案描述
    openapi_schema["components"]["securitySchemes"] = {
        "ApiKeyAuth": {
            "type": "apiKey",
            "in": "header",
            "name": "X-API-Token",
            "description": "API Token 鉴权，需要在请求头中添加 X-API-Token"
        }
    }
    openapi_schema["security"] = [{"ApiKeyAuth": []}]

    app.openapi_schema = openapi_schema
    return app.openapi_schema


app.openapi = custom_openapi


@app.get(
    "/",
    summary="服务健康检查",
    description="检查服务是否正常运行，无需鉴权",
    tags=["系统"]
)
async def root():
    """
    根路径健康检查接口。
    无需 Token 鉴权，用于确认服务运行状态。
    """
    return {
        "service": "Sub Manager Lite",
        "status": "running",
        "version": "1.0.0",
        "docs": "/docs"
    }