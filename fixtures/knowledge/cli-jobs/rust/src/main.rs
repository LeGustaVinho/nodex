use axum::routing::get;
use axum::Router;
use clap::{Parser, Subcommand};

mod actix_api;
mod rocket_api;
mod routes;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Starts the HTTP server.
    Serve { port: u16 },
    /// Applies database migrations.
    MigrateDb,
}

async fn health() -> &'static str {
    "ok"
}

fn app() -> Router {
    let api = Router::new().route("/health", get(health));
    Router::new().nest("/api", api).nest("/api/orders", routes::order_routes())
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    match cli.command {
        Commands::Serve { port } => {
            let listener = tokio::net::TcpListener::bind(("0.0.0.0", port)).await.unwrap();
            axum::serve(listener, app()).await.unwrap();
        }
        Commands::MigrateDb => {}
    }
}
