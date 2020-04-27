use super::schema::{typing_sessions, typing_texts};

#[derive(Queryable, Clone, Debug)]
pub struct TypingSession {
    pub id: i32,
    pub inputs: String,
    pub wpm: i32,
    pub wpm80: i32,
    pub parent: i32,
}

#[derive(Insertable)]
#[table_name = "typing_sessions"]
pub struct NewTypingSession {
    pub inputs: String,
    pub wpm: i32,
    pub wpm80: i32,
    pub parent: i32,
}


#[derive(Queryable, Clone, Debug)]
pub struct TypingText {
    pub id: i32,
    pub text: String,
}

#[derive(Insertable)]
#[table_name = "typing_texts"]
pub struct NewTypingText {
    pub text: String,
}
