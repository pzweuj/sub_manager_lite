use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa::{Modify, OpenApi};

use crate::handlers;
use crate::models::{
    BillingCycle, SubscriptionCreate, SubscriptionOut, SubscriptionStatus, SubscriptionUpdate,
};
use crate::stats::StatsResponse;

/// Adds the same header-based authentication scheme exposed by the Python API.
struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Default::default);
        components.add_security_scheme(
            "ApiKeyAuth",
            SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::new("X-API-Token"))),
        );
        openapi.security = Some(vec![utoipa::openapi::SecurityRequirement::new(
            "ApiKeyAuth",
            Vec::<String>::new(),
        )]);
    }
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Sub Manager Lite",
        version = "1.0.0",
        description = "订阅管理服务 API"
    ),
    paths(
        handlers::root,
        handlers::create_subscription,
        handlers::update_subscription,
        handlers::cancel_subscription,
        handlers::restore_subscription,
        handlers::delete_subscription,
        handlers::list_subscriptions,
        handlers::get_stats,
        handlers::get_upcoming_bills,
        handlers::get_expired_subscriptions
    ),
    components(schemas(
        BillingCycle,
        SubscriptionStatus,
        SubscriptionCreate,
        SubscriptionUpdate,
        SubscriptionOut,
        StatsResponse
    )),
    modifiers(&SecurityAddon),
    tags(
        (name = "订阅管理", description = "订阅服务的增删改查和统计接口"),
        (name = "系统", description = "服务健康检查")
    )
)]
pub struct ApiDoc;
