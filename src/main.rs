#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_contrib;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate validator_derive;

use rocket_contrib::json::{Json, JsonValue};
use serde::Serialize;
use validator::Validate;

#[derive(Deserialize, Serialize, Validate)]
struct RegistrationParams {
    #[validate(email(message = "Email address is invalid"))]
    pub email: String,
    #[validate(length(min = 10, message = "Password is too short"))]
    pub password: String,
}

#[derive(Serialize)]
struct User {
    pub id: String,
    pub email: String,
}

#[derive(Serialize)]
struct UserResponse {
    user: User,
}

#[derive(Serialize)]
enum Status {
    #[serde(rename = "ok")]
    Ok,
    #[serde(rename = "error")]
    Error,
}

fn ok<T: Serialize>(body: T) -> JsonValue {
    json!({
        "status": Status::Ok,
        "body": body
    })
}

fn not_ok<T: Serialize>(body: T) -> JsonValue {
    json!({
        "status": Status::Error,
        "body": body
    })
}

#[post("/auth/create", data = "<params>")]
fn register(params: Json<RegistrationParams>) -> JsonValue {
    match params.validate() {
        Ok(_) => ok(json!({
            "user": User {
                email: params.email.to_string(),
                id: String::from("me"),
            },
        })),
        Err(e) => not_ok(e),
    }
}

fn main() {
    rocket::ignite().mount("/", routes![register]).launch();
}
