CREATE TABLE currency (
    -- Three letter currency code.
    code TEXT PRIMARY KEY,
    -- Symbol used when displaying values if one exists.
    symbol TEXT NOT NULL DEFAULT '',
    -- Minor units indicating number of decimal places allowed by the currency.
    minor_units SMALLINT NOT NULL
);
