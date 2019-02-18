use crate::schema::users;
use crate::{access_token, KnotesDBConnection};
use bcrypt::{hash, verify, DEFAULT_COST};
use diesel::prelude::*;
use diesel::{MysqlConnection, RunQueryDsl};
use rocket::http::Status;
use rocket::request::{self, FromRequest, Request};
use rocket::Outcome;

#[derive(Serialize, Queryable)]
pub struct User {
    pub id: i32,
    pub email: String,
    #[serde(rename = "accessToken")]
    pub access_token: Option<String>,
    #[serde(skip_serializing)]
    password: String,
}

impl User {
    pub fn verify_password(&self, other_password: &str) -> bool {
        match verify(other_password, self.password()) {
            Ok(pw_match) => pw_match,
            Err(e) => {
                println!("Error verifying password: #{:?}", e);
                false
            }
        }
    }
}

#[derive(Insertable)]
#[table_name = "users"]
struct NewUser<'a> {
    email: &'a str,
    password: &'a str,
    access_token: Option<&'a str>,
}

#[derive(Serialize)]
pub enum CreateUserError {
    AlreadyExists,
    InvalidAttributes,
    DBWrite,
    TokenError,
}

impl CreateUserError {
    pub fn message(self) -> &'static str {
        match self {
            CreateUserError::AlreadyExists => "Email is in use",
            CreateUserError::InvalidAttributes => "Attributes are invalid",
            CreateUserError::DBWrite => "There was an error writing to the databse",
            CreateUserError::TokenError => "Unable to create token",
        }
    }
}

impl User {
    pub fn create(
        conn: &MysqlConnection,
        email: &str,
        password: &str,
    ) -> Result<User, CreateUserError> {
        use users::dsl;

        if let Some(_) = User::get_by_email(email, conn) {
            return Err(CreateUserError::AlreadyExists);
        }

        let hashed_pw = hash(&password, DEFAULT_COST).unwrap();
        let token = match access_token::create() {
            Err(e) => {
                println!("Error creating access token: {:?}", e);
                return Err(CreateUserError::TokenError);
            }
            Ok(t) => t,
        };

        let new_user = NewUser {
            email,
            password: &hashed_pw,
            access_token: Some(&token),
        };

        if let Err(e) = diesel::insert_into(users::table)
            .values(&new_user)
            .execute(conn)
        {
            println!("Error saving new user: {:?}", e);
            return Err(CreateUserError::AlreadyExists);
        };

        match dsl::users.order(dsl::id.desc()).first(conn) {
            Ok(user) => Ok(user),
            Err(_) => Err(CreateUserError::AlreadyExists),
        }
    }

    pub fn get_by_email(email: &str, conn: &MysqlConnection) -> Option<User> {
        use users::dsl;
        match dsl::users
            .filter(users::email.eq(email))
            .first(conn)
            .optional()
        {
            Ok(result) => result,
            Err(e) => {
                println!("Error querying users by email: {:?}", e);
                None
            }
        }
    }

    pub fn get_by_access_token(token: &str, conn: &MysqlConnection) -> Option<User> {
        use users::dsl;
        match dsl::users
            .filter(users::access_token.eq(token))
            .first(conn)
            .optional()
        {
            Ok(user) => user,
            Err(e) => {
                println!("Error querying users by access_token: {:?}", e);
                None
            }
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
        match User::get_by_access_token(token, &db) {
            Some(user) => Outcome::Success(user),
            _ => failed(),
        }
    }
}
