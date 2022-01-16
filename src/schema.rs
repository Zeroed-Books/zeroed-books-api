table! {
    account (id) {
        id -> Uuid,
        user_id -> Uuid,
        name -> Text,
        created_at -> Timestamptz,
    }
}

table! {
    currency (code) {
        code -> Text,
        symbol -> Text,
        minor_units -> Int2,
    }
}

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
    transaction (id) {
        id -> Uuid,
        user_id -> Uuid,
        date -> Date,
        payee -> Text,
        notes -> Text,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

table! {
    transaction_entry (id) {
        id -> Uuid,
        transaction_id -> Uuid,
        order -> Int4,
        account_id -> Uuid,
        currency -> Text,
        amount -> Int4,
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

joinable!(account -> user (user_id));
joinable!(email -> user (user_id));
joinable!(email_verification -> email (email_id));
joinable!(transaction -> user (user_id));
joinable!(transaction_entry -> account (account_id));
joinable!(transaction_entry -> currency (currency));
joinable!(transaction_entry -> transaction (transaction_id));

allow_tables_to_appear_in_same_query!(
    account,
    currency,
    email,
    email_verification,
    transaction,
    transaction_entry,
    user,
);
