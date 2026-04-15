CREATE MATERIALIZED VIEW IF NOT EXISTS account_mint_refresh
REFRESH AFTER 60 MINUTE
ENGINE = MergeTree
ORDER BY (account)
AS
SELECT account, mint
FROM account_mint_state FINAL
WHERE mint != ''
SETTINGS max_threads = 4;