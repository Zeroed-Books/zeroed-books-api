table! {
    email (id) {
        id -> Uuid,
        user_id -> Uuid,
        provided_address -> Text,
        normalized_address -> Text,
        created_at -> Timestamptz,
        verified_at -> Nullable<Timestamptz>,
    }
}

table! {
    email_verification (token) {
        token -> Text,
        email_id -> Uuid,
        created_at -> Timestamptz,
    }
}

table! {
    user (id) {
        id -> Uuid,
        password -> Text,
        created_at -> Timestamptz,
        last_login -> Nullable<Timestamptz>,
    }
}

joinable!(email -> user (user_id));
joinable!(email_verification -> email (email_id));

allow_tables_to_appear_in_same_query!(
    email,
    email_verification,
    user,
);
