CREATE TABLE "transaction_entry" (
    id uuid PRIMARY KEY DEFAULT uuid_generate_v4(),
    transaction_id uuid NOT NULL REFERENCES "transaction" (id)
        ON DELETE CASCADE,
    "order" INTEGER NOT NULL,
    account_id uuid NOT NULL REFERENCES "account" (id)
        ON DELETE RESTRICT,
    currency TEXT NOT NULL REFERENCES "currency"
        ON DELETE RESTRICT,
    amount INTEGER NOT NULL,

    UNIQUE (transaction_id, "order")
);

CREATE FUNCTION update_transaction_modification_time() RETURNS trigger as $$
BEGIN
    IF (tg_op = 'UPDATE' OR tg_op = 'INSERT') THEN
        UPDATE "transaction" SET updated_at = now()
            WHERE id = NEW.transaction_id;

        RETURN NEW;
    ELSIF tg_op = 'DELETE' THEN
        UPDATE "transaction" SET updated_at = now()
            WHERE id = OLD.transaction_id;

        RETURN OLD;
    END IF;
END;
$$ language plpgsql;

CREATE TRIGGER manage_transaction_modification_time
AFTER INSERT OR UPDATE OR DELETE
ON "transaction_entry"
FOR EACH ROW
EXECUTE PROCEDURE update_transaction_modification_time();
