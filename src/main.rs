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

use bcrypt::{hash, DEFAULT_COST};
use jwt::{decode, encode, Algorithm, Header, Validation};
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
pub struct User {
    pub id: String,
    pub email: String,
    pub access_token: Option<String>,
}

impl User {
    pub fn from(doc: Document) -> Self {
        User {
            id: doc.get_object_id("_id").ok().unwrap().to_string(),
            email: doc.get_str("email").unwrap().to_string(),
            access_token: match doc.get_str("access_token") {
                Ok(str) => Some(String::from(str)),
                Err(_) => None,
            },
        }
    }
}

#[derive(Serialize)]
struct UserResponse {
    user: User,
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

    pub fn get_by_email(email: &str, coll: Collection) -> Option<User> {
        match coll.find_one(Some(doc! { "email": email}), None).unwrap() {
            Some(user_doc) => Some(User::from(user_doc)),
            None => None,
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

fn create_token() -> Result<String, CreateUserError> {
    #[derive(Debug, Serialize, Deserialize)]
    struct Claims {
        sub: String,
        company: String,
        exp: u64,
    };

    let my_claims = Claims {
        sub: "knotes.com".to_owned(),
        company: "akonwi".to_owned(),
        exp: 10000000000,
    };
    let key = "secret"; // externalize

    let mut header = Header::default();
    header.kid = Some("signing_key".to_owned()); // externalize key
    header.alg = Algorithm::HS512;

    match encode(&header, &my_claims, key.as_ref()) {
        Ok(t) => Ok(t),
        Err(e) => {
            println!("There was an error creating a jwt token: {:?}", e);
            Err(CreateUserError::TokenError)
        }
    }
}

fn create_user(
    conn: &mongodb::db::Database,
    email: &str,
    password: &str,
) -> Result<User, CreateUserError> {
    let collection = conn.collection("users");

    if let Some(_) = user::get_by_email(email, collection) {
        return Err(CreateUserError::AlreadyExists);
    };

    let hashed_pw = hash(&password, DEFAULT_COST).unwrap();

    let token = create_token()?;

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

#[post("/auth/create", data = "<params>")]
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

#[database("knotes")]
struct KnotesDBConnection(mongodb::db::Database);

fn main() {
    rocket::ignite()
        .attach(KnotesDBConnection::fairing())
        .mount("/", routes![register])
        .launch();
}
