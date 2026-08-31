use std::sync::{Arc, Mutex};

use axum::body::{to_bytes, Body};
use axum::http::{Method, Request, Response, StatusCode};
use reqwest::Client;
use rusqlite::Connection;
use serde_json::{json, Value};
use sub_manager_lite::{build_app, db, state::AppState};
use tower::ServiceExt;

const TOKEN: &str = "0123456789abcdef";

fn test_state() -> (AppState, Arc<Mutex<Connection>>) {
    let conn = db::open("sqlite://").unwrap();
    let shared = Arc::new(Mutex::new(conn));
    let state = AppState {
        db: shared.clone(),
        api_token: TOKEN.to_string(),
        base_currency: "CNY".to_string(),
        limiter: Arc::new(sub_manager_lite::ratelimit::RateLimiter::default()),
        http: Client::new(),
    };
    (state, shared)
}

async fn request(
    app: &axum::Router,
    method: Method,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> Response<axum::body::Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(token) = token {
        builder = builder.header("X-API-Token", token);
    }
    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }
    let payload = body
        .map(|value| Body::from(value.to_string()))
        .unwrap_or_else(Body::empty);
    app.clone()
        .oneshot(builder.body(payload).unwrap())
        .await
        .unwrap()
}

async fn raw_request(
    app: &axum::Router,
    method: Method,
    uri: &str,
    token: Option<&str>,
    body: &str,
) -> Response<axum::body::Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header("X-API-Token", token);
    }
    app.clone()
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}

async fn json_body(response: Response<Body>) -> Value {
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn public_docs_and_auth_are_compatible() {
    let (state, _) = test_state();
    let app = build_app(state);

    let root_response = request(&app, Method::GET, "/", None, None).await;
    assert_eq!(root_response.status(), StatusCode::OK);
    let docs = request(&app, Method::GET, "/docs", None, None).await;
    assert_eq!(docs.status(), StatusCode::SEE_OTHER);
    assert_eq!(docs.headers().get("location").unwrap(), "/docs/");
    assert_eq!(
        request(&app, Method::GET, "/docs/", None, None)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        request(&app, Method::GET, "/redoc", None, None)
            .await
            .status(),
        StatusCode::OK
    );

    let openapi = request(&app, Method::GET, "/openapi.json", None, None).await;
    assert_eq!(openapi.status(), StatusCode::OK);
    let document = json_body(openapi).await;
    for path in [
        "/",
        "/subscriptions/",
        "/subscriptions/{id}",
        "/subscriptions/{id}/cancel",
        "/subscriptions/{id}/restore",
        "/subscriptions/stats",
        "/subscriptions/upcoming",
        "/subscriptions/expired",
    ] {
        assert!(
            document["paths"][path].is_object(),
            "missing OpenAPI path {path}"
        );
    }
    assert!(document["paths"]["/subscriptions/"]["get"].is_object());
    assert!(document["paths"]["/subscriptions/"]["post"].is_object());
    assert_eq!(
        document["components"]["securitySchemes"]["ApiKeyAuth"]["name"],
        "X-API-Token"
    );
    assert!(document["security"].is_array());
    assert!(
        document["paths"]["/subscriptions/stats"]["get"]["parameters"]
            .as_array()
            .unwrap()
            .iter()
            .any(|parameter| parameter["name"] == "period")
    );
    assert!(
        document["paths"]["/subscriptions/upcoming"]["get"]["parameters"]
            .as_array()
            .unwrap()
            .iter()
            .any(|parameter| parameter["name"] == "days")
    );

    let missing = request(&app, Method::GET, "/subscriptions/", None, None).await;
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        json_body(missing).await["detail"],
        "未提供认证 Token，请在请求头中添加 X-API-Token"
    );

    let invalid = request(
        &app,
        Method::GET,
        "/subscriptions/",
        Some("wrong-token"),
        None,
    )
    .await;
    assert_eq!(invalid.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        json_body(invalid).await["detail"],
        "Token 无效或已过期，请检查 X-API-Token 是否正确"
    );

    assert_eq!(
        request(&app, Method::GET, "/unknown", None, None)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn crud_and_statistics_work_with_existing_contract() {
    let (state, _) = test_state();
    let app = build_app(state);
    let created = request(
        &app,
        Method::POST,
        "/subscriptions/",
        Some(TOKEN),
        Some(json!({
            "name": "Netflix",
            "price": 10.0,
            "ending_date": "2099-01-01"
        })),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    assert_eq!(json_body(created).await["id"], 1);

    let listed = request(&app, Method::GET, "/subscriptions/", Some(TOKEN), None).await;
    assert_eq!(listed.status(), StatusCode::OK);
    assert_eq!(json_body(listed).await.as_array().unwrap().len(), 1);

    let updated = request(
        &app,
        Method::PUT,
        "/subscriptions/1",
        Some(TOKEN),
        Some(json!({"price": 12.5})),
    )
    .await;
    assert_eq!(updated.status(), StatusCode::OK);
    assert_eq!(json_body(updated).await["price"], 12.5);

    let stats = request(&app, Method::GET, "/subscriptions/stats", Some(TOKEN), None).await;
    assert_eq!(stats.status(), StatusCode::OK);
    let stats_body = json_body(stats).await;
    assert_eq!(stats_body["total_cost"], 12.5);
    assert_eq!(stats_body["active_count"], 1);
    assert_eq!(stats_body["rates_available"], true);

    let canceled = request(
        &app,
        Method::PUT,
        "/subscriptions/1/cancel",
        Some(TOKEN),
        None,
    )
    .await;
    assert_eq!(canceled.status(), StatusCode::OK);
    assert_eq!(json_body(canceled).await["status"], "Canceled");

    let restored = request(
        &app,
        Method::PUT,
        "/subscriptions/1/restore",
        Some(TOKEN),
        None,
    )
    .await;
    assert_eq!(restored.status(), StatusCode::OK);
    assert_eq!(json_body(restored).await["status"], "Active");

    let deleted = request(&app, Method::DELETE, "/subscriptions/1", Some(TOKEN), None).await;
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        request(&app, Method::GET, "/subscriptions/", Some(TOKEN), None)
            .await
            .status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn invalid_inputs_return_json_422() {
    let (state, _) = test_state();
    let app = build_app(state);

    for uri in [
        "/subscriptions/upcoming?days=0",
        "/subscriptions/upcoming?days=366",
        "/subscriptions/upcoming?days=-1",
        "/subscriptions/stats?period=weekly",
    ] {
        let response = request(&app, Method::GET, uri, Some(TOKEN), None).await;
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY, "{uri}");
        assert!(json_body(response).await["detail"].is_string());
    }

    let invalid_path = request(
        &app,
        Method::PUT,
        "/subscriptions/not-an-id",
        Some(TOKEN),
        Some(json!({})),
    )
    .await;
    assert_eq!(invalid_path.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(json_body(invalid_path).await["detail"].is_string());

    let invalid_json = raw_request(&app, Method::POST, "/subscriptions/", Some(TOKEN), "{").await;
    assert_eq!(invalid_json.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert!(json_body(invalid_json).await["detail"].is_string());
}

#[tokio::test]
async fn database_write_errors_are_not_reported_as_success() {
    let (state, shared) = test_state();
    let app = build_app(state);
    let created = request(
        &app,
        Method::POST,
        "/subscriptions/",
        Some(TOKEN),
        Some(json!({"name": "Blocked", "price": 1, "ending_date": "2099-01-01"})),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);

    shared
        .lock()
        .unwrap()
        .execute(
            "CREATE TRIGGER fail_status BEFORE UPDATE OF status ON subscription
             BEGIN SELECT RAISE(ABORT, 'blocked'); END",
            [],
        )
        .unwrap();
    let canceled = request(
        &app,
        Method::PUT,
        "/subscriptions/1/cancel",
        Some(TOKEN),
        None,
    )
    .await;
    assert_eq!(canceled.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = json_body(canceled).await;
    assert_eq!(body["detail"], "服务器内部错误");
    assert!(!body.to_string().contains("blocked"));

    shared
        .lock()
        .unwrap()
        .execute("DROP TRIGGER fail_status", [])
        .unwrap();
    shared
        .lock()
        .unwrap()
        .execute(
            "CREATE TRIGGER fail_delete BEFORE DELETE ON subscription
             BEGIN SELECT RAISE(ABORT, 'blocked delete'); END",
            [],
        )
        .unwrap();
    let deleted = request(&app, Method::DELETE, "/subscriptions/1", Some(TOKEN), None).await;
    assert_eq!(deleted.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(json_body(deleted).await["detail"], "服务器内部错误");
}
