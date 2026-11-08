use axum::routing::get;
use axum::Router;

use crate::handlers;

pub fn router() -> Router {
    Router::new().route("/api/parts", get(handlers::list))
}
