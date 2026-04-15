"""
Token 鉴权模块
实现基于静态 Token 的 API 鉴权机制
"""
import os
from fastapi import HTTPException, Security, status
from fastapi.security import APIKeyHeader

# 从环境变量获取 API Token，默认为空（生产环境必须设置）
API_TOKEN = os.getenv("API_TOKEN", "")

# 定义 Token Header 名称
api_key_header = APIKeyHeader(name="X-API-Token", auto_error=False)


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

    使用示例:
        @router.get("/protected")
        async def protected_route(token: str = Depends(verify_token)):
            return {"message": "Access granted"}
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

    if api_key != API_TOKEN:
        raise HTTPException(
            status_code=status.HTTP_401_UNAUTHORIZED,
            detail="Token 无效或已过期，请检查 X-API-Token 是否正确"
        )

    return api_key