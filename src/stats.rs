use std::collections::{BTreeMap, HashMap};

use reqwest::Client;
use serde::Serialize;
use utoipa::ToSchema;

use crate::models::BillingCycle;

pub const FRANKFURTER_API: &str = "https://api.frankfurter.dev/v2";

#[derive(Serialize, ToSchema)]
pub struct StatsResponse {
    pub total_cost: f64,
    pub base_currency: String,
    pub active_count: usize,
    pub period: String,
    pub breakdown_by_category: BTreeMap<String, f64>,
    pub breakdown_by_currency: BTreeMap<String, f64>,
    pub rates_used: Option<BTreeMap<String, f64>>,
    pub rates_available: bool,
}

/// 单个订阅折算到 monthly/yearly 的费用，与 Python 版公式一致。
pub fn period_cost(period: &str, cycle: BillingCycle, interval: i64, price: f64) -> f64 {
    let interval = if interval < 1 { 1 } else { interval } as f64;
    if period == "yearly" {
        match cycle {
            BillingCycle::Monthly => price * 12.0 / interval,
            BillingCycle::Yearly => price / interval,
            BillingCycle::Weekly => price * 52.0 / interval,
        }
    } else {
        match cycle {
            BillingCycle::Monthly => price / interval,
            BillingCycle::Yearly => price / 12.0 / interval,
            BillingCycle::Weekly => price * 4.33 / interval,
        }
    }
}

/// 批量获取各货币到基准货币的汇率。与 Python 版行为一致：每次调用都请求一次 Frankfurter。
pub async fn fetch_all_rates_to_base(
    client: &Client,
    currencies: &[String],
    base: &str,
) -> HashMap<String, Option<f64>> {
    let mut rates: HashMap<String, Option<f64>> = HashMap::new();
    if currencies.is_empty() {
        return rates;
    }

    let need_fetch: Vec<&String> = currencies.iter().filter(|c| c.as_str() != base).collect();
    if need_fetch.is_empty() {
        rates.insert(base.to_string(), Some(1.0));
        return rates;
    }

    rates.insert(base.to_string(), Some(1.0));

    let quotes_param = need_fetch
        .iter()
        .map(|c| c.as_str())
        .chain(std::iter::once(base))
        .collect::<Vec<_>>()
        .join(",");

    let response = client
        .get(format!("{FRANKFURTER_API}/rates"))
        .query(&[("base", "EUR"), ("quotes", quotes_param.as_str())])
        .send()
        .await;

    let mark_failed = |rates: &mut HashMap<String, Option<f64>>| {
        for c in &need_fetch {
            rates.insert((*c).clone(), None);
        }
    };

    match response {
        Ok(resp) if resp.status().is_success() => match resp.json::<serde_json::Value>().await {
            Ok(data) => {
                let quotes = data.get("quotes");
                let base_rate = quotes.and_then(|q| q.get(base)).and_then(|v| v.as_f64());
                match base_rate {
                    Some(br) if br != 0.0 => {
                        for c in &need_fetch {
                            let currency_rate = quotes
                                .and_then(|q| q.get(c.as_str()))
                                .and_then(|v| v.as_f64());
                            match currency_rate {
                                Some(cr) if cr != 0.0 => {
                                    rates.insert((*c).clone(), Some(br / cr));
                                }
                                _ => {
                                    rates.insert((*c).clone(), None);
                                }
                            }
                        }
                    }
                    _ => mark_failed(&mut rates),
                }
            }
            Err(_) => mark_failed(&mut rates),
        },
        _ => mark_failed(&mut rates),
    }

    rates
}
