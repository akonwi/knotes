table! {
    notes (id) {
        id -> Integer,
        user_id -> Integer,
        title -> Varchar,
        body -> Text,
    }
}

table! {
    users (id) {
        id -> Integer,
        email -> Varchar,
        access_token -> Nullable<Text>,
        password -> Varchar,
    }
}

joinable!(notes -> users (user_id));

allow_tables_to_appear_in_same_query!(notes, users,);
