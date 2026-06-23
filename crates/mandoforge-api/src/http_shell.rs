use axum::{
    extract::Request,
    http::{HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};
use tower_http::cors::CorsLayer;

use crate::{
    CONSOLE_CONTENT_SECURITY_POLICY, CONSOLE_DEV_CONTENT_SECURITY_POLICY, env_bool,
    insecure_dev_auth_enabled,
};

pub(crate) async fn security_headers_middleware(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let csp = if console_loopback_connect_allowed() {
        CONSOLE_DEV_CONTENT_SECURITY_POLICY
    } else {
        CONSOLE_CONTENT_SECURITY_POLICY
    };
    response.headers_mut().insert(
        HeaderName::from_static("content-security-policy"),
        HeaderValue::from_static(csp),
    );
    response
}

fn console_loopback_connect_allowed() -> bool {
    insecure_dev_auth_enabled()
        || env_bool("MANDOFORGE_CONSOLE_ALLOW_LOOPBACK_CONNECT")
        || env_bool("MANDOFORGE_ALLOW_INSECURE_CONSOLE_LOOPBACK")
}

pub(crate) fn api_cors_layer() -> CorsLayer {
    if insecure_dev_auth_enabled() {
        CorsLayer::permissive()
    } else {
        CorsLayer::new()
    }
}
