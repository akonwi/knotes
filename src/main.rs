#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate diesel;
#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_contrib;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate validator_derive;
extern crate jsonwebtoken as jwt;

mod access_token;
mod cors;
mod models;
mod schema;

use models::note::{CreateNoteParams, Note, NoteError, UpdateNoteParams};
use models::user::{CreateUserError, User};
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

type LoginParams = RegistrationParams;

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

#[derive(Serialize)]
enum AuthenticationError {
    InvalidCredentials,
}

#[post("/auth/register", data = "<params>")]
fn register(params: Json<RegistrationParams>, conn: KnotesDBConnection) -> JsonValue {
    match params.validate() {
        Ok(_) => {
            let user = match User::create(&conn, &params.email, &params.password) {
                Ok(user) => user,
                Err(e) => {
                    return not_ok(json!({
                        "type": e,
                        "message": e.message(),
                    }));
                }
            };

            ok(json!({
                "user": user,
            }))
        }
        Err(e) => not_ok(json!({
            "type": CreateUserError::InvalidAttributes,
            "message": CreateUserError::InvalidAttributes.message(),
            "errors": e
        })),
    }
}

#[post("/auth/login", data = "<params>")]
fn login(params: Json<LoginParams>, db: KnotesDBConnection) -> JsonValue {
    fn failed() -> JsonValue {
        not_ok(json!({
            "type": AuthenticationError::InvalidCredentials,
            "message": "Email and password combination is invalid"
        }))
    }

    if params.password.len() == 0 || params.email.len() == 0 {
        return failed();
    }

    let user = match User::get_by_email(&params.email, &db) {
        None => return failed(),
        Some(u) => u,
    };

    match user.verify_password(&params.password) {
        true => ok(json!({ "user": user })),
        false => failed(),
    }
}

#[get("/notes")]
fn get_notes(user: User, db: KnotesDBConnection) -> JsonValue {
    ok(json!({ "notes": Note::find_by_user(&user.id, &db)}))
}

#[post("/notes", data = "<params>")]
fn create_note(params: Json<CreateNoteParams>, user: User, db: KnotesDBConnection) -> JsonValue {
    if let Err(e) = params.validate() {
        return not_ok(json!({
            "type": NoteError::InvalidAttributes,
            "message": NoteError::InvalidAttributes.message(),
            "errors": e
        }));
    };

    match Note::create_for_user(&user.id, &params, &db) {
        Ok(note) => ok(json!({ "note": note })),
        Err(e) => not_ok(json!({"type": e, "message": e.message()})),
    }
}

#[get("/notes/<id>")]
fn get_note(id: i32, _user: User, db: KnotesDBConnection) -> JsonValue {
    ok(json!({ "note": Note::get(id, &db) }))
}

#[put("/notes/<id>", data = "<params>")]
fn update_note(
    id: i32,
    params: Json<UpdateNoteParams>,
    _user: User,
    db: KnotesDBConnection,
) -> JsonValue {
    if let Err(e) = params.validate() {
        return not_ok(json!({
            "type": NoteError::InvalidAttributes,
            "message": NoteError::InvalidAttributes.message(),
            "errors": e
        }));
    };

    let mut note = match Note::get(id, &db) {
        None => {
            return not_ok(json!({
                "type": NoteError::NotFound,
                "message": NoteError::NotFound.message()
            }));
        }
        Some(n) => n,
    };

    match note.update(params.title, params.body, &db) {
        Ok(note) => ok(json!({ "note": note })),
        Err(e) => not_ok(json!({ "type": e, "message": e.message() })),
    }
}

#[delete("/notes/<id>")]
fn delete_note(id: i32, _user: User, db: KnotesDBConnection) -> JsonValue {
    match Note::delete(id, &db) {
        Ok(_) => ok(()),
        Err(_) => not_ok(()),
    }
}

#[database("knotes")]
pub struct KnotesDBConnection(diesel::MysqlConnection);

fn main() {
    rocket::ignite()
        .attach(KnotesDBConnection::fairing())
        .attach(cors::CORS())
        .mount(
            "/",
            routes![
                register,
                login,
                get_notes,
                create_note,
                get_note,
                update_note,
                delete_note,
            ],
        )
        .launch();
}
