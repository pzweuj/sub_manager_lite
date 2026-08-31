use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const RATE_LIMIT: usize = 120; // 每窗口最大请求数
const RATE_WINDOW: Duration = Duration::from_secs(60);
const AUTH_FAIL_LIMIT: usize = 10; // 窗口内最大鉴权失败次数
const AUTH_FAIL_WINDOW: Duration = Duration::from_secs(300);

#[derive(Default)]
struct Inner {
    requests: HashMap<String, VecDeque<Instant>>,
    failures: HashMap<String, VecDeque<Instant>>,
}

/// 进程内按 IP 的滑窗限流：普通请求限流 + 鉴权失败冷却。
/// 与 Python 版行为一致，仅适用于单实例部署。
#[derive(Default)]
pub struct RateLimiter {
    inner: Mutex<Inner>,
}

fn prune(queue: &mut VecDeque<Instant>, window: Duration, now: Instant) {
    while queue
        .front()
        .is_some_and(|t| now.duration_since(*t) >= window)
    {
        queue.pop_front();
    }
}

impl RateLimiter {
    /// 请求进入前调用。返回 true 表示放行，false 表示触发限流（429）。
    pub fn allow(&self, ip: &str) -> bool {
        let mut inner = self.inner.lock().unwrap();
        let now = Instant::now();

        let failures = inner.failures.entry(ip.to_string()).or_default();
        prune(failures, AUTH_FAIL_WINDOW, now);
        if failures.len() >= AUTH_FAIL_LIMIT {
            return false;
        }

        let requests = inner.requests.entry(ip.to_string()).or_default();
        prune(requests, RATE_WINDOW, now);
        if requests.len() >= RATE_LIMIT {
            return false;
        }
        requests.push_back(now);
        true
    }

    /// 响应为 401 时记录一次鉴权失败。
    pub fn record_failure(&self, ip: &str) {
        let mut inner = self.inner.lock().unwrap();
        let now = Instant::now();
        let failures = inner.failures.entry(ip.to_string()).or_default();
        prune(failures, AUTH_FAIL_WINDOW, now);
        failures.push_back(now);
    }
}
