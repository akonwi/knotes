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

use bcrypt::{hash, verify, DEFAULT_COST};
use mongodb::db::ThreadedDatabase;
use mongodb::Document;
use rocket::http::Status;
use rocket::request::{self, FromRequest, Request};
use rocket::Outcome;
use rocket_contrib::json::{Json, JsonValue};
use serde::Serialize;
use validator::Validate;

mod access_token;

#[derive(Deserialize, Serialize, Validate)]
struct RegistrationParams {
    #[validate(email(message = "Email address is invalid"))]
    pub email: String,
    #[validate(length(min = 10, message = "Password is too short"))]
    pub password: String,
}

type LoginParams = RegistrationParams;

#[derive(Serialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub access_token: Option<String>,
    #[serde(skip_serializing)]
    password: String,
}

impl User {
    pub fn from(doc: Document) -> Self {
        User {
            id: doc.get_object_id("_id").ok().unwrap().to_string(),
            email: doc.get_str("email").unwrap().to_string(),
            password: doc.get_str("password").unwrap().to_string(),
            access_token: match doc.get_str("access_token") {
                Ok(str) => Some(String::from(str)),
                Err(_) => None,
            },
        }
    }

    pub fn password(&self) -> &str {
        &self.password[..]
    }
}

impl<'a, 'r> FromRequest<'a, 'r> for User {
    type Error = ();

    fn from_request(request: &'a Request<'r>) -> request::Outcome<User, ()> {
        let failed = || Outcome::Failure((Status::Unauthorized, ()));

        let token = match request.headers().get_one("Authorization") {
            None => return failed(),
            Some(header) => &header[7..],
        };

        if access_token::is_valid(token) == false {
            println!("token is invalid");
            return failed();
        };

        let db = request.guard::<KnotesDBConnection>()?;
        match user::get_by_access_token(token, &db.collection("users")) {
            Some(user) => Outcome::Success(user),
            _ => failed(),
        }
    }
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

mod user {
    use super::User;
    use mongodb::coll::Collection;

    pub fn get_by_email(email: &str, coll: &Collection) -> Option<User> {
        match coll.find_one(Some(doc! { "email": email}), None).unwrap() {
            Some(user_doc) => Some(User::from(user_doc)),
            None => None,
        }
    }

    pub fn get_by_access_token(token: &str, coll: &Collection) -> Option<User> {
        match coll
            .find_one(Some(doc! { "access_token": token }), None)
            .unwrap()
        {
            Some(user_doc) => Some(User::from(user_doc)),
            _ => None,
        }
    }
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

    match verify(&params.password, &u.password) {
        Ok(b) => {
            if b {
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
fn get_notes(user: User) -> JsonValue {
    ok(json!({ "user": user }))
}

#[database("knotes")]
struct KnotesDBConnection(mongodb::db::Database);

fn main() {
    rocket::ignite()
        .attach(KnotesDBConnection::fairing())
        .mount("/", routes![register, login, get_notes])
        .launch();
}
