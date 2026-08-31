"""
Token 鉴权模块
实现基于静态 Token 的 API 鉴权机制
"""
import os
import secrets

from fastapi import HTTPException, Security, status
from fastapi.security import APIKeyHeader

# 从环境变量获取 API Token，默认为空（生产环境必须设置）
API_TOKEN = os.getenv("API_TOKEN", "")

# Token 最低强度要求
MIN_TOKEN_LENGTH = 16
PLACEHOLDER_TOKENS = {
    "your-secret-token-here",
    "your-token-here",
    "changeme",
    "change-me",
    "token",
    "secret",
}

# 定义 Token Header 名称
api_key_header = APIKeyHeader(name="X-API-Token", auto_error=False)


def validate_api_token() -> None:
    """
    启动时校验 API Token 强度。

    为空、过短或使用占位符时抛出 RuntimeError，阻止服务以弱 Token 上线。
    """
    if not API_TOKEN:
        raise RuntimeError("API_TOKEN 环境变量未配置，请设置一个强随机 Token 后重启")
    if len(API_TOKEN) < MIN_TOKEN_LENGTH:
        raise RuntimeError(f"API_TOKEN 长度必须至少 {MIN_TOKEN_LENGTH} 位")
    if API_TOKEN.lower() in PLACEHOLDER_TOKENS:
        raise RuntimeError("API_TOKEN 不能使用占位符，请改为强随机值")


async def verify_token(api_key: str = Security(api_key_header)) -> str:
    """
    验证 API Token 的依赖注入函数。

    该函数作为 FastAPI 依赖使用，验证请求头中的 X-API-Token 是否有效。

    参数:
        api_key: 从请求头 X-API-Token 中提取的 Token 值

    返回:
        验证通过后返回 Token 字符串

    异常:
        HTTPException: 当 Token 未提供或无效时抛出 401 错误
    """
    if not API_TOKEN:
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="API_TOKEN 环境变量未配置，请联系管理员"
        )

    if not api_key:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="未提供认证 Token，请在请求头中添加 X-API-Token"
        )

    if not secrets.compare_digest(api_key, API_TOKEN):
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Token 无效或已过期，请检查 X-API-Token 是否正确"
        )

    return api_key
