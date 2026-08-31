use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
pub enum BillingCycle {
    #[default]
    #[serde(rename = "Monthly")]
    Monthly,
    #[serde(rename = "Yearly")]
    Yearly,
    #[serde(rename = "Weekly")]
    Weekly,
}

impl BillingCycle {
    /// DB 中存储的是枚举名（大写），如 "MONTHLY"。
    pub fn db_value(self) -> &'static str {
        match self {
            BillingCycle::Monthly => "MONTHLY",
            BillingCycle::Yearly => "YEARLY",
            BillingCycle::Weekly => "WEEKLY",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "MONTHLY" => Some(BillingCycle::Monthly),
            "YEARLY" => Some(BillingCycle::Yearly),
            "WEEKLY" => Some(BillingCycle::Weekly),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, ToSchema)]
pub enum SubscriptionStatus {
    #[default]
    #[serde(rename = "Active")]
    Active,
    #[serde(rename = "Canceled")]
    Canceled,
}

impl SubscriptionStatus {
    /// DB 中存储的是枚举名（大写），如 "ACTIVE"。
    pub fn db_value(self) -> &'static str {
        match self {
            SubscriptionStatus::Active => "ACTIVE",
            SubscriptionStatus::Canceled => "CANCELED",
        }
    }

    pub fn from_db(s: &str) -> Option<Self> {
        match s {
            "ACTIVE" => Some(SubscriptionStatus::Active),
            "CANCELED" => Some(SubscriptionStatus::Canceled),
            _ => None,
        }
    }
}

fn default_currency() -> String {
    "CNY".to_string()
}

fn default_category() -> String {
    "其他".to_string()
}

const fn default_interval() -> i64 {
    1
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SubscriptionCreate {
    pub name: String,
    pub price: f64,
    #[serde(default = "default_currency")]
    pub currency: String,
    #[serde(default)]
    pub billing_cycle: BillingCycle,
    #[serde(default = "default_interval")]
    pub billing_interval: i64,
    pub ending_date: String,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default)]
    pub status: SubscriptionStatus,
    #[serde(default)]
    pub auto_renew: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SubscriptionOut {
    pub name: String,
    pub price: f64,
    pub currency: String,
    pub billing_cycle: BillingCycle,
    pub billing_interval: i64,
    pub ending_date: String,
    pub category: String,
    pub status: SubscriptionStatus,
    pub auto_renew: bool,
    pub id: i64,
}

#[derive(Debug, Clone, Default, Deserialize, ToSchema)]
pub struct SubscriptionUpdate {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub price: Option<f64>,
    #[serde(default)]
    pub currency: Option<String>,
    #[serde(default)]
    pub billing_cycle: Option<BillingCycle>,
    #[serde(default)]
    pub billing_interval: Option<i64>,
    #[serde(default)]
    pub ending_date: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub status: Option<SubscriptionStatus>,
    #[serde(default)]
    pub auto_renew: Option<bool>,
}
