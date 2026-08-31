use std::sync::LazyLock;

use axum::{
    extract::Request,
    http::{HeaderValue, header::CONTENT_SECURITY_POLICY},
    middleware::Next,
    response::Response,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use sha2::{Digest, Sha256};
use tower_http::cors::CorsLayer;

use crate::{env_bool, insecure_dev_auth_enabled};

const CONSOLE_INDEX_HTML: &str = include_str!("../../../web/index.html");
static CONSOLE_CONTENT_SECURITY_POLICY: LazyLock<HeaderValue> =
    LazyLock::new(|| console_content_security_policy(false));
static CONSOLE_DEV_CONTENT_SECURITY_POLICY: LazyLock<HeaderValue> =
    LazyLock::new(|| console_content_security_policy(true));

pub(crate) async fn security_headers_middleware(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let csp = if console_loopback_connect_allowed() {
        &*CONSOLE_DEV_CONTENT_SECURITY_POLICY
    } else {
        &*CONSOLE_CONTENT_SECURITY_POLICY
    };
    response
        .headers_mut()
        .insert(CONTENT_SECURITY_POLICY, csp.clone());
    response
}

fn console_content_security_policy(allow_loopback_connect: bool) -> HeaderValue {
    let script_hash = inline_module_script_hash(CONSOLE_INDEX_HTML)
        .expect("embedded web/index.html must contain the Trunk module bootstrap");
    let connect_src = if allow_loopback_connect {
        "'self' http://127.0.0.1:* http://localhost:*"
    } else {
        "'self'"
    };
    HeaderValue::from_str(&format!(
        "default-src 'self'; script-src 'self' 'wasm-unsafe-eval' '{script_hash}'; connect-src {connect_src}; img-src 'self' data:; style-src 'self' 'unsafe-inline'; font-src 'self' data:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'"
    ))
    .expect("console content security policy must be a valid header")
}

fn inline_module_script_hash(index_html: &str) -> Option<String> {
    let (_, script_and_suffix) = index_html.split_once("<script type=\"module\">")?;
    let (script, _) = script_and_suffix.split_once("</script>")?;
    let digest = Sha256::digest(script.as_bytes());
    Some(format!("sha256-{}", STANDARD.encode(digest)))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_csp_tracks_embedded_trunk_bootstrap() {
        let script_hash = inline_module_script_hash(CONSOLE_INDEX_HTML)
            .expect("embedded console index contains module bootstrap");

        for policy in [
            &*CONSOLE_CONTENT_SECURITY_POLICY,
            &*CONSOLE_DEV_CONTENT_SECURITY_POLICY,
        ] {
            let policy = policy.to_str().expect("console CSP is text");
            assert!(policy.contains(&format!("'{script_hash}'")));
            let script_src = policy
                .split(';')
                .find(|directive| directive.trim_start().starts_with("script-src "))
                .expect("console CSP contains script-src");
            assert!(!script_src.contains("'unsafe-inline'"));
        }
    }

    #[test]
    fn console_csp_rejects_index_without_trunk_bootstrap() {
        assert_eq!(inline_module_script_hash("<html></html>"), None);
    }
}
