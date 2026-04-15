"""
数据模型定义模块
定义订阅服务的核心数据模型
"""
from datetime import date
from enum import Enum
from typing import Optional
from sqlmodel import SQLModel, Field


class BillingCycle(str, Enum):
    """
    计费周期枚举
    - Monthly: 按月计费
    - Yearly: 按年计费
    - Weekly: 按周计费
    """
    MONTHLY = "Monthly"
    YEARLY = "Yearly"
    WEEKLY = "Weekly"


class SubscriptionStatus(str, Enum):
    """
    订阅状态枚举
    - Active: 活跃订阅，正在计费中
    - Canceled: 已取消订阅，不再计费
    """
    ACTIVE = "Active"
    CANCELED = "Canceled"


class SubscriptionBase(SQLModel):
    """
    订阅基础模型，包含所有可读写字段
    """
    name: str = Field(
        ...,
        description="订阅服务名称，如 Netflix、ChatGPT、GitHub Copilot 等"
    )
    price: float = Field(
        ...,
        gt=0,
        description="订阅价格，必须为正数"
    )
    currency: str = Field(
        default="CNY",
        description="货币类型，默认为人民币 CNY，支持 ISO 4217 货币代码"
    )
    billing_cycle: BillingCycle = Field(
        default=BillingCycle.MONTHLY,
        description="计费周期：Monthly(按月)、Yearly(按年)、Weekly(按周)"
    )
    ending_date: date = Field(
        ...,
        description="到期日/下次扣费日期，格式为 YYYY-MM-DD"
    )
    category: str = Field(
        default="其他",
        description="订阅分类，如：生产力、娱乐、服务器、教育、其他等"
    )
    status: SubscriptionStatus = Field(
        default=SubscriptionStatus.ACTIVE,
        description="订阅状态：Active(活跃) 或 Canceled(已取消)"
    )
    auto_renew: bool = Field(
        default=False,
        description="是否自动续期。True 表示到期后自动延长一个计费周期，False 表示不自动续期"
    )


class Subscription(SubscriptionBase, table=True):
    """
    订阅数据表模型
    继承自 SubscriptionBase，添加主键 id 字段
    """
    id: Optional[int] = Field(default=None, primary_key=True)


class SubscriptionCreate(SubscriptionBase):
    """
    创建订阅时的请求数据模型
    继承所有字段，用于 POST 请求体
    """
    pass


class SubscriptionRead(SubscriptionBase):
    """
    读取订阅时的响应数据模型
    包含所有基础字段及主键 id
    """
    id: int


class SubscriptionUpdate(SQLModel):
    """
    更新订阅时的请求数据模型
    所有字段均为可选，支持部分更新
    """
    name: Optional[str] = None
    price: Optional[float] = None
    currency: Optional[str] = None
    billing_cycle: Optional[BillingCycle] = None
    ending_date: Optional[date] = None
    category: Optional[str] = None
    status: Optional[SubscriptionStatus] = None
    auto_renew: Optional[bool] = None


class StatsResponse(SQLModel):
    """
    费用统计响应模型（支持月度和年度）
    """
    total_cost: float = Field(
        ...,
        description="总花费（月均或年度，取决于 period 参数）"
    )
    currency: str = Field(
        default="CNY",
        description="统计使用的货币单位"
    )
    active_count: int = Field(
        ...,
        description="当前活跃订阅数量"
    )
    period: str = Field(
        ...,
        description="统计周期：monthly 或 yearly"
    )
    breakdown: dict = Field(
        default_factory=dict,
        description="按分类的费用明细，key 为分类名称，value 为该分类的花费"
    )