"""
订阅管理路由模块
实现订阅服务的 CRUD 接口和统计接口
"""
from datetime import date, timedelta
from typing import Optional
from fastapi import APIRouter, Depends, HTTPException, Query, status
from sqlmodel import Session, col, select

from app.auth import verify_token
from app.database import get_session
from app.models import (
    BillingCycle,
    StatsResponse,
    Subscription,
    SubscriptionCreate,
    SubscriptionRead,
    SubscriptionStatus,
    SubscriptionUpdate,
)

router = APIRouter(
    prefix="/subscriptions",
    tags=["订阅管理"],
    dependencies=[Depends(verify_token)]
)


@router.post(
    "/",
    response_model=SubscriptionRead,
    status_code=status.HTTP_201_CREATED,
    summary="新增订阅记录",
    description="""
新增一条订阅服务记录。

## 功能说明
- 记录一个新的订阅服务信息，包括名称、价格、计费周期、到期日等
- 创建后订阅状态默认为 Active（活跃），自动续期默认为 False
- 系统自动生成唯一的主键 ID

## 请求体字段
- `name`: 订阅服务名称（必填）
- `price`: 订阅价格（必填，必须为正数）
- `currency`: 货币类型（默认 CNY）
- `billing_cycle`: 计费周期 Monthly/Yearly/Weekly（默认 Monthly）
- `ending_date`: 到期日/下次扣费日（必填，格式 YYYY-MM-DD）
- `category`: 分类标签（默认"其他"）
- `status`: 订阅状态（默认 Active）
- `auto_renew`: 是否自动续期（默认 False）

## 返回值
返回创建成功的订阅记录，包含系统生成的 ID。
""",
    responses={
        201: {"description": "订阅创建成功"},
        400: {"description": "请求数据无效"},
        401: {"description": "Token 无效或未提供"},
    }
)
async def create_subscription(
    subscription: SubscriptionCreate,
    session: Session = Depends(get_session)
) -> SubscriptionRead:
    db_subscription = Subscription.from_orm(subscription)
    session.add(db_subscription)
    session.commit()
    session.refresh(db_subscription)
    return db_subscription


@router.put(
    "/{sub_id}",
    response_model=SubscriptionRead,
    summary="更新订阅信息",
    description="""
更新指定订阅的信息。

## 功能说明
- 支持部分更新，只需提供要修改的字段
- 可更新：名称、价格、货币、计费周期、到期日、分类、状态、自动续期

## 参数
- `sub_id`: 订阅记录的唯一标识 ID

## 请求体
所有字段均为可选，提供即更新，未提供保持原值。
""",
    responses={
        200: {"description": "更新成功"},
        404: {"description": "订阅不存在"},
        401: {"description": "Token 无效或未提供"},
    }
)
async def update_subscription(
    sub_id: int,
    update_data: SubscriptionUpdate,
    session: Session = Depends(get_session)
) -> SubscriptionRead:
    subscription = session.get(Subscription, sub_id)
    if not subscription:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"订阅 ID {sub_id} 不存在"
        )

    update_dict = update_data.dict(exclude_unset=True)
    for key, value in update_dict.items():
        setattr(subscription, key, value)

    session.add(subscription)
    session.commit()
    session.refresh(subscription)
    return subscription


@router.put(
    "/{sub_id}/cancel",
    response_model=SubscriptionRead,
    summary="取消订阅",
    description="""
快捷取消订阅服务。

## 功能说明
- 将指定 ID 的订阅状态从 Active 更改为 Canceled
- 取消后的订阅将不再计入费用统计和账单预警
- 同时会关闭自动续期

## 参数
- `sub_id`: 订阅记录的唯一标识 ID
""",
    responses={
        200: {"description": "订阅取消成功"},
        400: {"description": "订阅已处于取消状态"},
        404: {"description": "订阅记录不存在"},
        401: {"description": "Token 无效或未提供"},
    }
)
async def cancel_subscription(
    sub_id: int,
    session: Session = Depends(get_session)
) -> SubscriptionRead:
    subscription = session.get(Subscription, sub_id)
    if not subscription:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"订阅 ID {sub_id} 不存在"
        )

    if subscription.status == SubscriptionStatus.CANCELED:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="该订阅已处于取消状态"
        )

    subscription.status = SubscriptionStatus.CANCELED
    subscription.auto_renew = False
    session.add(subscription)
    session.commit()
    session.refresh(subscription)
    return subscription


@router.put(
    "/{sub_id}/restore",
    response_model=SubscriptionRead,
    summary="恢复订阅",
    description="""
恢复已取消的订阅。

## 功能说明
- 将指定 ID 的订阅状态从 Canceled 更改为 Active
- 恢复后的订阅将重新计入费用统计和账单预警

## 参数
- `sub_id`: 订阅记录的唯一标识 ID
""",
    responses={
        200: {"description": "订阅恢复成功"},
        400: {"description": "订阅已处于活跃状态"},
        404: {"description": "订阅记录不存在"},
        401: {"description": "Token 无效或未提供"},
    }
)
async def restore_subscription(
    sub_id: int,
    session: Session = Depends(get_session)
) -> SubscriptionRead:
    subscription = session.get(Subscription, sub_id)
    if not subscription:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"订阅 ID {sub_id} 不存在"
        )

    if subscription.status == SubscriptionStatus.ACTIVE:
        raise HTTPException(
            status_code=status.HTTP_400_BAD_REQUEST,
            detail="该订阅已处于活跃状态"
        )

    subscription.status = SubscriptionStatus.ACTIVE
    session.add(subscription)
    session.commit()
    session.refresh(subscription)
    return subscription


@router.delete(
    "/{sub_id}",
    status_code=status.HTTP_204_NO_CONTENT,
    summary="删除订阅",
    description="""
删除订阅记录。

## 功能说明
- 永久删除指定 ID 的订阅记录
- 删除后无法恢复，请谨慎操作

## 参数
- `sub_id`: 订阅记录的唯一标识 ID
""",
    responses={
        204: {"description": "删除成功"},
        404: {"description": "订阅不存在"},
        401: {"description": "Token 无效或未提供"},
    }
)
async def delete_subscription(
    sub_id: int,
    session: Session = Depends(get_session)
) -> None:
    subscription = session.get(Subscription, sub_id)
    if not subscription:
        raise HTTPException(
            status_code=status.HTTP_404_NOT_FOUND,
            detail=f"订阅 ID {sub_id} 不存在"
        )

    session.delete(subscription)
    session.commit()


@router.get(
    "/",
    response_model=list[SubscriptionRead],
    summary="查询订阅列表",
    description="""
查询订阅记录列表，支持模糊搜索和状态过滤。

## 查询参数
- `name`: 订阅名称搜索关键字（可选，模糊匹配）
- `status`: 订阅状态过滤（可选，Active 或 Canceled）

## 返回值
返回符合条件（如有）的订阅记录列表，按到期日升序排列。
""",
    responses={
        200: {"description": "查询成功"},
        401: {"description": "Token 无效或未提供"},
    }
)
async def list_subscriptions(
    name: Optional[str] = Query(
        None,
        description="订阅名称搜索关键字，支持模糊匹配"
    ),
    status_filter: Optional[SubscriptionStatus] = Query(
        None,
        alias="status",
        description="订阅状态过滤：Active 或 Canceled"
    ),
    session: Session = Depends(get_session)
) -> list[SubscriptionRead]:
    statement = select(Subscription)

    if name:
        statement = statement.where(col(Subscription.name).ilike(f"%{name}%"))

    if status_filter:
        statement = statement.where(Subscription.status == status_filter)

    statement = statement.order_by(Subscription.ending_date)
    subscriptions = session.exec(statement).all()
    return subscriptions


@router.get(
    "/stats",
    response_model=StatsResponse,
    summary="费用统计",
    description="""
计算当前所有活跃订阅的费用统计。

## 功能说明
- 仅统计状态为 Active 的订阅
- 支持月度或年度统计（通过 period 参数）
- 按分类展示费用明细

## 计费周期折算
- Monthly: 直接计入
- Yearly: 价格除以 12（月度）或直接计入（年度）
- Weekly: 价格乘以 4.33（月度）或乘以 52（年度）

## 查询参数
- `period`: 统计周期，monthly（月度）或 yearly（年度），默认 monthly
""",
    responses={
        200: {"description": "统计成功"},
        401: {"description": "Token 无效或未提供"},
    }
)
async def get_stats(
    period: str = Query(
        "monthly",
        description="统计周期：monthly（月度）或 yearly（年度）"
    ),
    session: Session = Depends(get_session)
) -> StatsResponse:
    statement = select(Subscription).where(
        Subscription.status == SubscriptionStatus.ACTIVE
    )
    subscriptions = session.exec(statement).all()

    total_cost = 0.0
    breakdown: dict[str, float] = {}

    for sub in subscriptions:
        if period == "yearly":
            if sub.billing_cycle == BillingCycle.MONTHLY:
                cost = sub.price * 12
            elif sub.billing_cycle == BillingCycle.YEARLY:
                cost = sub.price
            elif sub.billing_cycle == BillingCycle.WEEKLY:
                cost = sub.price * 52
            else:
                cost = sub.price
        else:  # monthly
            if sub.billing_cycle == BillingCycle.MONTHLY:
                cost = sub.price
            elif sub.billing_cycle == BillingCycle.YEARLY:
                cost = sub.price / 12
            elif sub.billing_cycle == BillingCycle.WEEKLY:
                cost = sub.price * 4.33
            else:
                cost = sub.price

        total_cost += cost
        category = sub.category or "其他"
        breakdown[category] = breakdown.get(category, 0) + cost

    return StatsResponse(
        total_cost=round(total_cost, 2),
        currency="CNY",
        active_count=len(subscriptions),
        period=period,
        breakdown=breakdown
    )


@router.get(
    "/upcoming",
    response_model=list[SubscriptionRead],
    summary="账单预警",
    description="""
查询即将扣费的活跃订阅列表。

## 功能说明
- 仅返回状态为 Active 的订阅
- 默认查询未来 7 天内，可通过 days 参数调整
- 按到期日升序排列

## 查询参数
- `days`: 预警天数，默认 7 天
""",
    responses={
        200: {"description": "查询成功"},
        401: {"description": "Token 无效或未提供"},
    }
)
async def get_upcoming_bills(
    days: int = Query(
        7,
        ge=1,
        le=365,
        description="预警天数，查询未来 N 天内即将扣费的订阅，默认 7 天"
    ),
    session: Session = Depends(get_session)
) -> list[SubscriptionRead]:
    today = date.today()
    end_date = today + timedelta(days=days)

    statement = select(Subscription).where(
        Subscription.status == SubscriptionStatus.ACTIVE,
        Subscription.ending_date >= today,
        Subscription.ending_date <= end_date
    ).order_by(Subscription.ending_date)

    subscriptions = session.exec(statement).all()
    return subscriptions