pub mod config;
mod error;
pub(crate) mod live_service;
mod preflight;
mod request;
pub mod routes;
mod security;
pub use config::{Config, ConfiguredNode};
pub(crate) use live_service::LiveService;
pub use live_service::MAX_WS_CLIENTS;
use std::sync::Arc;
use tokio::sync::Semaphore;
#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) config: Arc<Config>,
    pub(crate) workers: Arc<Semaphore>,
    pub(crate) admitted: Arc<Semaphore>,
    pub(crate) live: Option<Arc<LiveService>>,
}
pub fn app() -> axum::Router {
    app_with_config(Config::default())
}
pub fn app_with_config(config: Config) -> axum::Router {
    let live = config
        .node
        .as_ref()
        .map(|node| LiveService::new(node.clone()));
    let state = AppState {
        config: Arc::new(config),
        workers: Arc::new(Semaphore::new(2)),
        admitted: Arc::new(Semaphore::new(4)),
        live,
    };
    axum::Router::new()
        .route("/api/v1/live/stream", axum::routing::get(routes::ws_stream))
        .fallback(routes::dispatch)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            security::guard,
        ))
        .with_state(state)
}
