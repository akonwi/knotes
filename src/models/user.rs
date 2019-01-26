use crate::{access_token, KnotesDBConnection};
use mongodb::coll::Collection;
use mongodb::db::ThreadedDatabase;
use mongodb::Document;
use rocket::http::Status;
use rocket::request::{self, FromRequest, Request};
use rocket::Outcome;

#[derive(Serialize)]
pub struct User {
    pub id: String,
    pub email: String,
    #[serde(rename = "accessToken")]
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
        match get_by_access_token(token, &db.collection("users")) {
            Some(user) => Outcome::Success(user),
            _ => failed(),
        }
    }
}

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
