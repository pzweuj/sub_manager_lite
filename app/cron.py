"""
自动续期任务模块
检测到期订阅并自动延长计费周期
"""
from datetime import date, timedelta
from logging import getLogger

from sqlmodel import Session, select

from app.database import engine
from app.models import BillingCycle, Subscription, SubscriptionStatus

logger = getLogger(__name__)


def process_auto_renew():
    """
    处理自动续期任务。

    检查所有设置了 auto_renew=True 且已到期的活跃订阅，
    自动将到期日延长一个计费周期。
    """
    today = date.today()

    with Session(engine) as session:
        # 查询需要续期的订阅
        statement = select(Subscription).where(
            Subscription.status == SubscriptionStatus.ACTIVE,
            Subscription.auto_renew == True,
            Subscription.ending_date <= today
        )
        subscriptions = session.exec(statement).all()

        if not subscriptions:
            logger.info("没有需要自动续期的订阅")
            return

        for sub in subscriptions:
            # 根据计费周期计算新的到期日
            if sub.billing_cycle == BillingCycle.MONTHLY:
                new_date = sub.ending_date + timedelta(days=30)
            elif sub.billing_cycle == BillingCycle.YEARLY:
                new_date = sub.ending_date + timedelta(days=365)
            elif sub.billing_cycle == BillingCycle.WEEKLY:
                new_date = sub.ending_date + timedelta(days=7)
            else:
                new_date = sub.ending_date + timedelta(days=30)

            sub.ending_date = new_date
            session.add(sub)
            logger.info(f"订阅 {sub.name} (ID: {sub.id}) 已自动续期至 {new_date}")

        session.commit()
        logger.info(f"已完成 {len(subscriptions)} 个订阅的自动续期")