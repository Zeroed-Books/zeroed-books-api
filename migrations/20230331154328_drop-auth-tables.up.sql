ALTER TABLE "account"
    DROP COLUMN "legacy_user_id";

ALTER TABLE "transaction"
    DROP COLUMN "legacy_user_id";

DROP TABLE "password_resets";
DROP TABLE "email_verification";
DROP TABLE "email";
DROP TABLE "user";
