use rocket::{get, routes};

#[get("/items")]
fn items() -> &'static str {
    "[]"
}

pub fn shop() -> rocket::Rocket<rocket::Build> {
    rocket::build().mount("/shop", routes![items])
}
