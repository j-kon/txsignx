//! Bounded local HTTP boundary. All inspection and decisions remain in the engine.
pub mod config;
mod error;
mod preflight;
mod request;
mod routes;
mod security;
pub use config::{Config, ConfiguredNode};
use std::sync::Arc;
use tokio::sync::Semaphore;
#[derive(Clone)]
struct AppState {
    config: Arc<Config>,
    workers: Arc<Semaphore>,
    admitted: Arc<Semaphore>,
}
pub fn app() -> axum::Router {
    app_with_config(Config::default())
}
pub fn app_with_config(config: Config) -> axum::Router {
    let state = AppState {
        config: Arc::new(config),
        workers: Arc::new(Semaphore::new(2)),
        admitted: Arc::new(Semaphore::new(4)),
    };
    axum::Router::new()
        .fallback(routes::dispatch)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            security::guard,
        ))
        .with_state(state)
}
