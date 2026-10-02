use crate::{AppState, error::ApiError};
use axum::{
    extract::{Request, State},
    http::{HeaderValue, Method, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
pub(crate) async fn guard(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let origin = req.headers().get(header::ORIGIN).cloned();
    let origin_ok = req.headers().get_all(header::ORIGIN).iter().count() <= 1
        && origin.as_ref().is_none_or(|v| {
            v.to_str()
                .is_ok_and(|v| state.config.allowed_origins.iter().any(|o| o == v))
        });
    let host_ok = req.headers().get_all(header::HOST).iter().count() == 1
        && req
            .headers()
            .get(header::HOST)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| {
                state
                    .config
                    .allowed_hosts
                    .iter()
                    .any(|h| h.eq_ignore_ascii_case(v))
            });
    let mut response = if !origin_ok || !host_ok {
        ApiError::FORBIDDEN.into_response()
    } else if req.method() == Method::OPTIONS {
        let requested = req
            .headers()
            .get(header::ACCESS_CONTROL_REQUEST_METHOD)
            .and_then(|v| v.to_str().ok());
        let headers_ok = req
            .headers()
            .get(header::ACCESS_CONTROL_REQUEST_HEADERS)
            .is_none_or(|v| {
                v.to_str().is_ok_and(|v| {
                    v.split(',')
                        .all(|h| h.trim().eq_ignore_ascii_case("content-type"))
                })
            });
        if origin.is_none() || !matches!(requested, Some("GET" | "POST")) || !headers_ok {
            ApiError::FORBIDDEN.into_response()
        } else {
            StatusCode::NO_CONTENT.into_response()
        }
    } else {
        match state.admitted.clone().try_acquire_owned() {
            Err(_) => ApiError::BUSY.into_response(),
            Ok(_permit) => {
                match tokio::time::timeout(state.config.request_timeout, next.run(req)).await {
                    Ok(response) => response,
                    Err(_) => ApiError::TIMEOUT.into_response(),
                }
            }
        }
    };
    if response.status() != StatusCode::SWITCHING_PROTOCOLS {
        let headers = response.headers_mut();
        headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
        headers.insert(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        );
        headers.insert(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        );
        headers.insert(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'"),
        );
        headers.insert(header::VARY, HeaderValue::from_static("Origin"));
        if origin_ok
            && host_ok
            && let Some(origin) = origin
        {
            headers.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
            headers.insert(
                header::ACCESS_CONTROL_ALLOW_METHODS,
                HeaderValue::from_static("GET, POST"),
            );
            headers.insert(
                header::ACCESS_CONTROL_ALLOW_HEADERS,
                HeaderValue::from_static("Content-Type"),
            );
        }
    }
    response
}
