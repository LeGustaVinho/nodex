use axum::routing::get;
use axum::Router;

async fn list_orders() -> &'static str {
    "[]"
}

async fn get_order() -> &'static str {
    "{}"
}

pub fn order_routes() -> Router {
    Router::new().route("/", get(list_orders)).route("/:id", get(get_order))
}
