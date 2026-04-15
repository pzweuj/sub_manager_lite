"""
Sub Manager Lite - 订阅管理服务主入口
基于 FastAPI 构建的本地订阅管理服务
"""
from contextlib import asynccontextmanager
from logging import getLogger

from apscheduler.schedulers.asyncio import AsyncIOScheduler
from fastapi import FastAPI
from fastapi.openapi.utils import get_openapi

from app.cron import process_auto_renew
from app.database import init_db
from app.routers import subscriptions

logger = getLogger(__name__)

# 定时任务调度器
scheduler = AsyncIOScheduler()


@asynccontextmanager
async def lifespan(app: FastAPI):
    """
    应用生命周期管理。
    启动时初始化数据库和定时任务，关闭时清理资源。
    """
    # 启动时：初始化数据库
    init_db()

    # 启动定时任务：每天 00:05 执行自动续期检查
    scheduler.add_job(
        process_auto_renew,
        trigger="cron",
        hour=0,
        minute=5,
        id="auto_renew_job",
        replace_existing=True
    )
    scheduler.start()
    logger.info("定时任务调度器已启动，自动续期任务将在每天 00:05 执行")

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