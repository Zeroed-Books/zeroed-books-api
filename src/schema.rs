table! {
    email (normalized_address) {
        provided_address -> Text,
        normalized_address -> Text,
        user_id -> Uuid,
        created_at -> Timestamptz,
        verified_at -> Nullable<Timestamptz>,
    }
}

table! {
    user (id) {
        id -> Uuid,
        created_at -> Timestamptz,
        last_login -> Nullable<Timestamptz>,
    }
}

joinable!(email -> user (user_id));

allow_tables_to_appear_in_same_query!(
    email,
    user,
);
