use std::collections::BTreeMap;
use std::net::SocketAddr;

use axum::extract::{
    rejection::{JsonRejection, PathRejection, QueryRejection},
    ConnectInfo, Path, Query, Request, State,
};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::{Duration, NaiveDate};
use serde::Deserialize;
use serde_json::json;

use crate::auth;
use crate::db::{self, SubRow};
use crate::error;
use crate::models::*;
use crate::state::AppState;
use crate::stats;

// ──────────────────────── helpers ────────────────────────

fn client_ip(req: &Request) -> String {
    req.extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn get_token(headers: &HeaderMap) -> Option<&str> {
    headers.get("X-API-Token").and_then(|v| v.to_str().ok())
}

// ──────────────────────── 中间件 ────────────────────────

/// 全局限流中间件（IP 滑窗 + 鉴权失败冷却）。
pub async fn rate_limit_middleware(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let ip = client_ip(&req);
    if !state.limiter.allow(&ip) {
        return error::err(StatusCode::TOO_MANY_REQUESTS, "请求过于频繁，请稍后再试");
    }
    let resp = next.run(req).await;
    if resp.status() == StatusCode::UNAUTHORIZED {
        state.limiter.record_failure(&ip);
    }
    resp
}

/// /subscriptions 路由的鉴权中间件。
pub async fn auth_middleware(State(state): State<AppState>, req: Request, next: Next) -> Response {
    // `main` rejects this configuration before binding a socket. Keep the
    // same defensive response for callers that construct the router directly
    // (for example, integration tests or embedded deployments).
    if state.api_token.is_empty() {
        return error::unconfigured_token();
    }
    let token = get_token(req.headers());
    match token {
        None | Some("") => error::missing_token(),
        Some(t) if auth::token_matches(t, &state.api_token) => next.run(req).await,
        Some(_) => error::invalid_token(),
    }
}

// ──────────────────────── 查询参数 ────────────────────────

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListParams {
    /// 订阅名称搜索关键字，支持模糊匹配。
    pub name: Option<String>,
    /// 订阅状态过滤：Active 或 Canceled。
    pub status: Option<String>,
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct StatsParams {
    /// 统计周期：monthly 或 yearly。
    #[param(default = "monthly")]
    pub period: Option<String>,
    /// 基准货币代码，默认使用 BASE_CURRENCY。
    pub base_currency: Option<String>,
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct UpcomingParams {
    /// 查询未来 N 天内即将扣费的订阅。
    #[param(default = 7, minimum = 1, maximum = 365)]
    pub days: Option<u32>,
}

// ──────────────────────── 通用响应 ────────────────────────

#[utoipa::path(
    get,
    path = "/",
    tag = "系统",
    summary = "服务健康检查",
    responses((status = 200, description = "服务运行正常"))
)]
pub async fn root() -> Json<serde_json::Value> {
    Json(json!({
        "service": "Sub Manager Lite",
        "status": "running",
        "version": "1.0.0",
        "docs": "/docs"
    }))
}

fn sub_to_out(r: &SubRow) -> SubscriptionOut {
    db::to_out(r)
}

// ──────────────────────── 验证逻辑 ────────────────────────

fn validate_create(c: &SubscriptionCreate) -> Result<(), &'static str> {
    if c.price <= 0.0 {
        return Err("price 必须为正数");
    }
    if c.billing_interval < 1 {
        return Err("billing_interval 必须 >= 1");
    }
    NaiveDate::parse_from_str(&c.ending_date, "%Y-%m-%d")
        .map_err(|_| "ending_date 格式应为 YYYY-MM-DD")?;
    Ok(())
}

fn validate_update(u: &SubscriptionUpdate) -> Result<(), &'static str> {
    if let Some(p) = u.price {
        if p <= 0.0 {
            return Err("price 必须为正数");
        }
    }
    if let Some(i) = u.billing_interval {
        if i < 1 {
            return Err("billing_interval 必须 >= 1");
        }
    }
    if let Some(ref d) = u.ending_date {
        NaiveDate::parse_from_str(d, "%Y-%m-%d").map_err(|_| "ending_date 格式应为 YYYY-MM-DD")?;
    }
    Ok(())
}

fn invalid_json(rejection: JsonRejection) -> Response {
    tracing::debug!(error = %rejection, "请求 JSON 校验失败");
    error::validation_error("请求数据无效")
}

fn invalid_query(rejection: QueryRejection) -> Response {
    tracing::debug!(error = %rejection, "查询参数解析失败");
    error::validation_error("查询参数无效")
}

fn invalid_path(rejection: PathRejection) -> Response {
    tracing::debug!(error = %rejection, "路径参数解析失败");
    error::validation_error("路径参数无效")
}

// ──────────────────────── CRUD 接口 ────────────────────────

#[utoipa::path(
    post,
    path = "/subscriptions/",
    tag = "订阅管理",
    request_body = SubscriptionCreate,
    responses(
        (status = 201, description = "订阅创建成功", body = SubscriptionOut),
        (status = 422, description = "请求数据无效")
    )
)]
pub async fn create_subscription(
    State(state): State<AppState>,
    body: Result<Json<SubscriptionCreate>, JsonRejection>,
) -> Response {
    let Json(body) = match body {
        Ok(body) => body,
        Err(rejection) => return invalid_json(rejection),
    };
    if let Err(detail) = validate_create(&body) {
        return error::validation_error(detail);
    }
    let db = state.db.lock().unwrap();
    match db::insert(&db, &body) {
        Ok(id) => match db::get(&db, id) {
            Ok(Some(r)) => (StatusCode::CREATED, Json(sub_to_out(&r))).into_response(),
            Ok(None) => error::internal_error(),
            Err(e) => error::internal_error_with_log("创建后读取失败", e),
        },
        Err(e) => error::internal_error_with_log("创建订阅失败", e),
    }
}

#[utoipa::path(
    put,
    path = "/subscriptions/{id}",
    tag = "订阅管理",
    params(("id" = i64, Path, description = "订阅记录 ID")),
    request_body = SubscriptionUpdate,
    responses(
        (status = 200, description = "更新成功", body = SubscriptionOut),
        (status = 404, description = "订阅不存在"),
        (status = 422, description = "请求数据无效")
    )
)]
pub async fn update_subscription(
    State(state): State<AppState>,
    id: Result<Path<i64>, PathRejection>,
    body: Result<Json<SubscriptionUpdate>, JsonRejection>,
) -> Response {
    let Path(id) = match id {
        Ok(id) => id,
        Err(rejection) => return invalid_path(rejection),
    };
    let Json(body) = match body {
        Ok(body) => body,
        Err(rejection) => return invalid_json(rejection),
    };
    if let Err(detail) = validate_update(&body) {
        return error::validation_error(detail);
    }
    let db = state.db.lock().unwrap();
    match db::get(&db, id) {
        Ok(None) => error::not_found(id),
        Err(e) => error::internal_error_with_log("更新前读取失败", e),
        Ok(Some(_)) => match db::update(&db, id, &body) {
            Ok(true) => match db::get(&db, id) {
                Ok(Some(r)) => Json(sub_to_out(&r)).into_response(),
                Ok(None) => error::internal_error(),
                Err(e) => error::internal_error_with_log("更新后读取失败", e),
            },
            Ok(false) => error::not_found(id),
            Err(e) => error::internal_error_with_log("更新订阅失败", e),
        },
    }
}

#[utoipa::path(
    put,
    path = "/subscriptions/{id}/cancel",
    tag = "订阅管理",
    params(("id" = i64, Path, description = "订阅记录 ID")),
    responses(
        (status = 200, description = "订阅取消成功", body = SubscriptionOut),
        (status = 400, description = "订阅已处于取消状态"),
        (status = 404, description = "订阅不存在")
    )
)]
pub async fn cancel_subscription(
    State(state): State<AppState>,
    id: Result<Path<i64>, PathRejection>,
) -> Response {
    let Path(id) = match id {
        Ok(id) => id,
        Err(rejection) => return invalid_path(rejection),
    };
    let db = state.db.lock().unwrap();
    match db::get(&db, id) {
        Ok(None) => error::not_found(id),
        Err(e) => error::internal_error_with_log("取消前读取失败", e),
        Ok(Some(r)) => {
            if r.status == SubscriptionStatus::Canceled {
                return error::bad_request("该订阅已处于取消状态");
            }
            if let Err(e) = db::set_status(&db, id, SubscriptionStatus::Canceled, true) {
                return error::internal_error_with_log("取消订阅失败", e);
            }
            match db::get(&db, id) {
                Ok(Some(r)) => Json(sub_to_out(&r)).into_response(),
                Ok(None) => error::internal_error(),
                Err(e) => error::internal_error_with_log("取消后读取失败", e),
            }
        }
    }
}

#[utoipa::path(
    put,
    path = "/subscriptions/{id}/restore",
    tag = "订阅管理",
    params(("id" = i64, Path, description = "订阅记录 ID")),
    responses(
        (status = 200, description = "订阅恢复成功", body = SubscriptionOut),
        (status = 400, description = "订阅已处于活跃状态"),
        (status = 404, description = "订阅不存在")
    )
)]
pub async fn restore_subscription(
    State(state): State<AppState>,
    id: Result<Path<i64>, PathRejection>,
) -> Response {
    let Path(id) = match id {
        Ok(id) => id,
        Err(rejection) => return invalid_path(rejection),
    };
    let db = state.db.lock().unwrap();
    match db::get(&db, id) {
        Ok(None) => error::not_found(id),
        Err(e) => error::internal_error_with_log("恢复前读取失败", e),
        Ok(Some(r)) => {
            if r.status == SubscriptionStatus::Active {
                return error::bad_request("该订阅已处于活跃状态");
            }
            if let Err(e) = db::set_status(&db, id, SubscriptionStatus::Active, false) {
                return error::internal_error_with_log("恢复订阅失败", e);
            }
            match db::get(&db, id) {
                Ok(Some(r)) => Json(sub_to_out(&r)).into_response(),
                Ok(None) => error::internal_error(),
                Err(e) => error::internal_error_with_log("恢复后读取失败", e),
            }
        }
    }
}

#[utoipa::path(
    delete,
    path = "/subscriptions/{id}",
    tag = "订阅管理",
    params(("id" = i64, Path, description = "订阅记录 ID")),
    responses(
        (status = 204, description = "删除成功"),
        (status = 404, description = "订阅不存在")
    )
)]
pub async fn delete_subscription(
    State(state): State<AppState>,
    id: Result<Path<i64>, PathRejection>,
) -> Response {
    let Path(id) = match id {
        Ok(id) => id,
        Err(rejection) => return invalid_path(rejection),
    };
    let db = state.db.lock().unwrap();
    match db::get(&db, id) {
        Ok(None) => error::not_found(id),
        Err(e) => error::internal_error_with_log("删除前读取失败", e),
        Ok(Some(_)) => match db::delete(&db, id) {
            Ok(true) => StatusCode::NO_CONTENT.into_response(),
            Ok(false) => error::not_found(id),
            Err(e) => error::internal_error_with_log("删除订阅失败", e),
        },
    }
}

#[utoipa::path(
    get,
    path = "/subscriptions/",
    tag = "订阅管理",
    params(ListParams),
    responses((status = 200, description = "查询成功", body = [SubscriptionOut]))
)]
pub async fn list_subscriptions(
    State(state): State<AppState>,
    params: Result<Query<ListParams>, QueryRejection>,
) -> Response {
    let Query(params) = match params {
        Ok(params) => params,
        Err(rejection) => return invalid_query(rejection),
    };
    let status_filter = match params.status.as_deref() {
        Some("Active") => Some(SubscriptionStatus::Active),
        Some("Canceled") => Some(SubscriptionStatus::Canceled),
        Some(_) => return error::validation_error("status 参数只能是 Active 或 Canceled"),
        None => None,
    };
    let db = state.db.lock().unwrap();
    match db::list(&db, params.name.as_deref(), status_filter) {
        Ok(rows) => {
            let out: Vec<_> = rows.iter().map(sub_to_out).collect();
            Json(out).into_response()
        }
        Err(e) => error::internal_error_with_log("查询订阅列表失败", e),
    }
}

// ──────────────────────── stats ────────────────────────

#[utoipa::path(
    get,
    path = "/subscriptions/stats",
    tag = "订阅管理",
    params(StatsParams),
    responses((status = 200, description = "统计成功", body = crate::stats::StatsResponse))
)]
pub async fn get_stats(
    params: Result<Query<StatsParams>, QueryRejection>,
    State(state): State<AppState>,
) -> Response {
    let Query(params) = match params {
        Ok(params) => params,
        Err(rejection) => return invalid_query(rejection),
    };
    let period = params.period.as_deref().unwrap_or("monthly");
    if !matches!(period, "monthly" | "yearly") {
        return error::validation_error("period 参数只能是 monthly 或 yearly");
    }
    // Match Python's ``base_currency or DEFAULT_BASE_CURRENCY`` behavior:
    // an explicitly empty query value still uses the configured default.
    let base = params
        .base_currency
        .as_deref()
        .filter(|currency| !currency.is_empty())
        .unwrap_or(&state.base_currency);

    let today = crate::cron::today();
    let today_str = today.format("%Y-%m-%d").to_string();

    // 只读取一次，确保汇率请求期间的统计使用同一份订阅快照。
    let rows = {
        let db = state.db.lock().unwrap();
        match db::active_unexpired(&db, &today_str) {
            Ok(r) => r,
            Err(e) => return error::internal_error_with_log("统计前读取失败", e),
        }
    };
    let active_count = rows.len();
    let mut breakdown_by_currency: BTreeMap<String, f64> = BTreeMap::new();
    for r in &rows {
        let cost = stats::period_cost(period, r.billing_cycle, r.billing_interval, r.price);
        let currency = if r.currency.is_empty() {
            "CNY"
        } else {
            r.currency.as_str()
        };
        *breakdown_by_currency
            .entry(currency.to_string())
            .or_insert(0.0) += cost;
    }
    let currencies: Vec<String> = breakdown_by_currency.keys().cloned().collect();

    let rates = stats::fetch_all_rates_to_base(&state.http, &currencies, base).await;
    let rates_available = rates.values().all(|v| v.is_some());

    let mut total_cost = 0.0_f64;
    let mut rates_used: BTreeMap<String, f64> = BTreeMap::new();
    let mut breakdown_by_category: BTreeMap<String, f64> = BTreeMap::new();
    for r in &rows {
        let cost = stats::period_cost(period, r.billing_cycle, r.billing_interval, r.price);
        let currency = if r.currency.is_empty() {
            "CNY"
        } else {
            r.currency.as_str()
        };
        if let Some(Some(rate)) = rates.get(currency) {
            let converted = cost * rate;
            total_cost += converted;
            rates_used.insert(currency.to_string(), *rate);
            let category = if r.category.is_empty() {
                "其他"
            } else {
                r.category.as_str()
            };
            *breakdown_by_category
                .entry(category.to_string())
                .or_insert(0.0) += converted;
        }
    }

    let rates_used_out = if rates_used.is_empty() {
        None
    } else {
        Some(rates_used)
    };

    let total_cost = (total_cost * 100.0).round() / 100.0;

    let resp = stats::StatsResponse {
        total_cost,
        base_currency: base.to_string(),
        active_count,
        period: period.to_string(),
        breakdown_by_category,
        breakdown_by_currency,
        rates_used: rates_used_out,
        rates_available,
    };

    Json(resp).into_response()
}

// ──────────────────────── upcoming / expired ────────────────────────

#[utoipa::path(
    get,
    path = "/subscriptions/upcoming",
    tag = "订阅管理",
    params(UpcomingParams),
    responses((status = 200, description = "查询成功", body = [SubscriptionOut]))
)]
pub async fn get_upcoming_bills(
    State(state): State<AppState>,
    params: Result<Query<UpcomingParams>, QueryRejection>,
) -> Response {
    let Query(params) = match params {
        Ok(params) => params,
        Err(rejection) => return invalid_query(rejection),
    };
    let days = params.days.unwrap_or(7);
    if !(1..=365).contains(&days) {
        return error::validation_error("days 参数必须在 1 到 365 之间");
    }
    let today = crate::cron::today();
    let end = today + Duration::days(days as i64);

    let db = state.db.lock().unwrap();
    match db::upcoming(
        &db,
        &today.format("%Y-%m-%d").to_string(),
        &end.format("%Y-%m-%d").to_string(),
    ) {
        Ok(rows) => {
            let out: Vec<_> = rows.iter().map(sub_to_out).collect();
            Json(out).into_response()
        }
        Err(e) => error::internal_error_with_log("查询即将扣费订阅失败", e),
    }
}

#[utoipa::path(
    get,
    path = "/subscriptions/expired",
    tag = "订阅管理",
    responses((status = 200, description = "查询成功", body = [SubscriptionOut]))
)]
pub async fn get_expired_subscriptions(State(state): State<AppState>) -> Response {
    let today = crate::cron::today();
    let db = state.db.lock().unwrap();
    match db::expired(&db, &today.format("%Y-%m-%d").to_string()) {
        Ok(rows) => {
            let out: Vec<_> = rows.iter().map(sub_to_out).collect();
            Json(out).into_response()
        }
        Err(e) => error::internal_error_with_log("查询过期订阅失败", e),
    }
}
