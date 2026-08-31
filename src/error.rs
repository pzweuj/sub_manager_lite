use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use std::fmt::Display;

pub fn err(status: StatusCode, detail: &str) -> Response {
    (status, Json(json!({"detail": detail}))).into_response()
}

pub fn missing_token() -> Response {
    err(
        StatusCode::UNAUTHORIZED,
        "未提供认证 Token，请在请求头中添加 X-API-Token",
    )
}

pub fn invalid_token() -> Response {
    err(
        StatusCode::UNAUTHORIZED,
        "Token 无效或已过期，请检查 X-API-Token 是否正确",
    )
}

pub fn unconfigured_token() -> Response {
    err(
        StatusCode::INTERNAL_SERVER_ERROR,
        "API_TOKEN 环境变量未配置，请联系管理员",
    )
}

pub fn not_found(id: i64) -> Response {
    err(StatusCode::NOT_FOUND, &format!("订阅 ID {id} 不存在"))
}

pub fn bad_request(detail: &str) -> Response {
    err(StatusCode::BAD_REQUEST, detail)
}

pub fn validation_error(detail: &str) -> Response {
    err(StatusCode::UNPROCESSABLE_ENTITY, detail)
}

pub fn internal_error() -> Response {
    err(StatusCode::INTERNAL_SERVER_ERROR, "服务器内部错误")
}

pub fn internal_error_with_log<E: Display>(context: &str, source: E) -> Response {
    tracing::error!(context, error = %source, "数据库操作失败");
    internal_error()
}
