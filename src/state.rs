use std::sync::{Arc, Mutex};

use reqwest::Client;
use rusqlite::Connection;

use crate::ratelimit::RateLimiter;

#[derive(Clone)]
pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    pub api_token: String,
    pub base_currency: String,
    pub limiter: Arc<RateLimiter>,
    pub http: Client,
}
