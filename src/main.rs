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
extern crate jsonwebtoken as jwt;

mod access_token;
mod models;

use bcrypt::{hash, verify, DEFAULT_COST};
use models::note::{self, Note, UpdateNoteParams};
use models::user::{self, User};
use mongodb::db::ThreadedDatabase;
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

#[derive(Serialize, Deserialize, Validate)]
struct CreateNoteParams {
    #[validate(length(min = 1, message = "Note title is required"))]
    title: String,
    body: Option<String>,
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

#[derive(Serialize)]
enum CreateUserError {
    AlreadyExists,
    InvalidAttributes,
    DBWrite,
    TokenError,
}

impl CreateUserError {
    fn message(self) -> &'static str {
        match self {
            CreateUserError::AlreadyExists => "Email is in use",
            CreateUserError::InvalidAttributes => "Attributes are invalid",
            CreateUserError::DBWrite => "There was an error writing to the databse",
            CreateUserError::TokenError => "Unable to create token",
        }
    }
}

#[derive(Serialize)]
enum AuthenticationError {
    InvalidCredentials,
}

#[derive(Serialize)]
enum CreateNoteError {
    InvalidAttributes,
    DBWrite,
}

impl CreateNoteError {
    fn message(self) -> &'static str {
        match self {
            CreateNoteError::InvalidAttributes => "Note attributes are invalid",
            CreateNoteError::DBWrite => "There was an error saving the note",
        }
    }
}

fn create_user(
    conn: &mongodb::db::Database,
    email: &str,
    password: &str,
) -> Result<User, CreateUserError> {
    let collection = conn.collection("users");

    if let Some(_) = user::get_by_email(email, &collection) {
        return Err(CreateUserError::AlreadyExists);
    };

    let hashed_pw = hash(&password, DEFAULT_COST).unwrap();

    let token = match access_token::create() {
        Err(_) => return Err(CreateUserError::TokenError),
        Ok(t) => t,
    };

    let coll = conn.collection("users");
    let result = coll
        .insert_one(
            doc! {"email": email, "password": hashed_pw, "access_token": token},
            None,
        )
        .unwrap();
    match coll
        .find_one(Some(doc! {"_id": result.inserted_id.unwrap()}), None)
        .unwrap()
    {
        Some(doc) => Ok(User::from(doc)),
        None => Err(CreateUserError::DBWrite),
    }
}

#[post("/auth/register", data = "<params>")]
fn register(params: Json<RegistrationParams>, conn: KnotesDBConnection) -> JsonValue {
    match params.validate() {
        Ok(_) => {
            let user = match create_user(&conn, &params.email, &params.password) {
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
fn login(params: Json<LoginParams>, conn: KnotesDBConnection) -> JsonValue {
    fn failed() -> JsonValue {
        not_ok(json!({
            "type": AuthenticationError::InvalidCredentials,
            "message": "Email and password combination is invalid"
        }))
    }

    if params.password.len() == 0 || params.email.len() == 0 {
        return failed();
    }

    let u = match user::get_by_email(&params.email, &conn.collection("users")) {
        None => return failed(),
        Some(u) => u,
    };

    match verify(&params.password, u.password()) {
        Ok(pw_match) => {
            if pw_match {
                ok(json!({ "user": u }))
            } else {
                failed()
            }
        }
        Err(e) => {
            println!("Error verifying password: #{:?}", e);
            failed()
        }
    }
}

#[get("/notes")]
fn get_notes(user: User, db: KnotesDBConnection) -> JsonValue {
    ok(json!({ "notes": note::find_by_user(&user.id, &db)}))
}

#[post("/notes", data = "<params>")]
fn create_note(params: Json<CreateNoteParams>, user: User, db: KnotesDBConnection) -> JsonValue {
    if let Err(e) = params.validate() {
        return not_ok(json!({
            "type": CreateNoteError::InvalidAttributes,
            "message": CreateNoteError::InvalidAttributes.message(),
            "errors": e
        }));
    };

    match note::create_for_user(&user.id, &params.title, params.body.as_ref(), &db) {
        Ok(note) => ok(json!({ "note": note })),
        Err(_) => not_ok(
            json!({"type": CreateNoteError::DBWrite, "message": CreateNoteError::DBWrite.message()}),
        ),
    }
}

#[get("/notes/<id>")]
fn get_note(id: String, _user: User, db: KnotesDBConnection) -> JsonValue {
    ok(json!({ "note": note::get(&id, &db) }))
}

#[put("/notes/<id>", data = "<params>")]
fn update_note(
    id: String,
    params: Json<UpdateNoteParams>,
    _user: User,
    db: KnotesDBConnection,
) -> JsonValue {
    if let Err(e) = params.validate() {
        return not_ok(json!({
            "type": CreateNoteError::InvalidAttributes,
            "message": CreateNoteError::InvalidAttributes.message(),
            "errors": e
        }));
    };

    match Note::update(&id, params.0, &db) {
        Ok(note) => ok(json!({ "note": note })),
        Err(_) => {
            not_ok(json!({ "type": "DBWrite", "message": "There was an error saving the note" }))
        }
    }
}

#[delete("/notes/<id>")]
fn delete_note(id: String, _user: User, db: KnotesDBConnection) -> JsonValue {
    match Note::delete(&id, &db) {
        Ok(_) => ok(()),
        Err(_) => not_ok(())
    }
}

#[database("knotes")]
pub struct KnotesDBConnection(mongodb::db::Database);

fn main() {
    rocket::ignite()
        .attach(KnotesDBConnection::fairing())
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
