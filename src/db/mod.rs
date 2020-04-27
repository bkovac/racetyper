use diesel::prelude::*;
use dotenv::dotenv;
use std::env;
use std::error;
use std::fmt;

pub mod schema;
pub mod models;

use self::models::{TypingSession, NewTypingSession, TypingText, NewTypingText};


type Result<T> = std::result::Result<T, DBError>;

#[derive(Debug, Clone)]
pub struct DBError {
    text: String,
}

impl fmt::Display for DBError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Database Error: {}", self.text)
    }
}
impl error::Error for DBError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        None
    }
}

impl From<diesel::result::Error> for DBError {
    //TODO: return the actual error
    fn from(_error: diesel::result::Error) -> Self {
        DBError {text: "wrapped unknown diesel error".to_owned()}
    }
}


pub fn establish_connection() -> SqliteConnection {
    dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    SqliteConnection::establish(&database_url).unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}


pub fn create_typing_session(conn: &SqliteConnection, session: &NewTypingSession) -> Result<TypingSession> {
    use schema::typing_sessions::dsl::*;

    return conn.transaction::<TypingSession, DBError, _>(|| {
        match diesel::insert_into(typing_sessions).values(session).execute(conn) {
            Ok(inserted_count) => {
                if inserted_count != 1 {
                    return Err(DBError{text: format!("Invalid number of inserted values: {}", inserted_count)});
                }
                //Pass
            },
            Err(_e) => {
                return Err(DBError{text: "insert_into error".to_owned()});
            }
        }

        let rows = match typing_sessions.order(id.desc()).limit(1).load::<TypingSession>(conn) {
            Ok(rows) => rows,
            Err(_e) => {
                return Err(DBError{text: "load error".to_owned()});
            },
        };

        if rows.len() != 1 {
            return Err(DBError{text: format!("Invalid number of rows in result: {}", rows.len())});
        } else {
            return Ok(rows[0].clone());
        }
    });
}

pub fn create_typing_text(conn: &SqliteConnection, text: String) -> usize {
    use schema::typing_texts;

    let new_typing_text = NewTypingText {
        text: text,
    };

    diesel::insert_into(typing_texts::table).values(&new_typing_text).execute(conn).expect("Error saving typing text")
}

pub fn get_random_typing_text(conn: &SqliteConnection) -> Result<TypingText> {
    use schema::typing_texts::dsl::*;

    no_arg_sql_function!(RANDOM, (), "Represents the sql RANDOM() function");
    let res = typing_texts.order(RANDOM).limit(1).load::<TypingText>(conn).expect("unable to load posts");

    if res.len() == 1 {
        Ok(res[0].clone())
    } else {
        Err(DBError{text: format!("Invalid number of entries for query: {}", res.len())})
    }
}
