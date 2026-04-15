CREATE MATERIALIZED VIEW IF NOT EXISTS close_account_refresh
REFRESH AFTER 60 MINUTE
ENGINE = MergeTree
ORDER BY (account)
AS
SELECT account
FROM close_account_state FINAL
WHERE closed = 1
SETTINGS max_threads = 4;