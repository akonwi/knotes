#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate mongodb;
#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_contrib;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate validator_derive;

use bcrypt::{hash, DEFAULT_COST};
use mongodb::db::ThreadedDatabase;
use mongodb::Document;
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

impl User {
    pub fn from(doc: Document) -> Self {
        User {
            id: doc.get_object_id("_id").ok().unwrap().to_string(),
            email: doc.get_str("email").unwrap().to_string(),
        }
    }
}

#[derive(Serialize)]
struct UserResponse {
    user: User,
}

#[derive(Serialize)]
struct ErrorResponse {
    message: String,
}

fn ok<T: Serialize>(body: T) -> JsonValue {
    json!({
        "status": "ok",
        "body": body
    })
}

fn not_ok<T: Serialize>(body: T) -> JsonValue {
    json!({
        "status": "error",
        "body": body
    })
}

fn create_user(conn: &mongodb::db::Database, email: &str, password: &str) -> Result<User, ()> {
    let hashed_pw = hash(&password, DEFAULT_COST).unwrap();

    let coll = conn.collection("users");
    let result = coll
        .insert_one(doc! {"email": email, "password": hashed_pw}, None)
        .unwrap();
    match coll
        .find_one(Some(doc! {"_id": result.inserted_id.unwrap()}), None)
        .unwrap()
    {
        Some(doc) => Ok(User::from(doc)),
        None => Err(()),
    }
}

#[post("/auth/create", data = "<params>")]
fn register(params: Json<RegistrationParams>, conn: KnotesDBConnection) -> JsonValue {
    match params.validate() {
        Ok(_) => {
            let user = match create_user(&conn, &params.email, &params.password) {
                Ok(user) => user,
                Err(_) => {
                    return not_ok(ErrorResponse {
                        message: String::from("Failed to create User"),
                    });
                }
            };

            ok(json!({
                "user": user,
            }))
        }
        Err(e) => not_ok(e),
    }
}

#[database("knotes")]
struct KnotesDBConnection(mongodb::db::Database);

fn main() {
    rocket::ignite()
        .attach(KnotesDBConnection::fairing())
        .mount("/", routes![register])
        .launch();
}
