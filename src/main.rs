use std::env;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use reqwest::Client;
use sub_manager_lite::auth;
use sub_manager_lite::build_app;
use sub_manager_lite::cron;
use sub_manager_lite::db;
use sub_manager_lite::ratelimit::RateLimiter;
use sub_manager_lite::state::AppState;

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn parse_interval(raw: &str) -> Result<u64, String> {
    let interval = raw
        .parse::<u64>()
        .map_err(|_| "SELF_CHECK_INTERVAL_MINUTES 必须是大于 0 的整数".to_string())?;
    if interval == 0 {
        return Err("SELF_CHECK_INTERVAL_MINUTES 必须是大于 0 的整数".to_string());
    }
    Ok(interval)
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // ── 配置 ──
    let api_token = env_or("API_TOKEN", "");
    if let Err(e) = auth::validate_token(&api_token) {
        eprintln!("启动失败: {e}");
        std::process::exit(1);
    }

    let db_url = env_or("DATABASE_URL", "sqlite:///./sub_manager.db");
    let base_currency = env_or("BASE_CURRENCY", "CNY");
    let interval_min = match parse_interval(&env_or("SELF_CHECK_INTERVAL_MINUTES", "60")) {
        Ok(value) => value,
        Err(e) => {
            eprintln!("启动失败: {e}");
            std::process::exit(1);
        }
    };
    let bind_addr = env_or("BIND_ADDR", "0.0.0.0:8000");

    // ── 数据库 ──
    let conn = db::open(&db_url).unwrap_or_else(|e| {
        eprintln!("无法打开或迁移数据库: {e}");
        std::process::exit(1);
    });
    let db = Arc::new(Mutex::new(conn));

    // ── 状态 ──
    let http = Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .unwrap_or_else(|e| {
            eprintln!("无法创建 HTTP 客户端: {e}");
            std::process::exit(1);
        });

    let state = AppState {
        db: db.clone(),
        api_token: api_token.clone(),
        base_currency: base_currency.clone(),
        limiter: Arc::new(RateLimiter::default()),
        http,
    };

    // ── 启动自检 ──
    {
        let mut conn = db.lock().unwrap();
        match cron::process_due_subscriptions(&mut conn) {
            Ok((c, r)) => {
                if c > 0 || r > 0 {
                    tracing::info!("启动自检完成：自动取消 {c} 个，自动续期 {r} 个");
                } else {
                    tracing::info!("启动自检完成：没有需要处理的到期订阅");
                }
            }
            Err(e) => tracing::error!("启动自检失败: {e}"),
        }
    }

    // ── 定期自检后台任务 ──
    {
        let db_bg = db.clone();
        tokio::spawn(async move {
            // 首次等待一个完整周期
            tokio::time::sleep(Duration::from_secs(interval_min * 60)).await;
            loop {
                {
                    let mut conn = db_bg.lock().unwrap();
                    match cron::process_due_subscriptions(&mut conn) {
                        Ok((c, r)) => {
                            if c > 0 || r > 0 {
                                tracing::info!("定期自检：取消 {c} 个，续期 {r} 个");
                            }
                        }
                        Err(e) => tracing::error!("定期自检失败: {e}"),
                    }
                }
                tokio::time::sleep(Duration::from_secs(interval_min * 60)).await;
            }
        });
    }

    let app = build_app(state);

    // ── 启动服务 ──
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .unwrap_or_else(|e| {
            eprintln!("无法绑定地址 {bind_addr}: {e}");
            std::process::exit(1);
        });
    tracing::info!("服务已启动，监听 {bind_addr}，自检间隔 {interval_min} 分钟");

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .expect("服务异常退出");
}

#[cfg(test)]
mod tests {
    use super::parse_interval;

    #[test]
    fn interval_must_be_positive() {
        assert_eq!(parse_interval("60"), Ok(60));
        assert!(parse_interval("0").is_err());
        assert!(parse_interval("invalid").is_err());
    }
}
