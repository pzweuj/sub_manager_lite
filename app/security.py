"""
轻量级内存限流中间件，用于防止 API Token 被暴力破解。

基于进程内字典实现，仅在单进程部署下有效（当前 Docker 默认单 worker）。
"""
import time
from collections import defaultdict, deque
from typing import Callable

from starlette.middleware.base import BaseHTTPMiddleware
from starlette.requests import Request
from starlette.responses import JSONResponse, Response

DEFAULT_RATE_LIMIT = 120        # 每个时间窗口内允许的最大请求数
DEFAULT_RATE_WINDOW = 60        # 普通请求限流窗口（秒）
DEFAULT_AUTH_FAIL_LIMIT = 10    # 时间窗口内允许的最大鉴权失败次数
DEFAULT_AUTH_FAIL_WINDOW = 300  # 鉴权失败限流窗口（秒）


class RateLimitMiddleware(BaseHTTPMiddleware):
    """按客户端 IP 的滑窗限流：普通请求限流 + 鉴权失败冷却。"""

    def __init__(
        self,
        app,
        *,
        rate_limit: int = DEFAULT_RATE_LIMIT,
        rate_window: int = DEFAULT_RATE_WINDOW,
        auth_fail_limit: int = DEFAULT_AUTH_FAIL_LIMIT,
        auth_fail_window: int = DEFAULT_AUTH_FAIL_WINDOW,
    ):
        super().__init__(app)
        self.rate_limit = rate_limit
        self.rate_window = rate_window
        self.auth_fail_limit = auth_fail_limit
        self.auth_fail_window = auth_fail_window
        self._requests: dict[str, deque] = defaultdict(deque)
        self._auth_failures: dict[str, deque] = defaultdict(deque)

    @staticmethod
    def _client_ip(request: Request) -> str:
        if request.client:
            return request.client.host
        return "unknown"

    @staticmethod
    def _prune(queue: deque, window: int, now: float) -> None:
        while queue and queue[0] <= now - window:
            queue.popleft()

    async def dispatch(self, request: Request, call_next: Callable) -> Response:
        ip = self._client_ip(request)
        now = time.monotonic()

        # 鉴权失败冷却：失败次数超阈值时直接拒绝
        failures = self._auth_failures[ip]
        self._prune(failures, self.auth_fail_window, now)
        if len(failures) >= self.auth_fail_limit:
            return JSONResponse(
                status_code=429,
                content={"detail": "请求过于频繁，请稍后再试"},
            )

        # 普通请求限流
        requests = self._requests[ip]
        self._prune(requests, self.rate_window, now)
        if len(requests) >= self.rate_limit:
            return JSONResponse(
                status_code=429,
                content={"detail": "请求过于频繁，请稍后再试"},
            )
        requests.append(now)

        response = await call_next(request)

        if response.status_code == 401:
            failures.append(now)

        return response
