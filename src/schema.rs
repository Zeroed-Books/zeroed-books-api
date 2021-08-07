table! {
    user (id) {
        id -> Uuid,
        created_at -> Timestamptz,
        last_login -> Nullable<Timestamptz>,
    }
}
