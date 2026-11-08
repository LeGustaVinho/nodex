use actix_web::{get, web, App, HttpResponse};

#[get("/users")]
async fn list_users() -> HttpResponse {
    HttpResponse::Ok().finish()
}

async fn version() -> HttpResponse {
    HttpResponse::Ok().body("1.0")
}

pub fn configure() -> App<()> {
    App::new()
        .service(web::scope("/v1").service(list_users))
        .route("/version", web::get().to(version))
}
