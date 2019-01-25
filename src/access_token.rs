use jwt::{decode, encode, Header, Validation};

const KEY: &str = "secret"; // externalize
const SUBJECT: &str = "knotes.com";
const COMPANY: &str = "akonwi";

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    company: String,
    exp: u64,
}

pub fn create() -> Result<String, ()> {
    let my_claims = Claims {
        sub: SUBJECT.to_owned(),
        company: COMPANY.to_owned(),
        exp: 10000000000,
    };
    
    match encode(&Header::default(), &my_claims, KEY.as_ref()) {
        Ok(t) => Ok(t),
        Err(e) => {
            println!("There was an error creating a jwt token: {:?}", e);
            Err(())
        }
    }
}

pub fn is_valid(token: &str) -> bool {
    let validation = Validation {
        sub: Some(SUBJECT.to_owned()),
        ..Validation::default()
    };
    match decode::<Claims>(token, KEY.as_ref(), &validation) {
        Ok(_) => true,
        Err(err) => {
            println!("There was an error decoding a token: {:?}", err);
            false
        }
    }
}
