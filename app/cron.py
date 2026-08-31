"""
自动续期与到期处理任务模块
检测到期订阅并自动续期或自动取消
"""
from datetime import date, timedelta
from logging import getLogger

from dateutil.relativedelta import relativedelta
from sqlmodel import Session, select

from app.database import engine
from app.models import BillingCycle, Subscription, SubscriptionStatus

logger = getLogger(__name__)

# 续期追赶的最大迭代次数，防止异常数据导致死循环
MAX_CATCHUP_ITERATIONS = 10000


def compute_next_due(ending_date: date, billing_cycle: BillingCycle, interval: int) -> date:
    """根据计费周期和间隔计算下一个到期日。"""
    interval = interval or 1
    if billing_cycle == BillingCycle.MONTHLY:
        return ending_date + relativedelta(months=interval)
    if billing_cycle == BillingCycle.YEARLY:
        return ending_date + relativedelta(years=interval)
    if billing_cycle == BillingCycle.WEEKLY:
        return ending_date + timedelta(weeks=interval)
    return ending_date + relativedelta(months=interval)


def process_due_subscriptions():
    """
    处理到期订阅任务。

    - 自动取消：auto_renew=False 且已过期的活跃订阅，置为 Canceled。
    - 自动续期：auto_renew=True 且已到期的活跃订阅，从到期日起连续
      累加计费周期直到未来日期，一次性补齐停机期间错过的所有周期。
    """
    today = date.today()

    with Session(engine) as session:
        # 自动取消：未设自动续期且已过期的活跃订阅
        cancel_statement = select(Subscription).where(
            Subscription.status == SubscriptionStatus.ACTIVE,
            Subscription.auto_renew == False,
            Subscription.ending_date < today
        )
        to_cancel = session.exec(cancel_statement).all()

        for sub in to_cancel:
            sub.status = SubscriptionStatus.CANCELED
            session.add(sub)
            logger.info(f"订阅 {sub.name} (ID: {sub.id}) 已到期自动取消")

        # 自动续期：设置了自动续期且已到期的活跃订阅
        renew_statement = select(Subscription).where(
            Subscription.status == SubscriptionStatus.ACTIVE,
            Subscription.auto_renew == True,
            Subscription.ending_date <= today
        )
        to_renew = session.exec(renew_statement).all()

        for sub in to_renew:
            new_date = sub.ending_date
            for _ in range(MAX_CATCHUP_ITERATIONS):
                if new_date > today:
                    break
                new_date = compute_next_due(new_date, sub.billing_cycle, sub.billing_interval)
            else:
                logger.error(f"订阅 {sub.name} (ID: {sub.id}) 续期追赶超过上限，已跳过")
                continue

            sub.ending_date = new_date
            session.add(sub)
            logger.info(f"订阅 {sub.name} (ID: {sub.id}) 已自动续期至 {new_date}")

        session.commit()

        if not to_cancel and not to_renew:
            logger.info("没有需要处理的到期订阅")
            return

        logger.info(f"已处理到期订阅：自动取消 {len(to_cancel)} 个，自动续期 {len(to_renew)} 个")
