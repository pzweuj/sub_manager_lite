"""
汇率服务模块
使用 Frankfurter API 获取实时汇率
"""
import os
from logging import getLogger
from typing import Optional

import httpx

logger = getLogger(__name__)

# Frankfurter API 基础地址
FRANKFURTER_API = "https://api.frankfurter.dev/v2"

# 默认基准货币
DEFAULT_BASE_CURRENCY = os.getenv("BASE_CURRENCY", "CNY")

# 汇率缓存（简单内存缓存，应用启动后首次获取）
_rate_cache: dict[str, float] = {}
_cache_base_currency: Optional[str] = None


async def get_exchange_rate(from_currency: str, to_currency: str) -> Optional[float]:
    """
    获取汇率。

    Args:
        from_currency: 源货币代码（如 USD）
        to_currency: 目标货币代码（如 CNY）

    Returns:
        汇率值，如果获取失败返回 None
    """
    if from_currency == to_currency:
        return 1.0

    # 检查缓存
    cache_key = f"{from_currency}_{to_currency}"
    if cache_key in _rate_cache:
        return _rate_cache[cache_key]

    try:
        async with httpx.AsyncClient(timeout=10.0) as client:
            response = await client.get(
                f"{FRANKFURTER_API}/rates",
                params={"base": from_currency, "quotes": to_currency}
            )

            if response.status_code == 200:
                data = response.json()
                rate = data.get("quotes", {}).get(to_currency)
                if rate:
                    # 缓存汇率
                    _rate_cache[cache_key] = rate
                    logger.info(f"获取汇率 {from_currency}/{to_currency} = {rate}")
                    return rate
            else:
                logger.warning(f"获取汇率失败: HTTP {response.status_code}")
                return None

    except Exception as e:
        logger.error(f"获取汇率异常: {e}")
        return None


async def fetch_all_rates_to_base(
    currencies: list[str],
    base_currency: str = DEFAULT_BASE_CURRENCY
) -> dict[str, Optional[float]]:
    """
    批量获取多种货币到基准货币的汇率。

    Args:
        currencies: 需要查询的货币列表
        base_currency: 基准货币

    Returns:
        货币到基准货币的汇率字典，失败的货币值为 None
    """
    if not currencies:
        return {}

    # 过滤掉基准货币本身
    need_fetch = [c for c in currencies if c != base_currency]
    if not need_fetch:
        return {base_currency: 1.0}

    rates: dict[str, Optional[float]] = {base_currency: 1.0}

    try:
        async with httpx.AsyncClient(timeout=15.0) as client:
            response = await client.get(
                f"{FRANKFURTER_API}/rates",
                params={"base": "EUR", "quotes": ",".join(need_fetch + [base_currency])}
            )

            if response.status_code == 200:
                data = response.json()
                quotes = data.get("quotes", {})
                base_rate = quotes.get(base_currency)

                if base_rate:
                    for currency in need_fetch:
                        currency_rate = quotes.get(currency)
                        if currency_rate:
                            # 计算 currency -> base_currency 的汇率
                            # EUR -> currency = currency_rate
                            # EUR -> base = base_rate
                            # currency -> base = base_rate / currency_rate
                            rates[currency] = base_rate / currency_rate
                            _rate_cache[f"{currency}_{base_currency}"] = rates[currency]
                        else:
                            rates[currency] = None
                            logger.warning(f"未找到货币 {currency} 的汇率")

            else:
                logger.warning(f"批量获取汇率失败: HTTP {response.status_code}")
                for currency in need_fetch:
                    rates[currency] = None

    except Exception as e:
        logger.error(f"批量获取汇率异常: {e}")
        for currency in need_fetch:
            rates[currency] = None

    return rates


def clear_cache():
    """清除汇率缓存。"""
    global _rate_cache, _cache_base_currency
    _rate_cache.clear()
    _cache_base_currency = None
    logger.info("汇率缓存已清除")