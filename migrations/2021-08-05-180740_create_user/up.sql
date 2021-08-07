CREATE TABLE "user" (
    id uuid PRIMARY KEY,
    password TEXT NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    last_login timestamptz
);
