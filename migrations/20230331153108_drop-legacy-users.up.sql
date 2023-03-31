DELETE FROM "account"
WHERE user_id IS NULL;

DELETE FROM "transaction"
WHERE user_id IS NULL;

ALTER TABLE "account"
    ALTER COLUMN "user_id" SET NOT NULL,
    ADD CONSTRAINT "account_name_user_unique" UNIQUE (user_id, name);

ALTER TABLE "transaction"
    ALTER COLUMN "user_id" SET NOT NULL;

DELETE FROM "user";

-- Get an account by name for a user, or create it if it doesn't exist. The
-- function only returns the account's ID, but this can be used to select the
-- remaining columns if desired. This function was inspired by
-- https://stackoverflow.com/a/15950324/3762084.
CREATE OR REPLACE FUNCTION get_or_create_account(owner_id text,
                                                 account_name text,
                                                 OUT _account_id uuid)
AS
$$
BEGIN
    LOOP
        -- The simplest, and least computationally expensive, case is that the
        -- account exists and we can select from it.
        SELECT account.id
        FROM account
        WHERE user_id = owner_id
          AND name = account_name
        INTO _account_id;

        -- If the select found something, we're done.
        EXIT WHEN FOUND;

        -- If the select did not find the account, try to insert it. This could
        -- fail if the account was just inserted so in that case, we let the
        -- loop continue and pick up the insert in the next try of the select
        -- statement.
        INSERT INTO account AS a (user_id, name)
        VALUES (owner_id, account_name)
        ON CONFLICT (user_id, name) DO NOTHING
        RETURNING a.id INTO _account_id;

        -- If the insert succeeded, we're done. Otherwise try it all again.
        EXIT WHEN FOUND;
    END LOOP;
END;
$$ LANGUAGE "plpgsql";
