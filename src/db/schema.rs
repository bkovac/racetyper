table! {
    typing_sessions (id) {
        id -> Integer,
        inputs -> Text,
        wpm -> Integer,
        wpm80 -> Integer,
        parent -> Integer,
    }
}

table! {
    typing_texts (id) {
        id -> Integer,
        text -> Text,
    }
}

allow_tables_to_appear_in_same_query!(
    typing_sessions,
    typing_texts,
);
