table! {
    account (id) {
        id -> Uuid,
        user_id -> Uuid,
        name -> Text,
        created_at -> Timestamptz,
    }
}

table! {
    account_old (id) {
        id -> Uuid,
        user_id -> Nullable<Uuid>,
        name -> Text,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
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
    email_old (id) {
        id -> Uuid,
        user_id -> Uuid,
        address -> Varchar,
        verified_at -> Nullable<Timestamptz>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
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
    email_verification_old (token) {
        token -> Varchar,
        email_id -> Uuid,
        created_at -> Timestamptz,
    }
}

table! {
    primary_email_old (user_id, email_id) {
        user_id -> Uuid,
        email_id -> Uuid,
    }
}

table! {
    schema_migrations (version) {
        version -> Int8,
        dirty -> Bool,
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
    transaction_entry_old (transaction_id, order) {
        transaction_id -> Uuid,
        order -> Int4,
        account_id -> Uuid,
        amount -> Int8,
    }
}

table! {
    transaction_old (id) {
        id -> Uuid,
        user_id -> Uuid,
        date -> Date,
        payee -> Text,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
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

table! {
    user_old (id) {
        id -> Uuid,
        email -> Text,
        password_hash -> Bpchar,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

joinable!(account -> user (user_id));
joinable!(account_old -> user_old (user_id));
joinable!(email -> user (user_id));
joinable!(email_old -> user_old (user_id));
joinable!(email_verification -> email (email_id));
joinable!(email_verification_old -> email_old (email_id));
joinable!(primary_email_old -> email_old (email_id));
joinable!(primary_email_old -> user_old (user_id));
joinable!(transaction -> user (user_id));
joinable!(transaction_entry -> account (account_id));
joinable!(transaction_entry -> currency (currency));
joinable!(transaction_entry -> transaction (transaction_id));
joinable!(transaction_entry_old -> account_old (account_id));
joinable!(transaction_entry_old -> transaction_old (transaction_id));
joinable!(transaction_old -> user_old (user_id));

allow_tables_to_appear_in_same_query!(
    account,
    account_old,
    currency,
    email,
    email_old,
    email_verification,
    email_verification_old,
    primary_email_old,
    schema_migrations,
    transaction,
    transaction_entry,
    transaction_entry_old,
    transaction_old,
    user,
    user_old,
);
